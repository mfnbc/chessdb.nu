use crate::eval::position::EvalGroups;
use crate::eval::sensor::SensorReport;
use crate::eval::concept_types::{GatedIssue, Side};

/// A named concept detected in a chess position, with severity and ELO threshold.
#[derive(Debug, serde::Serialize)]
pub struct Concept {
    pub name: String,
    pub severity: i64,
    pub side: Side,
    pub phrase: String,
    pub elo_min: i32,
}

/// Push one `Concept` per side that has at least one matching entry in `items`,
/// severity = count × `weight`. Covers the concepts whose shape is exactly
/// "count colored instances, weight the count" — not every concept fits this
/// (some sum a field instead of counting entries, some have an extra
/// predicate beyond color), so this is a targeted helper, not a general
/// registry; see the call sites below for which concepts use it.
#[allow(clippy::too_many_arguments)]
fn count_and_push_by_color<T>(
    concepts: &mut Vec<Concept>,
    items: &[T],
    color_of: impl Fn(&T) -> Side,
    us_color: Side,
    them_color: Side,
    name: &str,
    weight: i64,
    elo_min: i32,
    phrase: impl Fn(Side, i64) -> String,
) {
    let us_n = items.iter().filter(|t| color_of(t) == us_color).count() as i64;
    if us_n > 0 {
        concepts.push(Concept { name: name.into(), severity: us_n * weight, side: us_color, phrase: phrase(us_color, us_n), elo_min });
    }
    let them_n = items.iter().filter(|t| color_of(t) == them_color).count() as i64;
    if them_n > 0 {
        concepts.push(Concept { name: name.into(), severity: them_n * weight, side: them_color, phrase: phrase(them_color, them_n), elo_min });
    }
}

