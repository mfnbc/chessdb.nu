use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};
use std::collections::HashMap;
use shakmaty::Position;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;
use crate::eval::{
    build_sensor_report, compute_groups, compute_phase, decode_state_id, encode_state, StateVector,
};

/// The three tiers a per-move eval swing passes through before it becomes a
/// stored anomaly, smallest gate first:
///
/// 1. `NOISE_FLOOR_CP` — too small to tell from engine jitter; not even fed
///    into a baseline (`compute_baselines`).
/// 2. `ANOMALY_CANDIDATE_CP` — big enough to check against a baseline at all
///    (`detect_anomalies`'s `delta >= ...` gate) — well below "blunder," just
///    "worth comparing."
/// 3. `ANOMALY_Z_THRESHOLD` standard deviations above that baseline's own
///    mean, and only once the baseline itself has `count >= min_games`
///    samples (the `--min-games` CLI flag) — is what actually gets stored as
///    an anomaly.
const NOISE_FLOOR_CP: f64 = 1.0;
const ANOMALY_CANDIDATE_CP: f64 = 30.0;
const ANOMALY_Z_THRESHOLD: f64 = 2.0;
/// Floor on a Welford baseline's own standard deviation (see `Welford::std_dev`)
/// — without it, a remarkably consistent player/concept/phase combination
/// could have a near-zero std dev and turn any swing into an absurd z-score.
const STD_DEV_FLOOR_CP: f64 = 1.0;

/// A single transition's own, unrelated threshold: this many centipawns lost
/// in one ply, regardless of any baseline, counts as a blunder for
/// `compute_transitions`'s risk-rate tracking.
const BLUNDER_LOSS_CP: i64 = 200;
/// Blunder rate above which a (state_from, state_to) transition is flagged
/// risky — same `min_games` sample-size gate as the anomaly baselines above.
const RISKY_TRANSITION_RATE: f64 = 0.25;

/// The first four `hugm_eval_arr` positions (see `process_corpus.rs`'s
/// `PendingPos` construction, the one place this array is built, and
/// `chessdb/profile.nu`'s `json_extract(p.hugm_eval_arr, '$[N]')` reads,
/// which must stay in this same order) — named here once so
/// `compute_baselines` and `detect_anomalies` share one index contract
/// instead of two independently-maintained copies of the same list.
const EVAL_ARR_COMPONENTS: [(usize, &str); 4] =
    [(0, "material"), (1, "pawn_structure"), (2, "activity"), (3, "king_safety")];

pub struct DeriveCoachSignals;

impl PluginCommand for DeriveCoachSignals {
    type Plugin = ChessdbPlugin;
    fn name(&self) -> &str { "chessdb derive-coach-signals" }
    fn description(&self) -> &str {
        "Compute per-player baselines, state encodings, and anomaly alerts from a table of moves"
    }
    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![
                (Type::List(Box::new(Type::Record(vec![].into()))), Type::Record(vec![].into())),
            ])
            .named("min-games", nu_protocol::SyntaxShape::Int, "min samples for baseline trust (default 25)", None)
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }
    fn run(&self, _plugin: &Self::Plugin, _engine: &nu_plugin::EngineInterface, call: &EvaluatedCall, input: PipelineData) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let min_games: i64 = call.get_flag("min-games")?.unwrap_or(25);
        let input_value = input.into_value(span)?;

        let rows: Vec<MoveRecord> = match input_value {
            Value::List { vals, .. } => vals.iter().filter_map(parse_move_record).collect(),
            _ => return Err(LabeledError::new("Expected list of records with game_id, ply, fen, hugm_score")),
        };

        let states = encode_move_states(&rows);
        let baselines = compute_baselines(&rows, &states);
        let anomalies = detect_anomalies(&rows, &states, &baselines, min_games, span);
        let (transitions, transition_anomalies) = compute_transitions(&rows, &states, min_games, span);
        let (states_out, baselines_out) = format_results(&rows, &states, &baselines, span);

        Ok(PipelineData::Value(Value::record(nu_protocol::record! {
            "states" => states_out,
            "baselines" => baselines_out,
            "anomalies" => Value::list(anomalies, span),
            "transitions" => Value::list(transitions, span),
            "transition_anomalies" => Value::list(transition_anomalies, span),
        }, span), None))
    }
}

