use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};
use std::collections::HashMap;
use shakmaty::Position;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;
use crate::eval::StateVector;

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

struct MoveRecord { game_id: String, ply: i64, fen: String, hugm_score: i64, player: String, color: String, state_id: Option<u16>, eval_arr: Option<Vec<i64>> }

fn parse_move_record(v: &Value) -> Option<MoveRecord> {
    let rec = v.as_record().ok()?;
    let hugm_score = rec.get("hugm_score")
        .and_then(|v| v.as_int().ok())
        .unwrap_or(0) as i64;
    let player = rec.get("player").and_then(|v| v.as_str().ok()).unwrap_or("unknown").to_string();
    let color = rec.get("color").and_then(|v| v.as_str().ok()).unwrap_or("unknown").to_string();
    let state_id = rec.get("state_id").and_then(|v| v.as_int().ok()).map(|x| x as u16);
    let eval_arr = rec.get("hugm_eval_arr")
        .and_then(|v| v.as_str().ok())
        .and_then(|s| serde_json::from_str::<Vec<i64>>(s).ok());
    Some(MoveRecord {
        game_id: format!("{}", rec.get("game_id")?.as_int().ok()?),
        ply: rec.get("ply")?.as_int().ok()? as i64,
        fen: rec.get("fen")?.as_str().ok()?.to_string(),
        hugm_score,
        player,
        color,
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
            return crate::eval::decode_state_id(sid);
        }
        // Slow path: re-parse FEN (fallback for rows without state_id)
        let fen = match shakmaty::fen::Fen::from_ascii(r.fen.as_bytes()) {
            Ok(f) => f, Err(_) => return StateVector::default(),
        };
        let chess: shakmaty::Chess = match fen.into_position(shakmaty::CastlingMode::Standard) {
            Ok(c) => c, Err(_) => return StateVector::default(),
        };
        let phase = crate::eval::compute_phase(chess.board());
        let groups = crate::eval::compute_groups(&chess, phase, 0);
        let sensor = crate::eval::build_sensor_report(chess.board(), &r.fen, &groups, &chess, phase, None);
        crate::eval::encode_state(&sensor, &groups, phase)
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
        if self.count < 2.0 { 1.0 } else { (self.m2 / (self.count - 1.0)).sqrt().max(1.0) }
    }
}

fn compute_baselines(rows: &[MoveRecord], states: &[StateVector]) -> HashMap<(String, u8, String), (f64, f64)> {
    let mut prev_score: HashMap<(String, String), i64> = HashMap::new();
    let mut prev_eval_arr: HashMap<(String, String), Vec<i64>> = HashMap::new();
    let mut baselines: HashMap<(String, u8, String), Welford> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        let phase_bucket = states.get(i).map(|s| s.phase).unwrap_or(1);
        let game_key = (row.player.clone(), row.game_id.clone());
        let (delta, _signed_delta) = if let Some(prev) = prev_score.get(&game_key).copied() {
            let sd = row.hugm_score - prev;
            ((sd.abs() as f64), sd)
        } else { (0.0, 0) };
        prev_score.insert(game_key.clone(), row.hugm_score);

        if delta >= 1.0 {
            // Overall eval swing baseline
            baselines.entry((row.player.clone(), phase_bucket, "hugm_delta".into()))
                .or_insert_with(Welford::new).update(delta);
            // Binary state-vector concepts: eval swing when pattern was present
            let s = &states[i];
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
            ] {
                if flag {
                    baselines.entry((row.player.clone(), phase_bucket, concept.to_string()))
                        .or_insert_with(Welford::new).update(delta);
                }
            }
        }

        // Eval-component baselines: per-dimension delta (material, pawns, activity, king_safety)
        // Skip the first ply of each game (no prior same-player position to compare against).
        if let Some(arr) = &row.eval_arr {
            if let Some(prev) = prev_eval_arr.get(&game_key) {
                for (idx, name) in [(0usize, "material"), (1, "pawn_structure"), (2, "activity"), (3, "king_safety")] {
                    let curr = arr.get(idx).copied().unwrap_or(0);
                    let prv  = prev.get(idx).copied().unwrap_or(0);
                    let comp_delta = (curr - prv).abs() as f64;
                    if comp_delta >= 1.0 {
                        baselines.entry((row.player.clone(), phase_bucket, name.to_string()))
                            .or_insert_with(Welford::new).update(comp_delta);
                    }
                }
            }
            prev_eval_arr.insert(game_key.clone(), arr.clone());
        }
    }
    baselines.into_iter().map(|((p, ph, cn), w)| ((p, ph, cn), (w.mean, w.std_dev()))).collect()
}