/// Extract all detected concepts from a position's typed SensorReport, ranked by severity
/// descending. `groups` is consulted only for the handful of scalar (non-tag) magnitudes —
/// `material_total.value`, `king_safety.blended`, `development.blended` — that already live
/// on typed `GroupValue` fields rather than in a `terms` string-keyed bag.
pub fn extract_concepts(sensor: &SensorReport, groups: &EvalGroups, side_to_move: Side) -> Vec<Concept> {
    let mut concepts = Vec::new();
    let (us_color, them_color) = (side_to_move, side_to_move.other());

    // --- Material (ELO 600+) ---
    let material_imbalance = groups.material_total.value;
    if material_imbalance.abs() > 50 {
        let (side, phrase) = if material_imbalance > 0 {
            (Side::White, format!("White is up {} centipawns in material", material_imbalance))
        } else {
            (Side::Black, format!("Black is up {} centipawns in material", -material_imbalance))
        };
        concepts.push(Concept { name: "material_imbalance".into(), severity: material_imbalance.abs(), side, phrase, elo_min: 600 });
    }

    // Bishop pair (ELO 1800+)
    if let Some(bal) = &sensor.material.balance {
        if bal.bishop_pair_white { concepts.push(Concept { name: "bishop_pair".into(), severity: 40, side: Side::White, phrase: "White has the bishop pair".into(), elo_min: 1800 }); }
        if bal.bishop_pair_black { concepts.push(Concept { name: "bishop_pair".into(), severity: 40, side: Side::Black, phrase: "Black has the bishop pair".into(), elo_min: 1800 }); }
    }

    // Tactical: forks (1000+), pins (1200+), skewers (1200+), discovered (1400+), hanging (600+)
    count_and_push_by_color(&mut concepts, &sensor.tactical.forks, |f| f.attacker.color, us_color, them_color,
        "fork", 80, 1000, |side, n| format!("{} has {} fork(s)", side, n));
    count_and_push_by_color(&mut concepts, &sensor.tactical.pins, |p| p.attacker.color, us_color, them_color,
        "pin", 50, 1200, |side, n| format!("{} has {} pin(s)", side, n));
    count_and_push_by_color(&mut concepts, &sensor.tactical.skewers, |s| s.attacker.color, us_color, them_color,
        "skewer", 45, 1200, |side, n| format!("{} has {} skewer(s)", side, n));
    count_and_push_by_color(&mut concepts, &sensor.tactical.discovered, |d| d.attacker.color, us_color, them_color,
        "discovered_attack", 60, 1400, |side, n| format!("{} has {} discovered attack(s)", side, n));
    // Hanging pieces — typed data existed but this concept was never emitted before this session.
    count_and_push_by_color(&mut concepts, &sensor.tactical.hanging, |h| h.piece.color, us_color, them_color,
        "hanging_piece", 60, 600, |side, n| format!("{} has {} hanging piece(s)", side, n));

    // Pawn structure (ELO 1600+): isolated pawns, keyed by the pawn's real color (not us/them)
    count_and_push_by_color(&mut concepts, &sensor.positional.isolated_pawns, |p| p.color, Side::White, Side::Black,
        "isolated_pawn", 30, 1600, |side, n| format!("{} has {} isolated pawn(s)", side, n));

    // Doubled pawns: sums each entry's own `count` field rather than counting
    // entries, so this doesn't fit count_and_push_by_color.
    let doubled_white: i64 = sensor.positional.doubled_pawns.iter().filter(|p| p.color == Side::White).map(|p| p.count as i64).sum();
    if doubled_white > 0 { concepts.push(Concept { name: "doubled_pawn".into(), severity: doubled_white * 30, side: Side::White, phrase: format!("white has {} doubled pawn(s)", doubled_white), elo_min: 1600 }); }
    let doubled_black: i64 = sensor.positional.doubled_pawns.iter().filter(|p| p.color == Side::Black).map(|p| p.count as i64).sum();
    if doubled_black > 0 { concepts.push(Concept { name: "doubled_pawn".into(), severity: doubled_black * 30, side: Side::Black, phrase: format!("black has {} doubled pawn(s)", doubled_black), elo_min: 1600 }); }

    // Pawn majority (1800+) and breaks (1800+)
    for m in &sensor.positional.pawn_majority {
        concepts.push(Concept { name: "pawn_majority".into(), severity: m.count * 20, side: m.color, phrase: format!("{} has a pawn majority", m.color), elo_min: 1800 });
    }
    count_and_push_by_color(&mut concepts, &sensor.positional.pawn_breaks, |b| b.color, us_color, them_color,
        "pawn_break", 30, 1800, |side, n| format!("{} has {} pawn break candidate(s)", side, n));

    // Minority attack (ELO 2000+)
    if let Some(m) = &sensor.positional.minority_attack {
        concepts.push(Concept { name: "minority_attack".into(), severity: m.strength * 35, side: m.color, phrase: format!("{} has a minority attack", m.color), elo_min: 2000 });
    }

    // Outposts (ELO 1600+)
    count_and_push_by_color(&mut concepts, &sensor.positional.outposts, |o| o.piece.color, us_color, them_color,
        "outpost", 40, 1600, |side, n| format!("{} has {} outpost(s)", side, n));

    // Rook activity (ELO 1400+): open_files has an extra rook_count > 0 predicate beyond
    // color, and rook_on_seventh sums a `count` field — neither fits count_and_push_by_color.
    let rook_open_us = sensor.positional.open_files.iter().filter(|f| f.color == us_color && f.rook_count > 0).count() as i64;
    if rook_open_us > 0 { concepts.push(Concept { name: "rook_open_file".into(), severity: rook_open_us * 25, side: us_color, phrase: format!("{} has rook(s) on open file(s)", us_color), elo_min: 1400 }); }
    for r in sensor.positional.rook_on_seventh.iter().filter(|r| r.color == us_color) {
        concepts.push(Concept { name: "rook_seventh".into(), severity: r.count as i64 * 30, side: us_color, phrase: format!("{} has a rook on the 7th rank", us_color), elo_min: 1400 });
    }
    for r in sensor.positional.rook_on_seventh.iter().filter(|r| r.color == them_color) {
        concepts.push(Concept { name: "rook_seventh".into(), severity: r.count as i64 * 30, side: them_color, phrase: format!("{} has a rook on the 7th rank", them_color), elo_min: 1400 });
    }

    // King safety (ELO 1000-1400+)
    if sensor.in_check {
        concepts.push(Concept { name: "king_in_check".into(), severity: 100, side: us_color, phrase: format!("{}'s king is in check!", us_color), elo_min: 1000 });
    }
    if groups.king_safety.blended.abs() > 40 {
        // king_safety.blended is us-them relative (like development below), not
        // White-relative: blended < 0 means `us` (side to move) has the less safe
        // king; > 0 means `them` does. Previously hardcoded "white"/"black" here
        // regardless of side to move — wrong whenever Black was to move.
        let (side, phrase) = if groups.king_safety.blended < 0 {
            (us_color, format!("{}'s king is exposed", us_color))
        } else { (them_color, format!("{}'s king is exposed", them_color)) };
        concepts.push(Concept { name: "king_exposed".into(), severity: groups.king_safety.blended.abs(), side, phrase, elo_min: 1400 });
    }

    // Passed pawns (ELO 1400+)
    let passed_us = sensor.positional.passed_pawns.iter().filter(|p| p.color == us_color).count() as i64;
    if passed_us > 0 { concepts.push(Concept { name: "passed_pawn".into(), severity: passed_us * 50, side: us_color, phrase: format!("{} has {} passed pawn(s)", us_color, passed_us), elo_min: 1400 }); }
    let passed_them = sensor.positional.passed_pawns.iter().filter(|p| p.color == them_color).count() as i64;
    if passed_them > 0 { concepts.push(Concept { name: "passed_pawn".into(), severity: passed_them * 50, side: them_color, phrase: format!("{} has {} passed pawn(s)", them_color, passed_them), elo_min: 1400 }); }

    // Development (ELO 1400+)
    // development.blended > 0 means `us` (side to move) has the advantage
    if groups.development.blended.abs() > 20 {
        let (side, phrase) = if groups.development.blended > 0 {
            (us_color, format!("{} has a development advantage", us_color))
        } else { (them_color, format!("{} has a development advantage", them_color)) };
        concepts.push(Concept { name: "development".into(), severity: groups.development.blended.abs(), side, phrase, elo_min: 1400 });
    }

    // Center control (ELO 1800+)
    if let Some(cc) = &sensor.positional.center_control {
        concepts.push(Concept { name: "center_control".into(), severity: cc.strength, side: cc.color, phrase: format!("{} controls the center", cc.color), elo_min: 1800 });
    }

    concepts.sort_by_key(|c| std::cmp::Reverse(c.severity));
    concepts
}