struct MoveRecord { game_id: String, ply: i64, fen: String, hugm_score: i64, player: String, state_id: Option<u32>, eval_arr: Option<Vec<i64>> }

fn parse_move_record(v: &Value) -> Option<MoveRecord> {
    let rec = v.as_record().ok()?;
    let hugm_score = rec.get("hugm_score")
        .and_then(|v| v.as_int().ok())
        .unwrap_or(0);
    let player = rec.get("player").and_then(|v| v.as_str().ok()).unwrap_or("unknown").to_string();
    let state_id = rec.get("state_id").and_then(|v| v.as_int().ok()).map(|x| x as u32);
    let eval_arr = rec.get("hugm_eval_arr")
        .and_then(|v| v.as_str().ok())
        .and_then(|s| serde_json::from_str::<Vec<i64>>(s).ok());
    Some(MoveRecord {
        game_id: format!("{}", rec.get("game_id")?.as_int().ok()?),
        ply: rec.get("ply")?.as_int().ok()?,
        fen: rec.get("fen")?.as_str().ok()?.to_string(),
        hugm_score,
        player,
        state_id,
        eval_arr,
    })
}

/// Encode move states, using pre-computed state_id from ingestion when available.
/// Falls back to full FEN→shakmaty→eval→encode_state path only for rows missing state_id.
/// Returns typed `StateVector`s — converted to `Value` only at the Nu-output
/// boundary in `format_results`, not round-tripped through a string-keyed
/// record in between.
fn encode_move_states(rows: &[MoveRecord]) -> Vec<StateVector> {
    rows.iter().map(|r| {
        // Fast path: state_id already computed during process_corpus ingestion.
        // decode_state_id shares its bit layout with encode_state (concepts.rs),
        // so this can't silently desync the way a hand-rolled decoder could.
        if let Some(sid) = r.state_id {
            return decode_state_id(sid);
        }
        // Slow path: re-parse FEN (fallback for rows without state_id)
        let fen = match shakmaty::fen::Fen::from_ascii(r.fen.as_bytes()) {
            Ok(f) => f, Err(_) => return StateVector::default(),
        };
        let chess: shakmaty::Chess = match fen.into_position(shakmaty::CastlingMode::Standard) {
            Ok(c) => c, Err(_) => return StateVector::default(),
        };
        let phase = compute_phase(chess.board());
        let groups = compute_groups(&chess, phase, 0);
        let sensor = build_sensor_report(chess.board(), &r.fen, &groups, &chess, phase, None);
        encode_state(&sensor, &groups, phase)
    }).collect()
}

/// The one place a `StateVector` becomes a Nu `Value` — field names here are
/// the external contract (`chessdb/sync.nu`, `chessdb/profile.nu` query these
/// by name) and must not change independent of this refactor.
fn state_vector_to_value(game_id: &str, ply: i64, state: &StateVector, span: nu_protocol::Span) -> Value {
    Value::record(nu_protocol::record! {
        "game_id"         => Value::string(game_id, span),
        "ply"             => Value::int(ply, span),
        "state_id"        => Value::int(state.state_id as i64, span),
        "phase_bucket"    => Value::int(state.phase as i64, span),
        "king_exposed"    => Value::bool(state.king_exposed, span),
        "has_fork"        => Value::bool(state.has_fork, span),
        "has_pin"         => Value::bool(state.has_pin, span),
        "has_hanging"     => Value::bool(state.has_hanging, span),
        "has_outpost"     => Value::bool(state.has_outpost, span),
        "has_open_file"   => Value::bool(state.open_file, span),
        "has_passed_pawn" => Value::bool(state.has_passed_pawn, span),
        "has_skewer"      => Value::bool(state.has_skewer, span),
        "has_discovered"  => Value::bool(state.has_discovered, span),
        "has_outnumbered"    => Value::bool(state.has_outnumbered, span),
        "has_overloaded"     => Value::bool(state.has_overloaded, span),
        "has_false_defense"  => Value::bool(state.has_false_defense, span),
        "has_false_safety"   => Value::bool(state.has_false_safety, span),
    }, span)
}