fn detect_anomalies(rows: &[MoveRecord], states: &[StateVector], baselines: &HashMap<(String, u8, String), (f64, f64)>, min_games: i64, span: nu_protocol::Span) -> Vec<Value> {
    let _ = min_games;
    let mut prev_score: HashMap<(String, String), i64> = HashMap::new();
    let mut prev_eval_arr: HashMap<(String, String), Vec<i64>> = HashMap::new();
    let mut results = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        let game_key = (row.player.clone(), row.game_id.clone());
        let (delta, signed_delta) = if let Some(prev) = prev_score.get(&game_key).copied() {
            let sd = row.hugm_score - prev;
            ((sd.abs() as f64), sd)
        } else { (0.0, 0) };
        prev_score.insert(game_key.clone(), row.hugm_score);

        let s = &states[i];
        let phase_bucket = s.phase;
        let state_id = s.state_id as i64;

        // Binary state-vector anomalies (eval swing when pattern was present)
        if delta >= 30.0 {
            let check_concepts: [(&str, bool); 10] = [
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
            ];
            for (concept, should_check) in &check_concepts {
                if !should_check { continue; }
                let key = (row.player.clone(), phase_bucket, concept.to_string());
                if let Some((mean, std)) = baselines.get(&key) {
                    let z = (delta - mean) / std;
                    if z > 2.0 {
                        let hurt_player = (row.color == "white" && signed_delta < 0)
                            || (row.color == "black" && signed_delta > 0);
                        results.push(make_anomaly(&row.player, &row.game_id, row.ply, state_id, concept, z, delta, signed_delta as f64, hurt_player, span));
                    }
                }
            }
        }

        // Eval-component anomalies: per-dimension delta (continuous, no binary gate)
        // Skip the first ply of each game — same guard as baselines.
        if let Some(arr) = &row.eval_arr {
            if let Some(prev) = prev_eval_arr.get(&game_key) {
                for (idx, name) in [(0usize, "material"), (1, "pawn_structure"), (2, "activity"), (3, "king_safety")] {
                    let curr = arr.get(idx).copied().unwrap_or(0);
                    let prv  = prev.get(idx).copied().unwrap_or(0);
                    let comp_delta = (curr - prv).abs() as f64;
                    let comp_signed = (curr - prv) as f64;
                    if comp_delta >= 30.0 {
                        let key = (row.player.clone(), phase_bucket, name.to_string());
                        if let Some((mean, std)) = baselines.get(&key) {
                            let z = (comp_delta - mean) / std;
                            if z > 2.0 {
                                let hurt_player = (row.color == "white" && comp_signed < 0.0)
                                    || (row.color == "black" && comp_signed > 0.0);
                                results.push(make_anomaly(&row.player, &row.game_id, row.ply, state_id, name, z, comp_delta, comp_signed, hurt_player, span));
                            }
                        }
                    }
                }
            }
            prev_eval_arr.insert(game_key.clone(), arr.clone());
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

fn compute_transitions(rows: &[MoveRecord], states: &[StateVector], _min_games: i64, span: nu_protocol::Span) -> (Vec<Value>, Vec<Value>) {
    let mut transitions: HashMap<(i64, i64), (i64, i64)> = HashMap::new(); // (state_from, state_to) → (total, blunders)
    let mut prev: Option<(String, i64, i64)> = None; // (game_id, state_id, score)

    for (i, row) in rows.iter().enumerate() {
        let state_id = states.get(i).map(|s| s.state_id as i64).unwrap_or(0);
        if let Some((pg, prev_state, pscore)) = prev.take() {
            if pg == row.game_id {
                let delta = row.hugm_score - pscore;
                let entry = transitions.entry((prev_state, state_id)).or_insert((0, 0));
                entry.0 += 1;
                if delta < -200 { entry.1 += 1; } // blunder: lost > 200cp
            }
        }
        prev = Some((row.game_id.clone(), state_id, row.hugm_score));
    }

    let mut trans_list = Vec::new();
    let mut trans_anomalies = Vec::new();

    for ((from, to), (total, blunders)) in &transitions {
        let risk = if *total > 0 { *blunders as f64 / *total as f64 } else { 0.0 };
        trans_list.push(Value::record(nu_protocol::record! {
            "state_from" => Value::int(*from, span),
            "state_to" => Value::int(*to, span),
            "total_count" => Value::int(*total, span),
            "blunder_count" => Value::int(*blunders, span),
            "blunder_risk" => Value::float(risk, span),
        }, span));

        if risk > 0.25 && *total >= 3 {
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

fn format_results(rows: &[MoveRecord], states: &[StateVector], baselines: &HashMap<(String, u8, String), (f64, f64)>, span: nu_protocol::Span) -> (Value, Value) {
    let states_out: Vec<Value> = rows.iter().zip(states.iter())
        .map(|(row, state)| state_vector_to_value(&row.game_id, row.ply, state, span))
        .collect();
    let bl: Vec<Value> = baselines.iter().map(|((player, ph, cn), (mean, std))| {
        Value::record(nu_protocol::record! {
            "player" => Value::string(player, span),
            "phase_bucket" => Value::int(*ph as i64, span),
            "concept" => Value::string(cn, span),
            "mean" => Value::float(*mean, span),
            "std" => Value::float(*std, span),
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
        let phase = crate::eval::compute_phase(chess.board());
        let groups = crate::eval::compute_groups(&chess, phase, 0);
        let sensor = crate::eval::build_sensor_report(chess.board(), fen, &groups, &chess, phase, None);
        let slow = crate::eval::encode_state(&sensor, &groups, phase);

        let fast = crate::eval::decode_state_id(slow.state_id);

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
}