/// Filter concepts visible to a player at the given ELO.
pub fn concepts_for_elo(concepts: &[Concept], elo: i32) -> Vec<&Concept> {
    concepts.iter().filter(|c| c.elo_min <= elo).collect()
}

/// Gate concepts for a single position (no delta history).
/// Ranks by severity × elo_relevance × confidence. Returns top 1-3.
pub fn rank_issues_for_position(concepts: &[Concept], player_elo: i32) -> Vec<GatedIssue> {
    let mut issues: Vec<GatedIssue> = concepts.iter().filter_map(|c| {
        let elo_relevance = if c.elo_min <= player_elo { 1.0 }
            else { 0.5f64.powf((c.elo_min - player_elo) as f64 / 200.0) };
        let confidence = match c.name.as_str() {
            "fork"|"pin"|"skewer"|"discovered_attack"|"king_in_check" => 1.0,
            "hanging_piece"|"passed_pawn"|"material_imbalance" => 0.9,
            "rook_open_file"|"rook_seventh"|"outpost"|"development" => 0.8,
            "king_exposed"|"isolated_pawn"|"doubled_pawn" => 0.7,
            _ => 0.6,
        };
        let score = c.severity as f64 * elo_relevance * confidence;
        if score < 1.0 { return None; }
        Some(GatedIssue { name: c.name.clone(), severity: c.severity, elo_min: c.elo_min,
            magnitude: 1.0, elo_relevance, confidence, score,
            phrase: c.phrase.clone(), side: c.side })
    }).collect();
    issues.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let has_critical = issues.iter().any(|i| i.severity >= 80 && i.elo_min <= 1000 && i.score > 10.0);
    if has_critical { issues.retain(|i| i.elo_min <= 1200); }
    issues.truncate(3);
    issues
}