#[derive(Debug, Clone)]
struct Welford { count: f64, mean: f64, m2: f64 }

impl Welford {
    fn new() -> Self { Welford { count: 0.0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, value: f64) {
        self.count += 1.0;
        let delta = value - self.mean;
        self.mean += delta / self.count;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }
    fn std_dev(&self) -> f64 {
        // Sample variance needs at least 2 points; floor the result so a
        // near-zero-variance baseline (a player who's remarkably consistent
        // in this concept/phase) can't blow up z-scores by dividing by
        // something tiny.
        if self.count < 2.0 { STD_DEV_FLOOR_CP } else { (self.m2 / (self.count - 1.0)).sqrt().max(STD_DEV_FLOOR_CP) }
    }
}

/// One row's already-computed eval-swing data: the overall hugm_score delta
/// since this player's previous row in the same game, plus whichever
/// per-eval-component deltas are available (empty until a prior `eval_arr`
/// exists for this (player, game_id) — i.e. never on a game's first row for
/// that player). Computed once by `compute_row_deltas` and shared by
/// `compute_baselines` and `detect_anomalies`, which previously each
/// independently replayed the same (player, game_id)-keyed prev-score/
/// prev-eval_arr bookkeeping to arrive at identical numbers.
struct RowDelta {
    player: String,
    game_id: String,
    ply: i64,
    state: StateVector,
    delta: f64,
    signed_delta: i64,
    /// (component name, abs delta, signed delta) for each of `EVAL_ARR_COMPONENTS`.
    component_deltas: Vec<(&'static str, f64, f64)>,
}

fn compute_row_deltas(rows: &[MoveRecord], states: &[StateVector]) -> Vec<RowDelta> {
    let mut prev_score: HashMap<(String, String), i64> = HashMap::new();
    let mut prev_eval_arr: HashMap<(String, String), Vec<i64>> = HashMap::new();

    rows.iter().enumerate().map(|(i, row)| {
        let game_key = (row.player.clone(), row.game_id.clone());
        let (delta, signed_delta) = if let Some(prev) = prev_score.get(&game_key).copied() {
            let sd = row.hugm_score - prev;
            (sd.abs() as f64, sd)
        } else { (0.0, 0) };
        prev_score.insert(game_key.clone(), row.hugm_score);

        let mut component_deltas = Vec::new();
        if let Some(arr) = &row.eval_arr {
            if let Some(prev) = prev_eval_arr.get(&game_key) {
                for (idx, name) in EVAL_ARR_COMPONENTS {
                    let curr = arr.get(idx).copied().unwrap_or(0);
                    let prv  = prev.get(idx).copied().unwrap_or(0);
                    component_deltas.push((name, (curr - prv).abs() as f64, (curr - prv) as f64));
                }
            }
            prev_eval_arr.insert(game_key, arr.clone());
        }

        // `states` is always `encode_move_states(rows)` (guaranteed same
        // length) at the one real call site (`DeriveCoachSignals::run`) —
        // this default is purely defensive against a mismatched-length
        // caller, never exercised in practice.
        let state = states.get(i).copied().unwrap_or_default();

        RowDelta { player: row.player.clone(), game_id: row.game_id.clone(), ply: row.ply, state, delta, signed_delta, component_deltas }
    }).collect()
}

fn compute_baselines(rows: &[MoveRecord], states: &[StateVector]) -> HashMap<(String, u8, String), (f64, f64, i64)> {
    let mut baselines: HashMap<(String, u8, String), Welford> = HashMap::new();
    for rd in compute_row_deltas(rows, states) {
        let phase_bucket = rd.state.phase;

        if rd.delta >= NOISE_FLOOR_CP {
            // Overall eval swing baseline
            baselines.entry((rd.player.clone(), phase_bucket, "hugm_delta".into()))
                .or_insert_with(Welford::new).update(rd.delta);
            // Binary state-vector concepts: eval swing when pattern was present
            let s = &rd.state;
            for (concept, flag) in [
                ("fork",              s.has_fork),
                ("pin",               s.has_pin),
                ("hanging_piece",     s.has_hanging),
                ("outpost",           s.has_outpost),
                ("open_file",         s.open_file),
                ("passed_pawn",       s.has_passed_pawn),
                ("skewer",            s.has_skewer),
                ("discovered_attack", s.has_discovered),
                ("king_exposed",      s.king_exposed),
                // Failure-lattice rungs (threat_graph.rs module doc,
                // PLAN.md): raw-count outnumbered, commitment-elsewhere
                // (overloaded/false_defense), and the composite rung above
                // both (false_safety) — baselined the same way as every
                // other binary state-vector concept above, so a player's
                // eval-swing tendency at each specific rung becomes visible,
                // not just an aggregate "missed a tactic" signal.
                ("outnumbered",       s.has_outnumbered),
                ("overloaded",        s.has_overloaded),
                ("false_defense",     s.has_false_defense),
                ("false_safety",      s.has_false_safety),
            ] {
                if flag {
                    baselines.entry((rd.player.clone(), phase_bucket, concept.to_string()))
                        .or_insert_with(Welford::new).update(rd.delta);
                }
            }
        }

        // Eval-component baselines: per-dimension delta (material, pawns, activity, king_safety).
        // Empty on a game's first row for this player — no prior position to compare against.
        for (name, comp_delta, _signed) in &rd.component_deltas {
            if *comp_delta >= NOISE_FLOOR_CP {
                baselines.entry((rd.player.clone(), phase_bucket, name.to_string()))
                    .or_insert_with(Welford::new).update(*comp_delta);
            }
        }
    }
    baselines.into_iter().map(|((p, ph, cn), w)| ((p, ph, cn), (w.mean, w.std_dev(), w.count as i64))).collect()
}

fn detect_anomalies(rows: &[MoveRecord], states: &[StateVector], baselines: &HashMap<(String, u8, String), (f64, f64, i64)>, min_games: i64, span: nu_protocol::Span) -> Vec<Value> {
    let mut results = Vec::new();

    for rd in compute_row_deltas(rows, states) {
        let s = &rd.state;
        let phase_bucket = s.phase;
        let state_id = s.state_id as i64;

        // Binary state-vector anomalies (eval swing when pattern was present)
        if rd.delta >= ANOMALY_CANDIDATE_CP {
            let check_concepts: [(&str, bool); 14] = [
                ("hugm_delta",        true),
                ("fork",              s.has_fork),
                ("pin",               s.has_pin),
                ("hanging_piece",     s.has_hanging),
                ("outpost",           s.has_outpost),
                ("open_file",         s.open_file),
                ("passed_pawn",       s.has_passed_pawn),
                ("skewer",            s.has_skewer),
                ("discovered_attack", s.has_discovered),
                ("king_exposed",      s.king_exposed),
                ("outnumbered",       s.has_outnumbered),
                ("overloaded",        s.has_overloaded),
                ("false_defense",     s.has_false_defense),
                ("false_safety",      s.has_false_safety),
            ];
            for (concept, should_check) in &check_concepts {
                if !should_check { continue; }
                let key = (rd.player.clone(), phase_bucket, concept.to_string());
                if let Some((mean, std, count)) = baselines.get(&key) {
                    // Don't trust a baseline built from too few samples —
                    // this is the entire point of --min-games: a Welford
                    // mean/std from a handful of games is noisy enough that
                    // its z-scores aren't meaningful yet.
                    if *count < min_games { continue; }
                    let z = (rd.delta - mean) / std;
                    if z > ANOMALY_Z_THRESHOLD {
                        // signed_delta compares this player's own hugm_score
                        // across two of their own consecutive moves, so both
                        // terms are relative to the same side: their
                        // opponent (whoever is to move right after this
                        // player's move). A positive signed_delta always
                        // means the opponent's advantage grew — this player
                        // was hurt — regardless of which color they are.
                        let hurt_player = rd.signed_delta > 0;
                        results.push(make_anomaly(&rd.player, &rd.game_id, rd.ply, state_id, concept, z, rd.delta, rd.signed_delta as f64, hurt_player, span));
                    }
                }
            }
        }

        // Eval-component anomalies: per-dimension delta (continuous, no binary gate)
        for (name, comp_delta, comp_signed) in &rd.component_deltas {
            if *comp_delta >= ANOMALY_CANDIDATE_CP {
                let key = (rd.player.clone(), phase_bucket, name.to_string());
                if let Some((mean, std, count)) = baselines.get(&key) {
                    if *count < min_games { continue; }
                    let z = (*comp_delta - mean) / std;
                    if z > ANOMALY_Z_THRESHOLD {
                        // Same reasoning as signed_delta above: both terms
                        // are this player's own consecutive rows, so
                        // comp_signed > 0 always means the opponent's
                        // component-level advantage grew.
                        let hurt_player = *comp_signed > 0.0;
                        results.push(make_anomaly(&rd.player, &rd.game_id, rd.ply, state_id, name, z, *comp_delta, *comp_signed, hurt_player, span));
                    }
                }
            }
        }
    }
    results
}

#[allow(clippy::too_many_arguments)]
fn make_anomaly(player: &str, game_id: &str, ply: i64, state_id: i64, concept: &str, z: f64, severity: f64, signed_delta: f64, hurt_player: bool, span: nu_protocol::Span) -> Value {
    Value::record(nu_protocol::record! {
        "player"       => Value::string(player, span),
        "game_id"      => Value::string(game_id, span),
        "ply"          => Value::int(ply, span),
        "state_id"     => Value::int(state_id, span),
        "anomaly_type" => Value::string("z_score", span),
        "concept_name" => Value::string(concept, span),
        "z_score"      => Value::float(z, span),
        "severity"     => Value::float(severity, span),
        "signed_delta" => Value::float(signed_delta, span),
        "hurt_player"  => Value::bool(hurt_player, span),
    }, span)
}

fn compute_transitions(rows: &[MoveRecord], states: &[StateVector], min_games: i64, span: nu_protocol::Span) -> (Vec<Value>, Vec<Value>) {
    let mut transitions: HashMap<(i64, i64), (i64, i64)> = HashMap::new(); // (state_from, state_to) → (total, blunders)
    let mut prev: Option<(String, i64, i64)> = None; // (game_id, state_id, score)

    for (i, row) in rows.iter().enumerate() {
        let state_id = states.get(i).map(|s| s.state_id as i64).unwrap_or(0);
        if let Some((pg, prev_state, pscore)) = prev.take() {
            if pg == row.game_id {
                let delta = row.hugm_score - pscore;
                let entry = transitions.entry((prev_state, state_id)).or_insert((0, 0));
                entry.0 += 1;
                if delta < -BLUNDER_LOSS_CP { entry.1 += 1; }
            }
        }
        prev = Some((row.game_id.clone(), state_id, row.hugm_score));
    }

    let mut trans_list = Vec::new();
    let mut trans_anomalies = Vec::new();

    // Sorted so repeated runs over the same input produce the same row
    // order — HashMap iteration order is otherwise randomized per process.
    let mut transitions: Vec<_> = transitions.into_iter().collect();
    transitions.sort_by_key(|&((from, to), _)| (from, to));

    for ((from, to), (total, blunders)) in &transitions {
        let risk = if *total > 0 { *blunders as f64 / *total as f64 } else { 0.0 };
        trans_list.push(Value::record(nu_protocol::record! {
            "state_from" => Value::int(*from, span),
            "state_to" => Value::int(*to, span),
            "total_count" => Value::int(*total, span),
            "blunder_count" => Value::int(*blunders, span),
            "blunder_risk" => Value::float(risk, span),
        }, span));

        if risk > RISKY_TRANSITION_RATE && *total >= min_games {
            trans_anomalies.push(Value::record(nu_protocol::record! {
                "state_from" => Value::int(*from, span),
                "state_to" => Value::int(*to, span),
                "anomaly_type" => Value::string("transition_risk", span),
                "blunder_risk" => Value::float(risk, span),
                "total_count" => Value::int(*total, span),
                "blunder_count" => Value::int(*blunders, span),
            }, span));
        }
    }

    (trans_list, trans_anomalies)
}

fn format_results(rows: &[MoveRecord], states: &[StateVector], baselines: &HashMap<(String, u8, String), (f64, f64, i64)>, span: nu_protocol::Span) -> (Value, Value) {
    let states_out: Vec<Value> = rows.iter().zip(states.iter())
        .map(|(row, state)| state_vector_to_value(&row.game_id, row.ply, state, span))
        .collect();
    // Sorted so repeated runs over the same input produce the same row
    // order — HashMap iteration order is otherwise randomized per process.
    let mut sorted_baselines: Vec<_> = baselines.iter().collect();
    sorted_baselines.sort_by(|a, b| a.0.cmp(b.0));
    let bl: Vec<Value> = sorted_baselines.into_iter().map(|((player, ph, cn), (mean, std, count))| {
        Value::record(nu_protocol::record! {
            "player" => Value::string(player, span),
            "phase_bucket" => Value::int(*ph as i64, span),
            "concept" => Value::string(cn, span),
            "mean" => Value::float(*mean, span),
            "std" => Value::float(*std, span),
            "count" => Value::int(*count, span),
        }, span)
    }).collect();
    (Value::list(states_out, span), Value::list(bl, span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_and_slow_path_agree_on_state_id() {
        let fen = "rnb1kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1qPP/RNBQKB1R w KQkq - 0 1";
        let parsed = shakmaty::fen::Fen::from_ascii(fen.as_bytes()).unwrap();
        let chess: shakmaty::Chess = parsed.into_position(shakmaty::CastlingMode::Standard).unwrap();
        let phase = compute_phase(chess.board());
        let groups = compute_groups(&chess, phase, 0);
        let sensor = build_sensor_report(chess.board(), fen, &groups, &chess, phase, None);
        let slow = encode_state(&sensor, &groups, phase);

        let fast = decode_state_id(slow.state_id);

        assert_eq!(fast.state_id, slow.state_id);
        assert_eq!(fast.phase, slow.phase);
        assert_eq!(fast.material_sign, slow.material_sign);
        assert_eq!(fast.king_exposed, slow.king_exposed);
        assert_eq!(fast.in_check, slow.in_check);
        assert_eq!(fast.has_fork, slow.has_fork);
        assert_eq!(fast.has_pin, slow.has_pin);
        assert_eq!(fast.has_hanging, slow.has_hanging);
        assert_eq!(fast.has_outpost, slow.has_outpost);
        assert_eq!(fast.open_file, slow.open_file);
        assert_eq!(fast.has_passed_pawn, slow.has_passed_pawn);
        assert_eq!(fast.has_skewer, slow.has_skewer);
        assert_eq!(fast.has_discovered, slow.has_discovered);
    }

    fn move_record(player: &str, game_id: &str, ply: i64, hugm_score: i64) -> MoveRecord {
        MoveRecord {
            game_id: game_id.to_string(),
            ply,
            fen: String::new(),
            hugm_score,
            player: player.to_string(),
            state_id: Some(0),
            eval_arr: None,
        }
    }

    // Regression for a real sign bug (found 2026-07-30 auditing chess-profile
    // output): hugm_score, for a player's own consecutive moves, is always
    // relative to their *opponent* (whoever is to move right after this
    // player's move) — so `signed_delta = curr - prev` across two of a
    // player's own rows always means "how much did my opponent's advantage
    // change," regardless of which color the player is. hurt_player must
    // therefore be `signed_delta > 0` unconditionally; a color-conditional
    // version (as the code used to have) gets one color backwards.
    #[test]
    fn hurt_player_is_positive_signed_delta_regardless_of_color() {
        // Bypass compute_baselines (Welford stats over tiny hand-built
        // samples are fragile to get right) and hand-supply a tight
        // baseline (mean=0, std=1, count=25) directly, so any real swing
        // clears the z > 2.0 gate and the min-games trust gate — isolates
        // exactly the hurt_player sign logic in detect_anomalies.
        let mut baselines = HashMap::new();
        baselines.insert(("white_player".to_string(), 0u8, "hugm_delta".to_string()), (0.0, 1.0, 25));
        baselines.insert(("black_player".to_string(), 0u8, "hugm_delta".to_string()), (0.0, 1.0, 25));

        let rows = vec![
            // White's own two consecutive moves: hugm_score is Black-relative
            // both times, so signed_delta = 500 - 0 means Black's advantage
            // grew by 500 — white_player was hurt.
            move_record("white_player", "g1", 0, 0),
            move_record("white_player", "g1", 2, 500),
            // Black's own two consecutive moves: hugm_score is White-relative
            // both times, so signed_delta = 500 - 0 means White's advantage
            // grew by 500 — black_player was hurt.
            move_record("black_player", "g2", 1, 0),
            move_record("black_player", "g2", 3, 500),
        ];
        let states = vec![StateVector::default(); rows.len()];
        let anomalies = detect_anomalies(&rows, &states, &baselines, 0, nu_protocol::Span::test_data());

        assert_eq!(anomalies.len(), 2, "expected exactly one hugm_delta anomaly per player, got {anomalies:?}");
        for a in &anomalies {
            let rec = a.as_record().unwrap();
            let player = rec.get("player").unwrap().as_str().unwrap();
            let hurt = rec.get("hurt_player").unwrap().as_bool().unwrap();
            assert!(hurt, "player {player} should be flagged as hurt_player=true (their opponent's advantage grew by 500), got false");
        }
    }

    // Regression for a real gap (found 2026-07-30 in the same audit as
    // hurt_player above): --min-games was accepted by the command but
    // silently discarded (`let _ = min_games;`) — detect_anomalies fired
    // z-score anomalies off a baseline regardless of how few samples built
    // it. Welford's real sample count is now threaded through
    // compute_baselines -> detect_anomalies and gates emission.
    #[test]
    fn detect_anomalies_respects_min_games_baseline_trust() {
        let mut baselines = HashMap::new();
        baselines.insert(("p".to_string(), 0u8, "hugm_delta".to_string()), (0.0, 1.0, 5));

        let rows = vec![
            move_record("p", "g1", 0, 0),
            move_record("p", "g1", 2, 500),
        ];
        let states = vec![StateVector::default(); rows.len()];

        // Baseline has only 5 samples; requiring 25 must suppress the anomaly.
        let suppressed = detect_anomalies(&rows, &states, &baselines, 25, nu_protocol::Span::test_data());
        assert!(suppressed.is_empty(), "anomaly should be suppressed when count(5) < min_games(25), got {suppressed:?}");

        // The same baseline clears a lower bar.
        let allowed = detect_anomalies(&rows, &states, &baselines, 5, nu_protocol::Span::test_data());
        assert!(!allowed.is_empty(), "anomaly should fire when count(5) >= min_games(5)");
    }

    // Regression (found 2026-07-30 auditing for reproducibility): format_results
    // and compute_transitions used to build their output lists straight off
    // HashMap iteration, whose order Rust randomizes per hasher instance —
    // identical input could come back with rows in a different order on every
    // run. Both are now sorted by key before being turned into output rows.
    #[test]
    fn baseline_output_rows_are_sorted_deterministically() {
        let span = nu_protocol::Span::test_data();
        let mut baselines = HashMap::new();
        baselines.insert(("zed".to_string(), 1u8, "fork".to_string()), (1.0, 1.0, 10));
        baselines.insert(("ann".to_string(), 0u8, "hugm_delta".to_string()), (2.0, 1.0, 10));
        baselines.insert(("ann".to_string(), 2u8, "pin".to_string()), (3.0, 1.0, 10));

        let (_, bl) = format_results(&[], &[], &baselines, span);
        let rows = bl.as_list().unwrap();
        let keys: Vec<(String, i64, String)> = rows.iter().map(|r| {
            let rec = r.as_record().unwrap();
            (
                rec.get("player").unwrap().as_str().unwrap().to_string(),
                rec.get("phase_bucket").unwrap().as_int().unwrap(),
                rec.get("concept").unwrap().as_str().unwrap().to_string(),
            )
        }).collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys, "baseline rows must be sorted by (player, phase_bucket, concept)");
    }

    #[test]
    fn transition_output_rows_are_sorted_deterministically() {
        let span = nu_protocol::Span::test_data();
        // Three games producing transitions in an order that would only
        // coincidentally already be sorted — forces a real sort to pass.
        let rows = vec![
            move_record("p", "g1", 0, 0),
            move_record("p", "g1", 1, 0),
            move_record("p", "g2", 0, 0),
            move_record("p", "g2", 1, 0),
            move_record("p", "g3", 0, 0),
            move_record("p", "g3", 1, 0),
        ];
        let mut states = vec![StateVector::default(); rows.len()];
        states[0].state_id = 9; states[1].state_id = 1;
        states[2].state_id = 5; states[3].state_id = 2;
        states[4].state_id = 1; states[5].state_id = 9;

        let (trans_list, _) = compute_transitions(&rows, &states, 0, span);
        let pairs: Vec<(i64, i64)> = trans_list.iter().map(|r| {
            let rec = r.as_record().unwrap();
            (rec.get("state_from").unwrap().as_int().unwrap(), rec.get("state_to").unwrap().as_int().unwrap())
        }).collect();
        let mut sorted_pairs = pairs.clone();
        sorted_pairs.sort();
        assert_eq!(pairs, sorted_pairs, "transition rows must be sorted by (state_from, state_to)");
    }

    // Regression for the compute_baselines/detect_anomalies dedup (2026-07-30):
    // both now share compute_row_deltas for the eval_arr component-delta path
    // (material/pawn_structure/activity/king_safety), which no prior test
    // exercised at all (move_record's eval_arr defaults to None). Locks in
    // that a component delta feeds its own named baseline (not hugm_delta),
    // and that detect_anomalies' component path fires correctly against it —
    // proving the shared row-delta computation didn't silently change either
    // function's behavior.
    #[test]
    fn eval_component_deltas_feed_baselines_and_anomalies() {
        let span = nu_protocol::Span::test_data();
        let mut r1 = move_record("p", "g1", 0, 0);
        r1.eval_arr = Some(vec![100, 50, 30, 20]);
        let mut r2 = move_record("p", "g1", 2, 0);
        r2.eval_arr = Some(vec![250, 50, 30, 20]); // material +150, others unchanged
        let rows = vec![r1, r2];
        let states = vec![StateVector::default(); rows.len()];

        let baselines = compute_baselines(&rows, &states);
        let (mean, _std, count) = baselines.get(&("p".to_string(), 0u8, "material".to_string()))
            .expect("material component baseline should exist");
        assert_eq!(*count, 1);
        assert_eq!(*mean, 150.0, "material delta should be |250-100|=150");
        assert!(
            !baselines.contains_key(&("p".to_string(), 0u8, "pawn_structure".to_string())),
            "unchanged components (delta=0, below NOISE_FLOOR_CP) must not get a baseline entry"
        );
        assert!(
            !baselines.contains_key(&("p".to_string(), 0u8, "hugm_delta".to_string())),
            "hugm_score itself didn't move, so no overall hugm_delta baseline should exist"
        );

        let mut tight_baseline = HashMap::new();
        tight_baseline.insert(("p".to_string(), 0u8, "material".to_string()), (0.0, 1.0, 25));
        let anomalies = detect_anomalies(&rows, &states, &tight_baseline, 0, span);
        assert_eq!(anomalies.len(), 1, "expected exactly one material-component anomaly, got {anomalies:?}");
        let rec = anomalies[0].as_record().unwrap();
        assert_eq!(rec.get("concept_name").unwrap().as_str().unwrap(), "material");
        assert_eq!(rec.get("severity").unwrap().as_float().unwrap(), 150.0);
        assert!(rec.get("hurt_player").unwrap().as_bool().unwrap(), "material component rose (comp_signed > 0), so the player should be flagged as hurt");
    }
}