/// Gate concepts for a player based on evaluation deltas between plies.
/// Returns top 1-3 issues. If a critical low-ELO issue exists, suppresses higher-level coaching.
pub fn rank_issues_for_player(
    concepts: &[Concept],
    player_elo: i32,
    deltas: &std::collections::HashMap<String, f64>,
) -> Vec<GatedIssue> {
    let mut issues: Vec<GatedIssue> = concepts.iter().filter_map(|c| {
        let magnitude = deltas.get(&c.name).copied().unwrap_or(0.0);
        if magnitude < 1.0 { return None; }
        let elo_relevance = if c.elo_min <= player_elo { 1.0 }
            else { 0.5f64.powf((c.elo_min - player_elo) as f64 / 200.0) };
        let confidence = match c.name.as_str() {
            "fork"|"pin"|"skewer"|"discovered_attack"|"king_in_check" => 1.0,
            "hanging_piece"|"passed_pawn"|"material_imbalance" => 0.9,
            "rook_open_file"|"rook_seventh"|"outpost"|"development" => 0.8,
            "king_exposed"|"isolated_pawn"|"doubled_pawn" => 0.7,
            _ => 0.6,
        };
        let score = magnitude * c.severity as f64 * elo_relevance * confidence;
        Some(GatedIssue { name: c.name.clone(), severity: c.severity, elo_min: c.elo_min,
            magnitude, elo_relevance, confidence, score, phrase: c.phrase.clone(), side: c.side })
    }).collect();
    issues.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let has_critical = issues.iter().any(|i| i.severity >= 80 && i.elo_min <= 1000 && i.score > 10.0);
    if has_critical { issues.retain(|i| i.elo_min <= 1200); }
    issues.truncate(3);
    issues
}

// ── Markov StateVector: compact position encoding for transition tracking ──

/// Compact state ID for a chess position. Deterministic — same position → same state.
/// Encodes phase, material balance, tactical flags, and positional features into ~13 bits.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct StateVector {
    pub state_id: u16,
    pub phase: u8,
    pub material_sign: i8,
    pub king_exposed: bool,
    pub in_check: bool,
    pub has_fork: bool,
    pub has_pin: bool,
    pub has_hanging: bool,
    pub has_outpost: bool,
    pub open_file: bool,
    pub has_passed_pawn: bool,
    pub has_skewer: bool,
    pub has_discovered: bool,
}

/// Encode a position into a compact state ID from the sensor report and groups.
// Bit layout for StateVector::state_id — the single definition shared by
// encode_state (pack) and decode_state_id (unpack) below, so a future change
// to the layout can't silently desync a hand-rolled decoder elsewhere in the
// crate (coach_derive_cmd.rs previously had exactly that: its own inline
// copy of these shifts, agreeing with this by coincidence, not by sharing).
const BIT_PHASE: u32 = 0;         // bits 0-1 (2 bits)
const BIT_MATERIAL_SIGN: u32 = 2; // bits 2-4 (3 bits)
const BIT_KING_EXPOSED: u32 = 5;
const BIT_IN_CHECK: u32 = 6;
const BIT_FORK: u32 = 7;
const BIT_PIN: u32 = 8;
const BIT_HANGING: u32 = 9;
const BIT_OUTPOST: u32 = 10;
const BIT_OPEN_FILE: u32 = 11;
const BIT_PASSED_PAWN: u32 = 12;
const BIT_SKEWER: u32 = 13;
const BIT_DISCOVERED: u32 = 14;

/// One row per boolean bit: (bit position, predicate over the sensor report).
/// Adding a new flag to `StateVector` means adding one row here and one field
/// to the struct — `encode_state` and `decode_state_id` don't otherwise need
/// to change. Checks aren't all textually identical (`BIT_FORK`'s is an OR of
/// two conditions, `BIT_KING_EXPOSED`'s maps through an `Option`) — that's
/// fine, each closure can differ; what matters is that bit position and check
/// live in the same tuple, so they can't end up paired with the wrong bit the
/// way two separately-maintained lists could.
type SensorPredicate = fn(&SensorReport) -> bool;
const BOOL_BITS: &[(u32, SensorPredicate)] = &[
    (BIT_KING_EXPOSED, |s| s.positional.king_exposure.as_ref().map(|k| k.attacker_count > 0).unwrap_or(false)),
    (BIT_IN_CHECK,     |s| s.in_check),
    (BIT_FORK,         |s| !s.tactical.forks.is_empty() || !s.evaluated_forks.is_empty()),
    (BIT_PIN,          |s| !s.tactical.pins.is_empty()),
    (BIT_HANGING,      |s| !s.tactical.hanging.is_empty()),
    (BIT_OUTPOST,      |s| !s.positional.outposts.is_empty()),
    (BIT_OPEN_FILE,    |s| !s.positional.open_files.is_empty()),
    (BIT_PASSED_PAWN,  |s| !s.positional.passed_pawns.is_empty()),
    (BIT_SKEWER,       |s| !s.tactical.skewers.is_empty()),
    (BIT_DISCOVERED,   |s| !s.tactical.discovered.is_empty()),
];

pub fn encode_state(sensor: &SensorReport, groups: &EvalGroups, phase: u8) -> StateVector {
    let phase_bits = match phase {
        0..=8 => 0u8,      // deep endgame
        9..=16 => 1,       // endgame
        17..=24 => 2,      // midgame
        _ => 3,            // opening
    };

    let mat = groups.material_total.value;
    let material_sign: i8 = if mat > 300 { 2 } else if mat > 100 { 1 }
        else if mat < -300 { -2 } else if mat < -100 { -1 } else { 0 };

    // Pack into u16 bitfield
    let mut id: u16 = 0;
    id |= (phase_bits as u16 & 0x3) << BIT_PHASE;
    id |= ((material_sign + 2) as u16 & 0x7) << BIT_MATERIAL_SIGN;
    for &(bit, check) in BOOL_BITS {
        if check(sensor) { id |= 1 << bit; }
    }

    // Build the named fields by decoding what was just packed — packer and
    // decoder are mutually verifying by construction this way: if
    // decode_state_id ever disagreed with the bits above, encode_state's own
    // output would immediately look wrong too.
    decode_state_id(id)
}

/// Unpack a previously-encoded `state_id` back into a `StateVector`. Shares
/// the `BIT_*` constants above with `encode_state`, so this and the packer
/// can never disagree about the layout — unlike the ad hoc bit-shifting that
/// used to live independently in `coach_derive_cmd.rs`'s "fast path".
pub fn decode_state_id(sid: u16) -> StateVector {
    let bit = |b: u32| (sid >> b) & 1 != 0;
    StateVector {
        state_id: sid,
        phase: (sid >> BIT_PHASE) as u8 & 0x3,
        material_sign: (((sid >> BIT_MATERIAL_SIGN) & 0x7) as i8) - 2,
        king_exposed: bit(BIT_KING_EXPOSED),
        in_check: bit(BIT_IN_CHECK),
        has_fork: bit(BIT_FORK),
        has_pin: bit(BIT_PIN),
        has_hanging: bit(BIT_HANGING),
        has_outpost: bit(BIT_OUTPOST),
        open_file: bit(BIT_OPEN_FILE),
        has_passed_pawn: bit(BIT_PASSED_PAWN),
        has_skewer: bit(BIT_SKEWER),
        has_discovered: bit(BIT_DISCOVERED),
    }
}

// ── Elo Sensor Taxonomy: Convergence Gate ──

/// Sensor tier classification for the attenuation matrix.
/// Each tier has a fundamentally different mathematical behavior:
/// - Survival/Threat: digital switches (on/off), always active
/// - Positional: hybrid, partially attenuated by tactical chaos
/// - Strategic: analog dials, fully suppressed in unstable positions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SensorTier {
    Survival,    // 600-1000: material imbalance, checks, hanging pieces
    Threat,      // 1000-1400: forks, pins, skewers, discovered attacks
    Positional,  // 1400-1800: outposts, open files, pawn structure
    Strategic,   // 1800-2000+: minority attack, pawn majority, bishop pair
}

/// Attenuation factor per tier given the chaos coefficient (0.0 = clean, 1.0 = chaotic).
/// Survival/Threat sensors are digital switches — always active regardless of chaos.
/// Positional sensors are dampened at 50% of chaos.
/// Strategic sensors are fully suppressed proportional to chaos.
pub fn attenuation(tier: SensorTier, chaos: f64) -> f64 {
    match tier {
        SensorTier::Survival => 1.0,
        SensorTier::Threat => 1.0,
        SensorTier::Positional => 1.0 - chaos * 0.5,
        SensorTier::Strategic => (1.0 - chaos).max(0.0),
    }
}
