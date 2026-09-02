use anyhow::{Context, Result};
use serde::Serialize;
use shakmaty::{attacks, fen::Fen, Bitboard, Chess, Color, File, Position, Rank, Role, Square};

use crate::eval::concept_types::*;
use crate::eval::sensor::{TacticalReport, PositionalReport, SensorReport, AggregatedScores, MaterialConceptReport};
use crate::eval::concepts::{encode_state, extract_concepts, rank_issues_for_position};
use crate::eval::threat_graph::{king_ring, role_name, ThreatGraph};
use crate::canonical::{normalize_to_white_to_move, unflip_square_str};

// Configurable constants (GUESS values) collected here for easier tuning.
const TACTICAL_BASE_PINS: i64 = 50;
const TACTICAL_BASE_FORKS: i64 = 80;
const TACTICAL_BASE_SKEWERS: i64 = 40;
const TACTICAL_BASE_DISC: i64 = 60;
const PHASE_FACTOR_DEN: i64 = 40;

const ROOK_OPEN_FILE_BONUS: i64 = 25;
const DOUBLED_ROOK_BONUS: i64 = 20;
const ROOK_SEVENTH_BONUS: i64 = 30;

const OUTPOST_WEIGHT: i64 = 40;

// Tropism piece weights
const TROPISM_QUEEN: i64 = 90;
const TROPISM_ROOK: i64 = 50;
const TROPISM_BISHOP: i64 = 35;
const TROPISM_KNIGHT: i64 = 30;
const TROPISM_PAWN: i64 = 10;

// Piece values used for fork/skewer heuristics
const VAL_QUEEN: i64 = 900;
const VAL_ROOK: i64 = 500;
const VAL_BISHOP: i64 = 330;
const VAL_KNIGHT: i64 = 320;
const VAL_PAWN: i64 = 100;

// Pawn-structure default weights
const PAWN_MAJORITY_WEIGHT: i64 = 20;
const PAWN_BREAK_WEIGHT: i64 = 30;
const MINORITY_ATTACK_WEIGHT: i64 = 35;

// Mobility weight (per-square)
const PIECE_MOBILITY_WEIGHT: i64 = 5;

use once_cell::sync::Lazy;
use std::sync::RwLock;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Weights {
    pub tactical_base_pins: i64,
    pub tactical_base_forks: i64,
    pub tactical_base_skewers: i64,
    pub tactical_base_disc: i64,
    pub phase_factor_den: i64,
    pub rook_open_file_bonus: i64,
    pub doubled_rook_bonus: i64,
    pub rook_seventh_bonus: i64,
    pub outpost_weight: i64,
    pub tropism_queen: i64,
    pub tropism_rook: i64,
    pub tropism_bishop: i64,
    pub tropism_knight: i64,
    pub tropism_pawn: i64,
    pub val_queen: i64,
    pub val_rook: i64,
    pub val_bishop: i64,
    pub val_knight: i64,
    pub val_pawn: i64,
    pub pawn_majority_weight: i64,
    pub pawn_break_weight: i64,
    pub minority_attack_weight: i64,
    pub piece_mobility_weight: i64,
    pub phase_bias_material: i64,
    pub phase_bias_pawn_structure: i64,
    pub phase_bias_piece_activity: i64,
    pub phase_bias_king_safety: i64,
    pub phase_bias_passed_pawns: i64,
    pub phase_bias_development: i64,
    pub phase_bias_vector_features: i64,
    pub phase_bias_strategic: i64,
}

impl Default for Weights {
    fn default() -> Self {
        Weights {
            tactical_base_pins: TACTICAL_BASE_PINS,
            tactical_base_forks: TACTICAL_BASE_FORKS,
            tactical_base_skewers: TACTICAL_BASE_SKEWERS,
            tactical_base_disc: TACTICAL_BASE_DISC,
            phase_factor_den: PHASE_FACTOR_DEN,
            rook_open_file_bonus: ROOK_OPEN_FILE_BONUS,
            doubled_rook_bonus: DOUBLED_ROOK_BONUS,
            rook_seventh_bonus: ROOK_SEVENTH_BONUS,
            outpost_weight: OUTPOST_WEIGHT,
            tropism_queen: TROPISM_QUEEN,
            tropism_rook: TROPISM_ROOK,
            tropism_bishop: TROPISM_BISHOP,
            tropism_knight: TROPISM_KNIGHT,
            tropism_pawn: TROPISM_PAWN,
            val_queen: VAL_QUEEN,
            val_rook: VAL_ROOK,
            val_bishop: VAL_BISHOP,
            val_knight: VAL_KNIGHT,
            val_pawn: VAL_PAWN,
            pawn_majority_weight: PAWN_MAJORITY_WEIGHT,
            pawn_break_weight: PAWN_BREAK_WEIGHT,
            minority_attack_weight: MINORITY_ATTACK_WEIGHT,
            piece_mobility_weight: PIECE_MOBILITY_WEIGHT,
            phase_bias_material: 0,
            phase_bias_pawn_structure: 0,
            phase_bias_piece_activity: 0,
            phase_bias_king_safety: 0,
            phase_bias_passed_pawns: 0,
            phase_bias_development: 0,
            phase_bias_vector_features: 0,
            phase_bias_strategic: 0,
        }
    }
}

#[derive(Deserialize)]
struct PartialWeights {
    tactical_base_pins: Option<i64>,
    tactical_base_forks: Option<i64>,
    tactical_base_skewers: Option<i64>,
    tactical_base_disc: Option<i64>,
    phase_factor_den: Option<i64>,
    rook_open_file_bonus: Option<i64>,
    doubled_rook_bonus: Option<i64>,
    rook_seventh_bonus: Option<i64>,
    outpost_weight: Option<i64>,
    tropism_queen: Option<i64>,
    tropism_rook: Option<i64>,
    tropism_bishop: Option<i64>,
    tropism_knight: Option<i64>,
    tropism_pawn: Option<i64>,
    val_queen: Option<i64>,
    val_rook: Option<i64>,
    val_bishop: Option<i64>,
    val_knight: Option<i64>,
    val_pawn: Option<i64>,
    pawn_majority_weight: Option<i64>,
    pawn_break_weight: Option<i64>,
    minority_attack_weight: Option<i64>,
    piece_mobility_weight: Option<i64>,
    phase_bias_material: Option<i64>,
    phase_bias_pawn_structure: Option<i64>,
    phase_bias_piece_activity: Option<i64>,
    phase_bias_king_safety: Option<i64>,
    phase_bias_passed_pawns: Option<i64>,
    phase_bias_development: Option<i64>,
    phase_bias_vector_features: Option<i64>,
    phase_bias_strategic: Option<i64>,
}

static WEIGHTS: Lazy<RwLock<Weights>> = Lazy::new(|| RwLock::new(Weights::default()));

fn weights() -> Weights {
    WEIGHTS.read().expect("weights lock").clone()
}

/// Load weights from a JSON file and override defaults. Keys match struct field names.
pub fn set_weights_from_file(path: &str) -> Result<(), String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("could not read weights file: {}", e))?;
    let p: PartialWeights = serde_json::from_str(&s).map_err(|e| format!("could not parse weights JSON: {}", e))?;
    let mut w = WEIGHTS.write().map_err(|e| format!("lock error: {:?}", e))?;
    if let Some(v) = p.tactical_base_pins { w.tactical_base_pins = v }
    if let Some(v) = p.tactical_base_forks { w.tactical_base_forks = v }
    if let Some(v) = p.tactical_base_skewers { w.tactical_base_skewers = v }
    if let Some(v) = p.tactical_base_disc { w.tactical_base_disc = v }
    if let Some(v) = p.phase_factor_den { w.phase_factor_den = v }
    if let Some(v) = p.rook_open_file_bonus { w.rook_open_file_bonus = v }
    if let Some(v) = p.doubled_rook_bonus { w.doubled_rook_bonus = v }
    if let Some(v) = p.rook_seventh_bonus { w.rook_seventh_bonus = v }
    if let Some(v) = p.outpost_weight { w.outpost_weight = v }
    if let Some(v) = p.tropism_queen { w.tropism_queen = v }
    if let Some(v) = p.tropism_rook { w.tropism_rook = v }
    if let Some(v) = p.tropism_bishop { w.tropism_bishop = v }
    if let Some(v) = p.tropism_knight { w.tropism_knight = v }
    if let Some(v) = p.tropism_pawn { w.tropism_pawn = v }
    if let Some(v) = p.val_queen { w.val_queen = v }
    if let Some(v) = p.val_rook { w.val_rook = v }
    if let Some(v) = p.val_bishop { w.val_bishop = v }
    if let Some(v) = p.val_knight { w.val_knight = v }
    if let Some(v) = p.val_pawn { w.val_pawn = v }
    if let Some(v) = p.pawn_majority_weight { w.pawn_majority_weight = v }
    if let Some(v) = p.pawn_break_weight { w.pawn_break_weight = v }
    if let Some(v) = p.minority_attack_weight { w.minority_attack_weight = v }
    if let Some(v) = p.piece_mobility_weight { w.piece_mobility_weight = v }
    if let Some(v) = p.phase_bias_material { w.phase_bias_material = v }
    if let Some(v) = p.phase_bias_pawn_structure { w.phase_bias_pawn_structure = v }
    if let Some(v) = p.phase_bias_piece_activity { w.phase_bias_piece_activity = v }
    if let Some(v) = p.phase_bias_king_safety { w.phase_bias_king_safety = v }
    if let Some(v) = p.phase_bias_passed_pawns { w.phase_bias_passed_pawns = v }
    if let Some(v) = p.phase_bias_development { w.phase_bias_development = v }
    if let Some(v) = p.phase_bias_vector_features { w.phase_bias_vector_features = v }
    if let Some(v) = p.phase_bias_strategic { w.phase_bias_strategic = v }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct PositionRecord {
    pub fen: String,
    pub normalized_fen: String,
    pub side_to_move: Side,
    pub phase: u8,
    /// `us − them` where `us = chess.turn()` — relative to whoever is
    /// actually to move in *this* position, not to White. This is the
    /// convention every scoring function in this module already uses (see
    /// `normalize_for_eval`'s doc comment), and it stays that way here: it's
    /// what every internal consumer wants, and it mirrors the DB's canonical
    /// (White-always-to-move) position-identity simplification.
    ///
    /// Comparing this value across two positions with different sides to
    /// move requires knowing whose turn each one is — `side_to_move` above
    /// carries exactly that, in one line: `if side_to_move == White {
    /// final_score } else { -final_score }`. A `final_score_white_relative`
    /// convenience field used to precompute that flip server-side; removed
    /// (FINDINGS.md, 2026-09-01) as part of a broader audit that found this
    /// crate had accumulated several *different* flip conventions living
    /// side by side (mover-relative scores, a White-relative score,
    /// mover-relative `Concept`/`GatedIssue` tags that were nonetheless
    /// unflipped back to real color for output, real-color `PieceRef`
    /// squares). One convention — mover-relative, plus `side_to_move` for
    /// whoever wants to translate it — is enough; a client that needs
    /// White-relative can compute it as easily as this field's own
    /// implementation did.
    pub final_score: i64,
    pub engine_score: Option<i64>,
    pub legal: LegalInfo,
    pub groups: EvalGroups,
    pub checks: Checks,
    pub sensor_report: SensorReport,
}

#[derive(Debug, Serialize)]
pub struct LegalInfo {
    pub is_legal: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub is_stalemate: bool,
    pub is_insufficient_material: bool,
    pub legal_move_count: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct GroupValue {
    pub mg: i64,
    pub eg: i64,
    pub blended: i64,
    pub terms: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Default)]
pub struct ScalarValue {
    pub value: i64,
    pub factor: i64,
}

/// Raw detector output from tactical_score — Square-level examples threaded through
/// EvalGroups so build_sensor_report can build typed structs without re-running detectors.
#[derive(Debug, Default)]
pub struct TacticalRaw {
    pub pin_ex_us:    Vec<(Square, Square, Square)>,
    pub pin_ex_them:  Vec<(Square, Square, Square)>,
    pub skew_ex_us:   Vec<(Square, Square, Square)>,
    pub skew_ex_them: Vec<(Square, Square, Square)>,
    pub disc_ex_us:   Vec<(Square, Square, Square)>,
    pub disc_ex_them: Vec<(Square, Square, Square)>,
}

#[derive(Debug, Serialize, Default)]
pub struct EvalGroups {
    pub material: GroupValue,
    pub pawn_structure: GroupValue,
    pub piece_activity: GroupValue,
    pub king_safety: GroupValue,
    pub passed_pawns: GroupValue,
    pub development: GroupValue,
    pub vector_features: GroupValue,
    pub strategic: GroupValue,
    pub tactical: GroupValue,
    pub scaling: ScalarValue,
    pub drawishness: ScalarValue,
    pub override_: ScalarValue,
    pub material_total: ScalarValue,
    pub positional_total: ScalarValue,
    pub tactical_total: ScalarValue,
    /// Cached raw detector results — skipped during serialization.
    #[serde(skip)]
    pub tactical_raw: TacticalRaw,
}

#[derive(Debug, Serialize, Default)]
pub struct Checks {
    pub sum_groups: i64,
    pub matches_final: bool,
    pub delta: Option<i64>,
}

fn piece_count(board: &shakmaty::Board, color: Color, role: Role) -> i64 {
    (board.by_color(color) & board.by_role(role)).count() as i64
}

fn bitboard_count(bb: Bitboard) -> i64 {
    bb.count() as i64
}

fn biased_phase(phase: u8, bias: i64) -> u8 {
    ((phase as i64 + bias).clamp(0, 32)) as u8
}

fn phase_split(value: i64, phase: u8) -> (i64, i64) {
    let bias = (i64::from(phase).saturating_sub(16).abs() * value.abs()) / 64;
    (value + bias, value - bias)
}

/// Centralized phase blending: Critter's scale() function.
///   blended = (mg * phase + eg * (32 - phase)) / 32
/// At phase 32 (opening): blended ≈ mg (positional)
/// At phase 0  (endgame): blended ≈ eg (material)
fn blend(mg: i64, eg: i64, phase: u8) -> i64 {
    let p = phase as i64;
    (mg * p + eg * (32 - p)) / 32
}

fn count_on_home(board: &shakmaty::Board, color: Color, role: Role, home: Bitboard) -> i64 {
    (board.by_color(color) & board.by_role(role) & home).count() as i64
}

fn pawn_attack_mask(board: &shakmaty::Board, color: Color) -> Bitboard {
    let pawns = board.by_color(color) & board.by_role(Role::Pawn);
    let mut atk = Bitboard::EMPTY;
    for sq in pawns {
        atk |= attacks::pawn_attacks(color, sq);
    }
    atk
}

pub fn compute_phase(board: &shakmaty::Board) -> u8 {
    let white_minor = piece_count(board, Color::White, Role::Knight)
        + piece_count(board, Color::White, Role::Bishop);
    let black_minor = piece_count(board, Color::Black, Role::Knight)
        + piece_count(board, Color::Black, Role::Bishop);
    let white_major = 2 * piece_count(board, Color::White, Role::Rook)
        + 4 * piece_count(board, Color::White, Role::Queen);
    let black_major = 2 * piece_count(board, Color::Black, Role::Rook)
        + 4 * piece_count(board, Color::Black, Role::Queen);
    (white_minor + black_minor + white_major + black_major).min(32) as u8
}

fn material_score(board: &shakmaty::Board, us: Color, phase: u8) -> GroupValue {
    let them = us.other();
    // Phase-dependent material adjustment coefficients, indexed by game phase (0..=32).
    // Each row: [unused0, unused1, unused2, unused3, unused4, bishop_pair, np_bonus, rp_penalty,
    //            bn_vs_rp, redundant_r, redundant_qr]
    // Material table ported from Critter 1.6a (battle-tested engine values).
    // Columns: Q_val, R_val, B_val, N_val, P_val, bishop_pair, np_bonus,
    //          rp_penalty, bn_vs_rp, redundant_r, redundant_qr
    let coeff = [
        [3004, 1533, 910, 875, 298, 118, 13,  0, 13, 82, 41],
        [2964, 1515, 899, 864, 293, 117, 12, 1, 14, 81, 40],
        [2923, 1496, 888, 854, 289, 116, 12, 1, 16, 79, 40],
        [2882, 1477, 877, 843, 284, 114, 12, 2, 18, 78, 39],
        [2841, 1458, 866, 832, 280, 113, 12, 3, 19, 77, 38],
        [2800, 1439, 855, 821, 275, 112, 11, 3, 21, 76, 38],
        [2759, 1420, 844, 811, 271, 110, 11, 4, 22, 74, 37],
        [2719, 1402, 833, 800, 266, 109, 11, 4, 24, 73, 36],
        [2678, 1383, 822, 789, 262, 108, 10, 5, 26, 72, 36],
        [2653, 1367, 817, 783, 259, 106, 10, 5, 26, 70, 35],
        [2629, 1351, 812, 777, 256, 105, 10, 6, 27, 69, 35],
        [2604, 1336, 808, 770, 253, 103, 9, 6, 28, 68, 34],
        [2580, 1320, 803, 764, 250, 102, 9, 6, 29, 67, 33],
        [2555, 1304, 798, 758, 247, 101, 9, 7, 30, 65, 33],
        [2531, 1288, 793, 752, 244, 99, 8, 7, 30, 64, 32],
        [2506, 1273, 789, 746, 241, 98, 8, 7, 31, 63, 31],
        [2482, 1257, 784, 740, 238, 97, 8, 8, 32, 61, 31],
        [2457, 1241, 779, 733, 235, 95, 7, 8, 33, 60, 30],
        [2433, 1226, 774, 727, 232, 94, 7, 8, 34, 59, 29],
        [2408, 1210, 770, 721, 229, 93, 7, 9, 34, 58, 29],
        [2384, 1194, 765, 715, 226, 91, 6, 9, 35, 56, 28],
        [2359, 1178, 760, 709, 223, 90, 6, 9, 36, 55, 28],
        [2335, 1163, 755, 703, 220, 89, 6, 10, 37, 54, 27],
        [2310, 1147, 751, 696, 217, 87, 5, 10, 38, 52, 26],
        [2286, 1131, 746, 690, 214, 86, 5, 10, 38, 51, 26],
        [2261, 1117, 741, 686, 211, 85, 4, 11, 40, 50, 25],
        [2237, 1103, 736, 681, 208, 83, 4, 11, 42, 49, 24],
        [2212, 1089, 732, 676, 205, 82, 3, 11, 43, 47, 24],
        [2188, 1075, 727, 672, 202, 81, 3, 12, 45, 46, 23],
        [2163, 1061, 722, 667, 199, 79, 2, 12, 46, 45, 22],
        [2139, 1046, 717, 663, 196, 78, 1, 12, 48, 44, 22],
        [2115, 1032, 713, 658, 193, 77, 1, 12, 50, 42, 21],
        [2090, 1018, 708, 653, 190, 75, 0, 13, 51, 41, 20],
    ][phase as usize];

    // `ours`/`theirs` — not `white`/`black` — because material_score's contract
    // is "return the us-relative score," and every sibling function in
    // compute_groups (pawn_structure_score, king_safety_score,
    // development_score) already takes `us: Color` for exactly this reason.
    // Only correct because `board` is always canonical (White-to-move) at
    // every current call site — see compute_groups's own doc comment — but
    // expressing it as us/them means that invariant no longer has to hold
    // for this function's *output* to still mean the right thing.
    let ours   = |role: Role, value: i64| piece_count(board, us, role) * value;
    let theirs = |role: Role, value: i64| piece_count(board, them, role) * value;

    // mg = opening (phase 32) — positional play dominates, pieces worth less
    let mg = (ours(Role::Queen, 2090)
        + ours(Role::Rook, 1018)
        + ours(Role::Bishop, 708)
        + ours(Role::Knight, 653)
        + ours(Role::Pawn, 190))
        - (theirs(Role::Queen, 2090)
            + theirs(Role::Rook, 1018)
            + theirs(Role::Bishop, 708)
            + theirs(Role::Knight, 653)
            + theirs(Role::Pawn, 190));

    // eg = endgame (phase 0) — material is decisive, pieces worth more
    let eg = (ours(Role::Queen, 3004)
        + ours(Role::Rook, 1533)
        + ours(Role::Bishop, 910)
        + ours(Role::Knight, 875)
        + ours(Role::Pawn, 298))
        - (theirs(Role::Queen, 3004)
            + theirs(Role::Rook, 1533)
            + theirs(Role::Bishop, 910)
            + theirs(Role::Knight, 875)
            + theirs(Role::Pawn, 298));

    let bishop_pair = if piece_count(board, us, Role::Bishop) >= 2 {
        coeff[5]
    } else {
        0
    } - if piece_count(board, them, Role::Bishop) >= 2 {
        coeff[5]
    } else {
        0
    };

    let rp_penalty = -((piece_count(board, us, Role::Pawn) - 5)
        * piece_count(board, us, Role::Rook)
        * coeff[7])
        + ((piece_count(board, them, Role::Pawn) - 5)
            * piece_count(board, them, Role::Rook)
            * coeff[7]);

    let np_bonus = ((piece_count(board, us, Role::Pawn) - 5)
        * piece_count(board, us, Role::Knight)
        * coeff[6])
        - ((piece_count(board, them, Role::Pawn) - 5)
            * piece_count(board, them, Role::Knight)
            * coeff[6]);

    let bn_vs_rp = if (piece_count(board, us, Role::Knight)
        + piece_count(board, us, Role::Bishop))
        != (piece_count(board, them, Role::Knight)
            + piece_count(board, them, Role::Bishop))
    {
        if (piece_count(board, us, Role::Knight)
            + piece_count(board, us, Role::Bishop))
            > (piece_count(board, them, Role::Knight)
                + piece_count(board, them, Role::Bishop))
        {
            coeff[8]
        } else {
            -coeff[8]
        }
    } else {
        0
    };

    let redundant_r = -if piece_count(board, us, Role::Rook) >= 2 {
        coeff[9]
    } else {
        0
    } + if piece_count(board, them, Role::Rook) >= 2 {
        coeff[9]
    } else {
        0
    };

    let redundant_qr = -if piece_count(board, us, Role::Queen)
        + piece_count(board, us, Role::Rook)
        >= 2
    {
        coeff[10]
    } else {
        0
    } + if piece_count(board, them, Role::Queen)
        + piece_count(board, them, Role::Rook)
        >= 2
    {
        coeff[10]
    } else {
        0
    };

    let adjustments = bishop_pair + rp_penalty + np_bonus + bn_vs_rp + redundant_r + redundant_qr;
    let blended = blend(mg, eg, phase) + adjustments;

    let mut terms = serde_json::Map::new();
    terms.insert(
        "white_queens".into(),
        serde_json::Value::from(piece_count(board, Color::White, Role::Queen)),
    );
    terms.insert(
        "black_queens".into(),
        serde_json::Value::from(piece_count(board, Color::Black, Role::Queen)),
    );
    terms.insert(
        "white_rooks".into(),
        serde_json::Value::from(piece_count(board, Color::White, Role::Rook)),
    );
    terms.insert(
        "black_rooks".into(),
        serde_json::Value::from(piece_count(board, Color::Black, Role::Rook)),
    );
    terms.insert(
        "white_bishops".into(),
        serde_json::Value::from(piece_count(board, Color::White, Role::Bishop)),
    );
    terms.insert(
        "black_bishops".into(),
        serde_json::Value::from(piece_count(board, Color::Black, Role::Bishop)),
    );
    terms.insert(
        "white_knights".into(),
        serde_json::Value::from(piece_count(board, Color::White, Role::Knight)),
    );
    terms.insert(
        "black_knights".into(),
        serde_json::Value::from(piece_count(board, Color::Black, Role::Knight)),
    );
    terms.insert(
        "white_pawns".into(),
        serde_json::Value::from(piece_count(board, Color::White, Role::Pawn)),
    );
    terms.insert(
        "black_pawns".into(),
        serde_json::Value::from(piece_count(board, Color::Black, Role::Pawn)),
    );
    terms.insert("bishop_pair".into(), serde_json::Value::from(bishop_pair));
    terms.insert("rp_penalty".into(), serde_json::Value::from(rp_penalty));
    terms.insert("np_bonus".into(), serde_json::Value::from(np_bonus));
    terms.insert("bn_vs_rp".into(), serde_json::Value::from(bn_vs_rp));
    terms.insert("redundant_r".into(), serde_json::Value::from(redundant_r));
    terms.insert("redundant_qr".into(), serde_json::Value::from(redundant_qr));
    terms.insert("adjustments".into(), serde_json::Value::from(adjustments));

    GroupValue { mg, eg, blended, terms }
}

fn count_undeveloped(board: &shakmaty::Board, color: Color) -> i64 {
    let knight_home = color.fold_wb(
        Bitboard::from(Square::B1) | Bitboard::from(Square::G1),
        Bitboard::from(Square::B8) | Bitboard::from(Square::G8),
    );
    let bishop_home = color.fold_wb(
        Bitboard::from(Square::C1) | Bitboard::from(Square::F1),
        Bitboard::from(Square::C8) | Bitboard::from(Square::F8),
    );
    count_on_home(board, color, Role::Knight, knight_home)
        + count_on_home(board, color, Role::Bishop, bishop_home)
}

fn passed_pawn_mask(board: &shakmaty::Board, color: Color) -> Bitboard {
    let pawns = board.by_color(color) & board.by_role(Role::Pawn);
    let opp_pawns = board.by_color(color.other()) & board.by_role(Role::Pawn);
    let mut passed = Bitboard::EMPTY;
    for sq in pawns {
        let mut front_span = in_front(color, sq);
        if let Some(f) = sq.file().offset(-1) {
            front_span |= in_front(color, Square::from_coords(f, sq.rank()));
        }
        if let Some(f) = sq.file().offset(1) {
            front_span |= in_front(color, Square::from_coords(f, sq.rank()));
        }
        if (opp_pawns & front_span) == Bitboard::EMPTY {
            passed |= Bitboard::from(sq);
        }
    }
    passed
}

fn pawn_structure_score(
    board: &shakmaty::Board,
    color: Color,
    phase: u8,
) -> (i64, serde_json::Map<String, serde_json::Value>) {
    let own = board.by_color(color) & board.by_role(Role::Pawn);
    let opp = board.by_color(color.other()) & board.by_role(Role::Pawn);
    let mut score = 0;
    let mut isolated = 0;
    let mut doubled = 0;
    let mut candidate = 0;
    let mut weak = 0;
    let mut passed = 0;
    let mut chain = 0;
    let mut files = [false; 8];
    let step = if color.is_white() { 1 } else { -1 };

    let passed_bb = passed_pawn_mask(board, color);

    for sq in own {
        let file = sq.file();
        let rank = sq.rank();
        let idx = usize::from(u8::from(file));
        files[idx] = true;

        let file_bb = Bitboard::from(file);
        let adjacent_own = [file.offset(-1), file.offset(1)]
            .into_iter()
            .flatten()
            .map(Bitboard::from)
            .fold(Bitboard::EMPTY, |acc, bb| acc | (own & bb));
        let same_file_others = (own & file_bb) ^ Bitboard::from(sq);
        let open_file = (own | opp) & in_front(color, sq) == Bitboard::EMPTY;

        if adjacent_own == Bitboard::EMPTY {
            isolated += 1;
            score -= if open_file {
                if phase >= 16 {
                    28
                } else {
                    36
                }
            } else if phase >= 16 {
                20
            } else {
                28
            };
        }

        if same_file_others != Bitboard::EMPTY {
            doubled += 1;
            score -= if open_file {
                if phase >= 16 {
                    12
                } else {
                    16
                }
            } else if phase >= 16 {
                8
            } else {
                12
            };
        }

        let support_rank = if color.is_white() {
            rank.offset(-1)
        } else {
            rank.offset(1)
        };
        let support = [file.offset(-1), file.offset(1)]
            .into_iter()
            .flatten()
            .flat_map(|f| support_rank.map(|r| Bitboard::from(Square::from_coords(f, r))))
            .fold(Bitboard::EMPTY, |acc, bb| acc | (own & bb));
        if support != Bitboard::EMPTY {
            chain += 1;
        }

        let passed_here = (passed_bb & Bitboard::from(sq)) != Bitboard::EMPTY;

        if passed_here {
            passed += 1;
        } else {
            let mut own_ahead = 0;
            let mut opp_ahead = 0;
            for df in [-1, 1] {
                if let Some(f) = file.offset(df) {
                    let mut r = rank;
                    while let Some(next_r) = r.offset(step) {
                        r = next_r;
                        let bb = Bitboard::from(Square::from_coords(f, r));
                        if (own & bb) != Bitboard::EMPTY {
                            own_ahead += 1;
                        }
                        if (opp & bb) != Bitboard::EMPTY {
                            opp_ahead += 1;
                        }
                    }
                }
            }
            if own_ahead >= opp_ahead && own_ahead > 0 {
                candidate += 1;
                score += 6 + i64::from(phase) / 4;
            } else if adjacent_own == Bitboard::EMPTY && opp_ahead > 0 {
                weak += 1;
                score -= if phase >= 16 { 13 } else { 19 };
            }
        }
    }

    let islands = files
        .into_iter()
        .fold((0_i64, false), |(count, prev), cur| {
            let count = if cur && !prev { count + 1 } else { count };
            (count, cur)
        })
        .0;
    if islands > 1 {
        score -= (islands - 1) * 7;
    }

    if files.iter().all(|&has| has) {
        score -= 10;
    }

    let mut terms = serde_json::Map::new();
    terms.insert("isolated".into(), serde_json::Value::from(isolated));
    terms.insert("doubled".into(), serde_json::Value::from(doubled));
    terms.insert("candidate".into(), serde_json::Value::from(candidate));
    terms.insert("weak".into(), serde_json::Value::from(weak));
    terms.insert("chain".into(), serde_json::Value::from(chain));
    terms.insert(
        "open_files".into(),
        serde_json::Value::from(files.iter().filter(|&&has| has).count() as i64),
    );
    terms.insert("passed".into(), serde_json::Value::from(passed));
    terms.insert("islands".into(), serde_json::Value::from(islands));

    // --- Pawn-majority / flank counts ---
    let mut own_files_count = [0_i64; 8];
    let mut opp_files_count = [0_i64; 8];
    for f in 0..8 {
        let file_mask = Bitboard::from(File::new(f));
        let idx = f as usize;
        own_files_count[idx] = (own & file_mask).count() as i64;
        opp_files_count[idx] = (opp & file_mask).count() as i64;
    }
    let own_qs = own_files_count[0] + own_files_count[1] + own_files_count[2];
    let opp_qs = opp_files_count[0] + opp_files_count[1] + opp_files_count[2];
    let own_center = own_files_count[3] + own_files_count[4];
    let opp_center = opp_files_count[3] + opp_files_count[4];
    let own_ks = own_files_count[5] + own_files_count[6] + own_files_count[7];
    let opp_ks = opp_files_count[5] + opp_files_count[6] + opp_files_count[7];

    terms.insert("queenside_count".into(), serde_json::Value::from(own_qs));
    terms.insert("queenside_opp".into(), serde_json::Value::from(opp_qs));
    terms.insert("center_count".into(), serde_json::Value::from(own_center));
    terms.insert("center_opp".into(), serde_json::Value::from(opp_center));
    terms.insert("kingside_count".into(), serde_json::Value::from(own_ks));
    terms.insert("kingside_opp".into(), serde_json::Value::from(opp_ks));

    let w = weights();
    let maj_qs = own_qs - opp_qs;
    let maj_center = own_center - opp_center;
    let maj_ks = own_ks - opp_ks;
    score += maj_qs * w.pawn_majority_weight + maj_center * w.pawn_majority_weight + maj_ks * w.pawn_majority_weight;

    terms.insert(
        "majority_queenside".into(),
        serde_json::Value::from(if own_qs > opp_qs { 1 } else { 0 }),
    );
    terms.insert(
        "majority_center".into(),
        serde_json::Value::from(if own_center > opp_center { 1 } else { 0 }),
    );
    terms.insert(
        "majority_kingside".into(),
        serde_json::Value::from(if own_ks > opp_ks { 1 } else { 0 }),
    );

    // --- Pawn-break detection (simple passed-pawn creating pushes/captures) ---
    let mut break_count = 0_i64;
    let mut break_examples: Vec<serde_json::Value> = Vec::new();
    let opp_pawns_bb = board.by_color(color.other()) & board.by_role(Role::Pawn);
    for sq in own {
        let file = sq.file();
        let rank = sq.rank();
        // push one
        if let Some(next_rank) = if color.is_white() { rank.offset(1) } else { rank.offset(-1) } {
            let to = Square::from_coords(file, next_rank);
            if (board.occupied() & Bitboard::from(to)) == Bitboard::EMPTY {
                let front = in_front(color, to);
                if (opp_pawns_bb & front) == Bitboard::EMPTY {
                    break_count += 1;
                    let mut map = serde_json::Map::new();
                    map.insert("pawn".into(), serde_json::Value::from(piece_square_name(board, sq)));
                    map.insert("to".into(), serde_json::Value::from(to.to_string()));
                    map.insert("kind".into(), serde_json::Value::from("push"));
                    break_examples.push(serde_json::Value::Object(map));
                }
            }
        }
        // captures
        for df in [-1_i8, 1_i8] {
            if let Some(f) = file.offset(df as i32) {
                if let Some(next_rank) = if color.is_white() { rank.offset(1) } else { rank.offset(-1) } {
                    let to = Square::from_coords(f, next_rank);
                    if (board.by_color(color.other()) & Bitboard::from(to)).any() {
                        let front = in_front(color, to);
                        if (opp_pawns_bb & front) == Bitboard::EMPTY {
                            break_count += 1;
                            let mut map = serde_json::Map::new();
                            map.insert("pawn".into(), serde_json::Value::from(piece_square_name(board, sq)));
                            map.insert("to".into(), serde_json::Value::from(to.to_string()));
                            map.insert("kind".into(), serde_json::Value::from("capture"));
                            break_examples.push(serde_json::Value::Object(map));
                        }
                    }
                }
            }
        }
    }
    score += break_count * w.pawn_break_weight;
    terms.insert("pawn_breaks".into(), serde_json::Value::from(break_count));
    if !break_examples.is_empty() {
        terms.insert("pawn_break_examples".into(), serde_json::Value::Array(break_examples));
    }

    // --- Minority attack potential (advanced template + strength heuristic) ---
    let mut minority_flag = 0_i64;
    let mut minority_strength = 0_i64;
    let mut minority_examples: Vec<serde_json::Value> = Vec::new();
    if own_qs > opp_qs && opp_files_count[1] > 0 && (opp_files_count[0] > 0 || opp_files_count[2] > 0) {
        // compute simple vulnerability metrics
        let mut opp_pawns_on_qs = 0_i64;
        let mut opp_defended = 0_i64;
        let opp_color = color.other();
        for f in 0..3 {
            let file_mask = Bitboard::from(File::new(f));
            for sq in board.by_color(opp_color) & board.by_role(Role::Pawn) & file_mask {
                opp_pawns_on_qs += 1;
                // is this pawn defended by another opponent pawn?
                let def_by_pawn = (pawn_attack_mask(board, opp_color) & Bitboard::from(sq)).any();
                if def_by_pawn {
                    opp_defended += 1;
                }
            }
        }

        // candidate target squares for minority (b4, b5, c4, c5) — generic useful targets
        let targets = vec![Square::B4, Square::B5, Square::C4, Square::C5];
        let mut holes = 0_i64;
        let mut empty_targets: Vec<String> = Vec::new();
        for &t in &targets {
            if (board.occupied() & Bitboard::from(t)) == Bitboard::EMPTY {
                holes += 1;
                empty_targets.push(t.to_string());
            }
        }

        // strength heuristic: base diff scaled by vulnerabilities and holes
        let base = (own_qs - opp_qs).max(1);
        let vuln = (opp_pawns_on_qs - opp_defended).max(0);
        minority_strength = base * (1 + holes + vuln);

        minority_flag = 1;
        score += minority_strength * w.minority_attack_weight; // scale

        let mut m = serde_json::Map::new();
        m.insert("flank".into(), serde_json::Value::from("queenside"));
        m.insert("ours".into(), serde_json::Value::from(own_qs));
        m.insert("theirs".into(), serde_json::Value::from(opp_qs));
        m.insert("holes".into(), serde_json::Value::from(holes));
        m.insert("vulnerability".into(), serde_json::Value::from(vuln));
        m.insert(
            "targets".into(),
            serde_json::Value::Array(empty_targets.into_iter().map(serde_json::Value::from).collect()),
        );
        terms.insert("minority_attack_example".into(), serde_json::Value::Object(m.clone()));
        minority_examples.push(serde_json::Value::Object(m));
    }
    terms.insert("minority_attack".into(), serde_json::Value::from(minority_flag));
    terms.insert("minority_attack_strength".into(), serde_json::Value::from(minority_strength));
    if !minority_examples.is_empty() {
        terms.insert("minority_attack_examples".into(), serde_json::Value::Array(minority_examples));
    }

    (score, terms)
}

fn passed_pawn_score(
    board: &shakmaty::Board,
    color: Color,
) -> (i64, serde_json::Map<String, serde_json::Value>) {
    let mut score = 0;
    let mut count = 0;

    for sq in passed_pawn_mask(board, color) {
        count += 1;
        let rank = sq.rank();
        let advance = if color.is_white() {
            u32::from(rank)
        } else {
            7 - u32::from(rank)
        };
        score += 20 + i64::from(advance) * 12;
        if advance >= 4 {
            score += 18;
        }
        if advance >= 5 {
            score += 24;
        }
    }

    let mut terms = serde_json::Map::new();
    terms.insert("passed_count".into(), serde_json::Value::from(count));
    (score, terms)
}

// _phase is kept for potential future phase-dependent king safety tuning but is
// not currently used inside this function.
fn king_safety_score(board: &shakmaty::Board, graph: &ThreatGraph, color: Color, in_check: bool, _phase: u8) -> i64 {
    let king_sq = match board.king_of(color) {
        Some(sq) => sq,
        None => return 0,
    };

    let mut score = 0;
    if in_check {
        score -= 80;
    }

    // Critter-style pawn_safe: squares NOT attacked by enemy pawns.
    // King is safer when standing in pawn shelter. board.attacks_from(pawn_sq)
    // is exactly attacks::pawn_attacks(color, sq) (shakmaty dispatches pawns
    // there unconditionally, ignoring occupancy — pawn attacks aren't
    // blockable), so this is exactly pawn_attack_mask, not a fresh derivation.
    let pawn_safe = !pawn_attack_mask(board, color.other());
    if (Bitboard::from(king_sq) & pawn_safe).any() {
        score += 15; // king stands on pawn-protected square
    }

    // Same primitive attackers_to is built from, read from the graph already
    // built for this position instead of a second, separate shakmaty call
    // (same pattern as is_in_check).
    let attackers = graph.attackers(king_sq, color.other());
    // Critter-style weighted king attacks: queen=300, rook=200, minor=100
    let queen_att  = (attackers & board.by_role(Role::Queen)).count() as i64 * 3;
    let rook_att  = (attackers & board.by_role(Role::Rook)).count() as i64 * 2;
    let minor_att = (attackers & (board.by_role(Role::Knight) | board.by_role(Role::Bishop))).count() as i64;
    let danger = (queen_att + rook_att + minor_att).min(20);
    score -= danger * 12;

    let own_pawns = board.by_color(color) & board.by_role(Role::Pawn);
    let enemy_pawns = board.by_color(color.other()) & board.by_role(Role::Pawn);
    let file = king_sq.file();
    for df in -1..=1 {
        if let Some(f) = file.offset(df) {
            let mut shield_rank = if color.is_white() {
                Rank::First
            } else {
                Rank::Eighth
            };
            let mut storm_rank = shield_rank;
            for r in 0..8 {
                let rank = Rank::new(r as u32);
                let sq = Square::from_coords(f, rank);
                if (own_pawns & Bitboard::from(sq)) != Bitboard::EMPTY {
                    shield_rank = rank;
                }
                if (enemy_pawns & Bitboard::from(sq)) != Bitboard::EMPTY {
                    storm_rank = rank;
                    break;
                }
            }

            let shield = if color.is_white() {
                u8::from(shield_rank)
            } else {
                7 - u8::from(shield_rank)
            };
            let storm = if color.is_white() {
                u8::from(storm_rank)
            } else {
                7 - u8::from(storm_rank)
            };
            score += [77, 0, 13, 38, 51, 64, 64, 64][shield as usize];
            score += [13, 0, 90, 38, 13, 0, 0, 0][storm as usize];
        }
    }

    score
}

fn development_score(board: &shakmaty::Board, color: Color) -> i64 {
    const NOT_DEVELOPED: [i64; 16] = [
        0, 3, 10, 15, 25, 38, 51, 77, 69, 79, 89, 100, 115, 115, 115, 115,
    ];
    let undeveloped = count_undeveloped(board, color).min(15) as usize;
    -NOT_DEVELOPED[undeveloped]
}

fn development_space_score(board: &shakmaty::Board, graph: &ThreatGraph, color: Color, phase: u8) -> i64 {
    let own_pawns = board.by_color(color) & board.by_role(Role::Pawn);
    let enemy_pawn_attacks = pawn_attack_mask(board, color.other());
    // Same primitive attackers_to is built from, read from the graph already
    // built for this position instead of two more separate shakmaty calls.
    let enemy_attacks = graph.attackers(
        board.king_of(color).unwrap_or(if color.is_white() {
            Square::E1
        } else {
            Square::E8
        }),
        color.other(),
    );
    let own_attacks = graph.attackers(
        board.king_of(color.other()).unwrap_or(if color.is_white() {
            Square::E8
        } else {
            Square::E1
        }),
        color,
    );

    let base_mask = Bitboard::from(Square::C4)
        | Bitboard::from(Square::D4)
        | Bitboard::from(Square::E4)
        | Bitboard::from(Square::F4)
        | Bitboard::from(Square::C5)
        | Bitboard::from(Square::D5)
        | Bitboard::from(Square::E5)
        | Bitboard::from(Square::F5);

    let safe = base_mask & !(own_pawns | enemy_pawn_attacks | (enemy_attacks & !own_attacks));
    let pawns = board.by_color(color) & board.by_role(Role::Pawn);
    let mut shifted = pawns;
    if color.is_white() {
        shifted |= shifted.shift(8);
        shifted |= shifted.shift(16);
    } else {
        shifted |= shifted.shift(-8);
        shifted |= shifted.shift(-16);
    }

    bitboard_count(safe) * i64::from(phase.max(1)) * (bitboard_count(shifted) + 1) / 8
}

fn in_front(color: Color, sq: Square) -> Bitboard {
    let sq_bb = Bitboard::from(sq);
    let (s1, s2, s4) = if color.is_white() {
        (8_i32, 16, 32)
    } else {
        (-8_i32, -16, -32)
    };
    let mut bb = sq_bb;
    bb |= bb.shift(s1);
    bb |= bb.shift(s2);
    bb |= bb.shift(s4);
    bb ^ sq_bb
}

/// Critter-style mobility mask: squares a piece can actually move to.
/// Excludes friendly occupied squares and squares attacked by enemy pawns.
fn mobility_mask(board: &shakmaty::Board, color: Color) -> Bitboard {
    let friendly = board.by_color(color);
    // Same derivation as king_safety_score's pawn_safe: board.attacks_from on
    // a pawn square is exactly attacks::pawn_attacks (occupancy-independent),
    // i.e. exactly pawn_attack_mask — not re-derived by hand here either.
    !(friendly | pawn_attack_mask(board, color.other()))
}

fn piece_activity_score(
    board: &shakmaty::Board,
    graph: &ThreatGraph,
    color: Color,
    phase: u8,
    pawn_safe: Bitboard,
    king_ring_bb: Bitboard,
) -> (i64, serde_json::Map<String, serde_json::Value>) {
    let mob_mask = mobility_mask(board, color);
    let mut score = 0;
    let occupied = board.occupied();
    let enemy = board.by_color(color.other());
    let enemy_king = board.king_of(color.other());
    let w = weights();

    let mut knight_score = 0;
    for sq in board.by_color(color) & board.by_role(Role::Knight) {
        let atk = attacks::knight_attacks(sq);
        let mut local = 0;
        local += 15 * (atk & mob_mask & in_front(color, sq)).count() as i64;
        if (atk & king_ring_bb) != Bitboard::EMPTY {
            local += 10;
        }
        if (Bitboard::from(sq) & Bitboard::CENTER) != Bitboard::EMPTY {
            local += 8;
        }
        if (atk & pawn_safe & enemy.intersect(board.by_role(Role::Pawn))).any() {
            local += 9;
        }
        if let Some(ksq) = enemy_king {
            if (atk & Bitboard::from(ksq)) != Bitboard::EMPTY {
                local += 12;
            }
        }
        if (atk & enemy.intersect(board.by_role(Role::Rook) | board.by_role(Role::Queen))).any() {
            local += 10;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Pawn)))
        .any()
        {
            local -= 13;
        }
        if (atk & board.by_color(color).intersect(board.by_role(Role::Pawn))).count() == 0 {
            local -= 18;
        }
        if color.is_white() {
            if sq.rank() == Rank::First {
                local -= 10;
            }
        } else if sq.rank() == Rank::Eighth {
            local -= 10;
        }
        knight_score += local;
    }

    let mut bishop_score = 0;
    for sq in board.by_color(color) & board.by_role(Role::Bishop) {
        let atk = attacks::bishop_attacks(sq, occupied);
        let mut local = 0;
        local += 12 * (atk & mob_mask & in_front(color, sq)).count() as i64;
        if (atk & king_ring_bb) != Bitboard::EMPTY {
            local += 13;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Pawn)))
        .any()
        {
            local += 7;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Knight)))
        .any()
        {
            local += 13;
        }
        if (atk
            & (board
                .by_color(color.other())
                .intersect(board.by_role(Role::Rook))
                | board
                    .by_color(color.other())
                    .intersect(board.by_role(Role::Queen))))
        .any()
        {
            local += 18;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Pawn)))
        .any()
        {
            local -= 13;
        }
        if color.is_white() {
            if sq.rank() == Rank::First {
                local -= 13;
            }
        } else if sq.rank() == Rank::Eighth {
            local -= 13;
        }
        bishop_score += local;
    }

    let mut rook_score = 0;
    let mut open_file_controlled = 0;
    let mut rook_on_seventh = 0;
    for sq in board.by_color(color) & board.by_role(Role::Rook) {
        let atk = attacks::rook_attacks(sq, occupied);
        let mut local = 0;
        local += 5 * (atk & in_front(color, sq)).count() as i64;
        if (atk & king_ring_bb) != Bitboard::EMPTY {
            local += 8;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Pawn)))
        .any()
        {
            local += 8;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Knight) | board.by_role(Role::Bishop)))
        .any()
        {
            local += 13;
        }
        if (atk
            & board
                .by_color(color.other())
                .intersect(board.by_role(Role::Queen)))
        .any()
        {
            local += 13;
        }
        if (atk & board.by_color(color).intersect(board.by_role(Role::Pawn))).count() == 0 {
            local += 15;
        }
        // Identify open file: file has no pawns of either color
        let file_mask = Bitboard::from(sq.file());
        let pawns_on_file = (board.by_role(Role::Pawn) & file_mask).any();
        if !pawns_on_file {
            open_file_controlled += 1;
            local += w.rook_open_file_bonus; // configurable
        }
        // Critter-style rook rank bonus: only if enemy has king or pawns there
        let rook_rank = sq.rank();
        let (seventh, eighth, sixth) = if color.is_white() {
            (Rank::Seventh, Rank::Eighth, Rank::Sixth)
        } else {
            (Rank::Second, Rank::First, Rank::Third)
        };
        let enemy_back_ranks = if color.is_white() {
            Bitboard::from(Rank::Seventh) | Bitboard::from(Rank::Eighth)
        } else {
            Bitboard::from(Rank::First) | Bitboard::from(Rank::Second)
        };
        let enemy_king_or_pawns = enemy & (board.by_role(Role::King) | board.by_role(Role::Pawn));
        if rook_rank == seventh {
            if (enemy_king_or_pawns & enemy_back_ranks).any() {
                local += w.rook_seventh_bonus;
                rook_on_seventh += 1;
            }
        } else if rook_rank == eighth {
            let enemy_king = board.by_color(color.other()) & board.by_role(Role::King);
            if (enemy_king & Bitboard::from(eighth)).any() {
                local += 13;
            }
        } else if rook_rank == sixth && (enemy_king_or_pawns & enemy_back_ranks).any() {
            local += 10;
        }
        rook_score += local;
    }

    let mut doubled_rooks = 0;
    for f in 0..8 {
        let file_mask = Bitboard::from(File::new(f));
        let cnt = (board.by_color(color) & board.by_role(Role::Rook) & file_mask).count();
        if cnt >= 2 {
            doubled_rooks += 1;
            rook_score += w.doubled_rook_bonus; // configurable
        }
    }

    let mut queen_score = 0;
    for sq in board.by_color(color) & board.by_role(Role::Queen) {
        let atk = attacks::queen_attacks(sq, occupied);
        let mut local = 0;
        local += 5 * (atk & mob_mask).count() as i64 / 2;
        if (atk & king_ring_bb) != Bitboard::EMPTY {
            local += 13;
        }
        if color.is_white() {
            if sq.rank() == Rank::Seventh {
                local += 13;
            }
        } else if sq.rank() == Rank::Second {
            local += 13;
        }
        if let Some(ksq) = enemy_king {
            if (atk & Bitboard::from(ksq)) != Bitboard::EMPTY {
                local += 18;
            }
        }
        queen_score += local;
    }

    score += knight_score + bishop_score + rook_score + queen_score;

    // Mobility counters per piece (counts of attacked squares excluding own-occupied squares)
    let mut knight_mob = 0_i64;
    let mut bishop_mob = 0_i64;
    let mut rook_mob = 0_i64;
    let mut queen_mob = 0_i64;
    let mut pawn_mob = 0_i64;

    // graph.attacks_from(sq) here is exactly board.attacks_from(sq) for these
    // occupied squares — read from the graph already built instead of asking
    // shakmaty to redo it.
    for sq in board.by_color(color) & board.by_role(Role::Knight) {
        knight_mob += (graph.attacks_from(sq) & !board.by_color(color)).count() as i64;
    }
    for sq in board.by_color(color) & board.by_role(Role::Bishop) {
        bishop_mob += (graph.attacks_from(sq) & !board.by_color(color)).count() as i64;
    }
    for sq in board.by_color(color) & board.by_role(Role::Rook) {
        rook_mob += (graph.attacks_from(sq) & !board.by_color(color)).count() as i64;
    }
    for sq in board.by_color(color) & board.by_role(Role::Queen) {
        queen_mob += (graph.attacks_from(sq) & !board.by_color(color)).count() as i64;
    }
    for sq in board.by_color(color) & board.by_role(Role::Pawn) {
        // pawn mobility: forward push if empty + captures
        let mut m = 0_i64;
        let file = sq.file();
        let rank = sq.rank();
        if let Some(nrank) = if color.is_white() { rank.offset(1) } else { rank.offset(-1) } {
            let to = Square::from_coords(file, nrank);
            if (board.occupied() & Bitboard::from(to)) == Bitboard::EMPTY {
                m += 1;
            }
        }
        for df in [-1_i8, 1_i8] {
            if let Some(f) = file.offset(df as i32) {
                if let Some(nrank) = if color.is_white() { rank.offset(1) } else { rank.offset(-1) } {
                    let to = Square::from_coords(f, nrank);
                    if (board.by_color(color.other()) & Bitboard::from(to)).any() {
                        m += 1;
                    }
                }
            }
        }
        pawn_mob += m;
    }
    let mobility_total = knight_mob + bishop_mob + rook_mob + queen_mob + pawn_mob;
    let w_mob = WEIGHTS.read().expect("weights lock").piece_mobility_weight;
    score += mobility_total * w_mob;

    let mut terms = serde_json::Map::new();
    terms.insert("knight".into(), serde_json::Value::from(knight_score));
    terms.insert("bishop".into(), serde_json::Value::from(bishop_score));
    terms.insert("rook".into(), serde_json::Value::from(rook_score));
    terms.insert("queen".into(), serde_json::Value::from(queen_score));
    terms.insert("phase".into(), serde_json::Value::from(phase as i64));
    terms.insert("open_files_controlled".into(), serde_json::Value::from(open_file_controlled));
    terms.insert("rook_on_seventh".into(), serde_json::Value::from(rook_on_seventh));
    terms.insert("doubled_rooks".into(), serde_json::Value::from(doubled_rooks));
    terms.insert("mobility_total".into(), serde_json::Value::from(mobility_total));
    terms.insert("mobility_knight".into(), serde_json::Value::from(knight_mob));
    terms.insert("mobility_bishop".into(), serde_json::Value::from(bishop_mob));
    terms.insert("mobility_rook".into(), serde_json::Value::from(rook_mob));
    terms.insert("mobility_queen".into(), serde_json::Value::from(queen_mob));
    terms.insert("mobility_pawn".into(), serde_json::Value::from(pawn_mob));
    (score, terms)
}

/// Tactical motif detectors and king tropism helpers (pins, forks, skewers, discovered, tropism)
fn chebyshev_distance(a: Square, b: Square) -> i64 {
    let df = (a.file() as i32 - b.file() as i32).abs() as i64;
    let dr = (a.rank() as i32 - b.rank() as i32).abs() as i64;
    df.max(dr)
}

fn king_tropism_score(board: &shakmaty::Board, color: Color) -> i64 {
    let enemy_king = match board.king_of(color.other()) {
        Some(sq) => sq,
        None => return 0,
    };
    let mut score = 0_i64;
    let w = weights();
    for sq in board.by_color(color) {
        // skip king itself
        if Some(sq) == board.king_of(color) {
            continue;
        }
        let dist = chebyshev_distance(sq, enemy_king);
        let closeness = 8 - dist; // 0..8
        if closeness <= 0 {
            continue;
        }
        // piece weight (configurable)
        let sq_bb = Bitboard::from(sq);
        let weight = if (sq_bb & board.by_role(Role::Queen)).any() {
            w.tropism_queen
        } else if (sq_bb & board.by_role(Role::Rook)).any() {
            w.tropism_rook
        } else if (sq_bb & board.by_role(Role::Bishop)).any() {
            w.tropism_bishop
        } else if (sq_bb & board.by_role(Role::Knight)).any() {
            w.tropism_knight
        } else if (sq_bb & board.by_role(Role::Pawn)).any() {
            w.tropism_pawn
        } else {
            0
        };
        score += weight * closeness / 2;
    }
    score
}

fn detect_pins(board: &shakmaty::Board, color: Color) -> (i64, Vec<(Square, Square, Square)>) {
    // returns (count, vec![(pinning_piece_sq, pinned_piece_sq, king_sq), ...])
    let king_sq = match board.king_of(color) {
        Some(sq) => sq,
        None => return (0, Vec::new()),
    };
    let occ = board.occupied();
    let mut pins = 0_i64;
    let mut examples: Vec<(Square, Square, Square)> = Vec::new();

    let sliders = board.by_color(color.other())
        & (board.by_role(Role::Rook) | board.by_role(Role::Bishop) | board.by_role(Role::Queen));

    for blocker in board.by_color(color) {
        let occ_minus = occ ^ Bitboard::from(blocker);
        for s in sliders {
            let s_bb = Bitboard::from(s);
            let is_rook = (s_bb & board.by_role(Role::Rook)).any();
            let is_bishop = (s_bb & board.by_role(Role::Bishop)).any();
            let is_queen = (s_bb & board.by_role(Role::Queen)).any();

            let mut before = Bitboard::EMPTY;
            let mut after = Bitboard::EMPTY;
            if is_rook || is_queen {
                before |= attacks::rook_attacks(s, occ);
                after |= attacks::rook_attacks(s, occ_minus);
            }
            if is_bishop || is_queen {
                before |= attacks::bishop_attacks(s, occ);
                after |= attacks::bishop_attacks(s, occ_minus);
            }

            let king_before = (before & Bitboard::from(king_sq)).any();
            let king_after = (after & Bitboard::from(king_sq)).any();
            if !king_before && king_after {
                pins += 1;
                examples.push((s, blocker, king_sq));
                break;
            }
        }
    }
    (pins, examples)
}

fn detect_forks(board: &shakmaty::Board, color: Color) -> (i64, Vec<(Square, Vec<Square>)>) {
    // returns (count, vec![(attacker_sq, vec![target_sqs...]), ...])
    let mut forks = 0_i64;
    let mut examples: Vec<(Square, Vec<Square>)> = Vec::new();
    let enemy_bb = board.by_color(color.other());
    for sq in board.by_color(color) {
        let attacks = board.attacks_from(sq);
        let attacked_pieces = attacks & enemy_bb;
        if attacked_pieces.count() < 2 {
            continue;
        }
        // sum values of attacked pieces (configurable values)
        let mut sum = 0_i64;
        let mut targets: Vec<Square> = Vec::new();
        let w = weights();
        for (role, val) in [
            (Role::Queen, w.val_queen),
            (Role::Rook, w.val_rook),
            (Role::Bishop, w.val_bishop),
            (Role::Knight, w.val_knight),
            (Role::Pawn, w.val_pawn),
        ] {
            let mask = attacked_pieces & board.by_role(role);
            for t in mask {
                sum += val;
                targets.push(t);
            }
        }
        // Count as fork when at least two pieces attacked and combined value above threshold
        if sum >= (w.val_rook) || attacked_pieces.count() >= 3 {
            forks += 1;
            examples.push((sq, targets.clone()));
        }
    }
    (forks, examples)
}

fn detect_skewers(board: &shakmaty::Board, color: Color) -> (i64, Vec<(Square, Square, Square)>) {
    // returns (count, vec![(attacker_sq, front_sq, back_sq), ...])
    let mut skewers = 0_i64;
    let enemy = color.other();
    let directions: &[(i8, i8)] = &[
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ];

    let mut examples: Vec<(Square, Square, Square)> = Vec::new();

    for s in board.by_color(color) & (board.by_role(Role::Rook) | board.by_role(Role::Bishop) | board.by_role(Role::Queen)) {
        let s_bb = Bitboard::from(s);
        let is_rook = (s_bb & board.by_role(Role::Rook)).any();
        let is_bishop = (s_bb & board.by_role(Role::Bishop)).any();
        let is_queen = (s_bb & board.by_role(Role::Queen)).any();

        for (df, dr) in directions {
            // skip directions inappropriate for piece
            if !is_queen {
                if is_rook && *dr != 0 && *df != 0 {
                    continue;
                }
                if is_bishop && (*dr == 0 || *df == 0) {
                    continue;
                }
            }

            // walk the ray
            let mut found: Vec<Square> = Vec::new();
            let mut cur_file = s.file();
            let mut cur_rank = s.rank();
            loop {
                if let Some(nf) = cur_file.offset(*df as i32) {
                    if let Some(nr) = cur_rank.offset(*dr as i32) {
                        cur_file = nf;
                        cur_rank = nr;
                        let sq = Square::from_coords(cur_file, cur_rank);
                        let sq_bb = Bitboard::from(sq);
                        if (board.occupied() & sq_bb).any() {
                            if (board.by_color(enemy) & sq_bb).any() {
                                found.push(sq);
                                if found.len() >= 2 {
                                    break; // found both pieces, done
                                }
                                // continue ray past first enemy (it would move when skewered)
                                continue;
                            }
                            break; // friendly piece blocks the ray
                        }
                        continue; // empty square, keep walking
                    }
                }
                break;
            }

            if found.len() >= 2 {
                let w = weights();
                let val = |sq: Square| {
                    if (Bitboard::from(sq) & board.by_role(Role::Queen)).any() {
                        w.val_queen
                    } else if (Bitboard::from(sq) & board.by_role(Role::Rook)).any() {
                        w.val_rook
                    } else if (Bitboard::from(sq) & board.by_role(Role::Bishop)).any() {
                        w.val_bishop
                    } else if (Bitboard::from(sq) & board.by_role(Role::Knight)).any() {
                        w.val_knight
                    } else if (Bitboard::from(sq) & board.by_role(Role::Pawn)).any() {
                        w.val_pawn
                    } else {
                        0
                    }
                };
                let v0 = val(found[0]);
                let v1 = val(found[1]);
                if v0 > v1 {
                    skewers += 1;
                    examples.push((s, found[0], found[1]));
                }
            }
        }
    }
    (skewers, examples)
}

/// Rough piece values for the discovered-attack "is this actually significant" filter below.
/// Not the tuned eval-material table — just enough to tell a winning reveal from a trivial one.
fn discovered_attack_piece_value(role: Role) -> i64 {
    match role {
        Role::Pawn => 100, Role::Knight => 320, Role::Bishop => 330,
        Role::Rook => 500, Role::Queen => 900, Role::King => 20000,
    }
}

fn detect_discovered(board: &shakmaty::Board, color: Color) -> (i64, Vec<(Square, Square, Square)>) {
    // returns (count, vec![(blocker_sq, slider_sq, target_sq), ...])
    let occ = board.occupied();
    let mut discovered = 0_i64;
    let mut examples: Vec<(Square, Square, Square)> = Vec::new();
    let enemy_bb = board.by_color(color.other());
    let sliders = board.by_color(color) & (board.by_role(Role::Rook) | board.by_role(Role::Bishop) | board.by_role(Role::Queen));

    for blocker in board.by_color(color) {
        let occ_minus = occ ^ Bitboard::from(blocker);
        for s in sliders {
            let s_bb = Bitboard::from(s);
            let is_rook = (s_bb & board.by_role(Role::Rook)).any();
            let is_bishop = (s_bb & board.by_role(Role::Bishop)).any();
            let is_queen = (s_bb & board.by_role(Role::Queen)).any();
            let mut before = Bitboard::EMPTY;
            let mut after = Bitboard::EMPTY;
            if is_rook || is_queen {
                before |= attacks::rook_attacks(s, occ);
                after |= attacks::rook_attacks(s, occ_minus);
            }
            if is_bishop || is_queen {
                before |= attacks::bishop_attacks(s, occ);
                after |= attacks::bishop_attacks(s, occ_minus);
            }
            let newly = (after & enemy_bb) & !before;
            if !newly.any() {
                continue;
            }
            // Reject trivial reveals: a slider seeing an adequately-defended enemy piece of
            // equal-or-lesser value through a now-open file/diagonal (e.g. a rook behind its
            // own pawn "attacking" the enemy's mirrored pawn) isn't a tactical discovered
            // attack — it's just normal blocked-slider geometry that exists in nearly every
            // position. Only count it when the target is undefended or worth more than the
            // attacker, i.e. actually winning something.
            let attacker_role = if is_queen { Role::Queen } else if is_rook { Role::Rook } else { Role::Bishop };
            let attacker_value = discovered_attack_piece_value(attacker_role);
            let significant = newly.into_iter().find(|&t| {
                let defended = board.attacks_to(t, color.other(), occ_minus).any();
                let target_value = board.piece_at(t).map(|p| discovered_attack_piece_value(p.role)).unwrap_or(0);
                !defended || target_value > attacker_value
            });
            if let Some(t) = significant {
                discovered += 1;
                examples.push((blocker, s, t));
                break;
            }
        }
    }
    (discovered, examples)
}

fn piece_square_name(board: &shakmaty::Board, sq: Square) -> String {
    if let Some(piece) = board.piece_at(sq) {
        let letter = match piece.role {
            Role::Pawn => "P", Role::Knight => "N", Role::Bishop => "B",
            Role::Rook => "R", Role::Queen => "Q", Role::King => "K",
        };
        return format!("{}{}", letter, sq);
    }
    format!("{}", sq)
}

fn board_to_piece_ref(board: &shakmaty::Board, sq: Square) -> Option<PieceRef> {
    board.piece_at(sq).map(|p| PieceRef {
        role: role_name(p.role),
        color: Side::from(p.color),
        square: sq.to_string(),
    })
}

fn outposts_to_typed(board: &shakmaty::Board, examples: &[(Square, Role, Square)]) -> Vec<Outpost> {
    examples.iter().filter_map(|(sq, role, support)| {
        let piece = board_to_piece_ref(board, *sq)?;
        // Defensive fallback only — the support square for a detected outpost
        // should always have a piece on it; role/color here are placeholders
        // that shouldn't ever actually surface.
        let support_ref = board_to_piece_ref(board, *support).unwrap_or(PieceRef {
            role: "Pawn".into(), color: Side::White, square: "?".into(),
        });
        if matches!(role, Role::Knight | Role::Bishop) {
            Some(Outpost { piece, supported_by: support_ref })
        } else { None }
    }).collect()
}

fn pins_to_typed(board: &shakmaty::Board, examples: &[(Square, Square, Square)]) -> Vec<Pin> {
    use PinType;
    examples.iter().filter_map(|(pinner_sq, pinned_sq, shielded_sq)| {
        let attacker = board_to_piece_ref(board, *pinner_sq)?;
        let pinned   = board_to_piece_ref(board, *pinned_sq)?;
        let shielded = board_to_piece_ref(board, *shielded_sq)?;
        let pin_type = if shielded.role == "King" { PinType::Absolute } else { PinType::Relative };
        Some(Pin { attacker, pinned, shielded, pin_type })
    }).collect()
}

fn skewers_to_typed(board: &shakmaty::Board, examples: &[(Square, Square, Square)]) -> Vec<Skewer> {
    examples.iter().filter_map(|(attacker_sq, front_sq, back_sq)| {
        let attacker = board_to_piece_ref(board, *attacker_sq)?;
        let front    = board_to_piece_ref(board, *front_sq)?;
        let behind   = board_to_piece_ref(board, *back_sq)?;
        Some(Skewer { attacker, front, behind })
    }).collect()
}

fn discovered_to_typed(board: &shakmaty::Board, examples: &[(Square, Square, Square)]) -> Vec<DiscoveredAttack> {
    examples.iter().filter_map(|(blocker_sq, slider_sq, target_sq)| {
        let mover   = board_to_piece_ref(board, *blocker_sq)?;
        let attacker = board_to_piece_ref(board, *slider_sq)?;
        let target  = board_to_piece_ref(board, *target_sq)?;
        Some(DiscoveredAttack { mover, attacker, target })
    }).collect()
}

// ── 1400 ELO extractors ──

fn extract_passed_pawns(board: &shakmaty::Board) -> Vec<PassedPawn> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        for sq in passed_pawn_mask(board, color) {
            let rank_idx = u32::from(sq.rank());
            let advance = if color.is_white() { rank_idx } else { 7 - rank_idx };
            let protected = board.attacks_to(sq, color, board.occupied()).any();
            results.push(PassedPawn {
                square: sq.to_string(), rank: advance as u8 + 2,
                color: Side::from(color),
                is_protected: protected,
            });
        }
    }
    results
}

fn extract_open_files(board: &shakmaty::Board) -> Vec<OpenFile> {
    let mut results = Vec::new();
    for file in 0..8u32 {
        let f = File::new(file);
        for color in [Color::White, Color::Black] {
            let own_pawns = board.by_color(color) & board.by_role(Role::Pawn) & Bitboard::from(f);
            let opp_pawns = board.by_color(color.other()) & board.by_role(Role::Pawn) & Bitboard::from(f);
            let rook_count = (board.by_color(color) & board.by_role(Role::Rook) & Bitboard::from(f)).count() as u8;
            if rook_count > 0 && own_pawns.is_empty() {
                let is_open = opp_pawns.is_empty();
                results.push(OpenFile {
                    file: f.to_string(), rook_count,
                    color: Side::from(color),
                });
                if is_open { break; }
            }
        }
    }
    results
}

fn extract_king_exposure(board: &shakmaty::Board) -> Vec<KingExposure> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        let king_sq = match board.king_of(color) {
            Some(sq) => sq, None => continue,
        };
        let ring = attacks::king_attacks(king_sq) | Bitboard::from(king_sq);
        let attacker_count = (board.by_color(color.other()) & ring).count() as u8;
        let file = king_sq.file();
        let mut shelter_files = 0u8;
        for df in -1..=1 {
            if let Some(f) = file.offset(df) {
                let pawns = board.by_color(color) & board.by_role(Role::Pawn) & Bitboard::from(f);
                if pawns.any() { shelter_files += 1; }
            }
        }
        // The king's own file counted separately and more strictly than
        // the flank-file tally above: a rook/queen has direct access down
        // this specific file, unlike the other two, so it needs its own
        // signal rather than being averaged into `shelter_files` (see the
        // field's doc comment).
        let king_file_open = (board.by_color(color) & board.by_role(Role::Pawn) & Bitboard::from(file)).is_empty();
        if attacker_count > 0 || shelter_files < 2 || king_file_open {
            results.push(KingExposure { color: Side::from(color), shelter_files, attacker_count, king_file_open });
        }
    }
    results
}

fn extract_isolated_pawns(board: &shakmaty::Board) -> Vec<IsolatedPawn> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        for sq in board.by_color(color) & board.by_role(Role::Pawn) {
            let file = sq.file();
            let adjacent = [file.offset(-1), file.offset(1)]
                .into_iter()
                .flatten()
                .filter_map(|f| {
                    let bb = board.by_color(color) & board.by_role(Role::Pawn) & Bitboard::from(f);
                    if bb.any() { Some(()) } else { None }
                })
                .count();
            if adjacent == 0 {
                results.push(IsolatedPawn {
                    square: sq.to_string(),
                    color: Side::from(color),
                });
            }
        }
    }
    results
}

fn extract_doubled_pawns(board: &shakmaty::Board) -> Vec<DoubledPawn> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        let pawns = board.by_color(color) & board.by_role(Role::Pawn);
        for file in 0..8u32 {
            let f = File::new(file);
            let count = (pawns & Bitboard::from(f)).count() as u8;
            if count > 1 {
                results.push(DoubledPawn {
                    file: f.to_string(), count,
                    color: Side::from(color),
                });
            }
        }
    }
    results
}

fn extract_pawn_islands(board: &shakmaty::Board) -> Vec<PawnIsland> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        let pawns = board.by_color(color) & board.by_role(Role::Pawn);
        let mut files = Vec::new();
        let mut prev_had = false;
        let mut island_count = 0u8;
        for file in 0..8u32 {
            let f = File::new(file);
            let has = (pawns & Bitboard::from(f)).any();
            if has {
                files.push(f.to_string());
                if !prev_had {
                    island_count += 1;
                }
            }
            prev_had = has;
        }
        if island_count > 1 {
            results.push(PawnIsland {
                files, count: island_count,
                color: Side::from(color),
            });
        }
    }
    results
}

fn extract_pawn_breaks(groups: &EvalGroups, us: Color, them: Color) -> Vec<PawnBreak> {
    let mut results = Vec::new();
    let break_examples = groups.pawn_structure.terms.get("pawn_break_examples");
    let opp_terms = groups.pawn_structure.terms.get("opp_terms")
        .and_then(|v| v.as_object());
    let opp_breaks = opp_terms.and_then(|o| o.get("pawn_break_examples"));
    let us_label = Side::from(us);
    let them_label = Side::from(them);

    // pawn_break_examples from the us side (for us pawns)
    if let Some(arr) = break_examples.and_then(|v| v.as_array()) {
        for ex in arr {
            if let (Some(pawn), Some(_to)) = (
                ex.get("pawn").and_then(|v| v.as_str()),
                ex.get("to").and_then(|v| v.as_str()),
            ) {
                results.push(PawnBreak { square: pawn.into(), color: us_label });
            }
        }
    }
    // opp pawn breaks
    if let Some(arr) = opp_breaks.and_then(|v| v.as_array()) {
        for ex in arr {
            if let (Some(pawn), Some(_to)) = (
                ex.get("pawn").and_then(|v| v.as_str()),
                ex.get("to").and_then(|v| v.as_str()),
            ) {
                results.push(PawnBreak { square: pawn.into(), color: them_label });
            }
        }
    }
    results
}

fn extract_minority_attack(groups: &EvalGroups, us: Color) -> Option<MinorityAttack> {
    let minority_flag = groups.pawn_structure.terms.get("minority_attack")
        .and_then(|v| v.as_i64()).unwrap_or(0);
    if minority_flag == 0 { return None; }
    let strength = groups.pawn_structure.terms.get("minority_attack_strength")
        .and_then(|v| v.as_i64()).unwrap_or(0);
    Some(MinorityAttack { color: Side::from(us), strength })
}

fn extract_pawn_majority(groups: &EvalGroups, us: Color, them: Color) -> Vec<PawnMajority> {
    let mut results = Vec::new();
    let majority_us = groups.pawn_structure.terms.get("majority_us").and_then(|v| v.as_i64()).unwrap_or(0);
    let majority_them = groups.pawn_structure.terms.get("majority_them").and_then(|v| v.as_i64()).unwrap_or(0);
    if majority_us > 0 {
        results.push(PawnMajority { color: Side::from(us), count: majority_us });
    }
    if majority_them > 0 {
        results.push(PawnMajority { color: Side::from(them), count: majority_them });
    }
    results
}

/// Rooks on the 7th (relative) rank, both sides. `piece_activity.terms` carries only the
/// "us" side's raw counters directly; the opponent's live under the nested `opp_terms` object.
fn extract_rook_on_seventh(groups: &EvalGroups, us: Color, them: Color) -> Vec<RookOnSeventh> {
    let mut results = Vec::new();
    let us_count = groups.piece_activity.terms.get("rook_on_seventh").and_then(|v| v.as_i64()).unwrap_or(0);
    let them_count = groups.piece_activity.terms.get("opp_terms")
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("rook_on_seventh"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if us_count > 0 {
        results.push(RookOnSeventh { color: Side::from(us), count: us_count as u8 });
    }
    if them_count > 0 {
        results.push(RookOnSeventh { color: Side::from(them), count: them_count as u8 });
    }
    results
}

fn extract_center_control(board: &shakmaty::Board) -> Option<CenterControl> {
    let cc_white = center_control_score(board, Color::White);
    let cc_black = center_control_score(board, Color::Black);
    let diff = cc_white - cc_black;
    if diff.abs() <= 15 { return None; }
    if diff > 0 {
        Some(CenterControl { color: Side::White, strength: diff })
    } else {
        Some(CenterControl { color: Side::Black, strength: -diff })
    }
}

fn extract_development_info(board: &shakmaty::Board, graph: &ThreatGraph) -> Vec<DevelopmentInfo> {
    let mut results = Vec::new();
    for color in [Color::White, Color::Black] {
        let undeveloped = count_undeveloped(board, color);
        let space = development_space_score(board, graph, color, compute_phase(board));
        if undeveloped > 0 || space < 0 {
            let pieces: Vec<PieceRef> = {
                let knight_home = color.fold_wb(
                    Bitboard::from(Square::B1) | Bitboard::from(Square::G1),
                    Bitboard::from(Square::B8) | Bitboard::from(Square::G8),
                );
                let bishop_home = color.fold_wb(
                    Bitboard::from(Square::C1) | Bitboard::from(Square::F1),
                    Bitboard::from(Square::C8) | Bitboard::from(Square::F8),
                );
                (board.by_color(color) & (board.by_role(Role::Knight) | board.by_role(Role::Bishop)) & (knight_home | bishop_home))
                    .into_iter()
                    .filter_map(|sq| board_to_piece_ref(board, sq))
                    .collect()
            };
            results.push(DevelopmentInfo {
                color: Side::from(color),
                undeveloped_pieces: pieces,
                space_advantage: if color.is_white() { space } else { -space },
            });
        }
    }
    results
}

fn tactical_score(board: &shakmaty::Board, us: Color, phase: u8) -> (GroupValue, TacticalRaw) {
    let them = us.other();
    let (pins_us,    pin_ex_us)    = detect_pins(board, us);
    let (pins_them,  pin_ex_them)  = detect_pins(board, them);
    let (forks_us,   fork_ex_us)   = detect_forks(board, us);
    let (forks_them, fork_ex_them) = detect_forks(board, them);
    let (skewers_us,   skew_ex_us)   = detect_skewers(board, us);
    let (skewers_them, skew_ex_them) = detect_skewers(board, them);
    let (disc_us,   disc_ex_us)   = detect_discovered(board, us);
    let (disc_them, disc_ex_them) = detect_discovered(board, them);

    // Tactical base weights (from configurable WEIGHTS)
    let phase_factor_num = i64::from(phase) + 8; // numerator
    let w_cfg = weights();

    let w_pins = w_cfg.tactical_base_pins * phase_factor_num / w_cfg.phase_factor_den;
    let w_forks = w_cfg.tactical_base_forks * phase_factor_num / w_cfg.phase_factor_den;
    let w_skewers = w_cfg.tactical_base_skewers * phase_factor_num / w_cfg.phase_factor_den;
    let w_disc = w_cfg.tactical_base_disc * phase_factor_num / w_cfg.phase_factor_den;

    let total_us = pins_us * w_pins + forks_us * w_forks + skewers_us * w_skewers + disc_us * w_disc;
    let total_them = pins_them * w_pins + forks_them * w_forks + skewers_them * w_skewers + disc_them * w_disc;
    let blended = total_us - total_them;
    let (mg, eg) = phase_split(blended, phase);

    let mut terms = serde_json::Map::new();
    terms.insert("pins_us".into(), serde_json::Value::from(pins_us));
    terms.insert("pins_them".into(), serde_json::Value::from(pins_them));
    terms.insert("forks_us".into(), serde_json::Value::from(forks_us));
    terms.insert("forks_them".into(), serde_json::Value::from(forks_them));
    terms.insert("skewers_us".into(), serde_json::Value::from(skewers_us));
    terms.insert("skewers_them".into(), serde_json::Value::from(skewers_them));
    terms.insert("discovered_us".into(), serde_json::Value::from(disc_us));
    terms.insert("discovered_them".into(), serde_json::Value::from(disc_them));
    terms.insert("total_us".into(), serde_json::Value::from(total_us));
    terms.insert("total_them".into(), serde_json::Value::from(total_them));

    // Examples (all collected) — insert plural arrays and a singular first-example for compatibility
    // Forks
    if !fork_ex_us.is_empty() {
        let arr: Vec<serde_json::Value> = fork_ex_us
            .iter()
            .map(|(att, targets)| {
                let attacker = piece_square_name(board, *att);
                let tnames: Vec<serde_json::Value> = targets.iter().map(|&t| serde_json::Value::from(piece_square_name(board, t))).collect();
                let mut map = serde_json::Map::new();
                map.insert("attacker".into(), serde_json::Value::from(attacker));
                map.insert("targets".into(), serde_json::Value::Array(tnames));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("fork_examples_us".into(), serde_json::Value::Array(arr.clone()))
            ;
        if let Some(first) = arr.first() {
            terms.insert("fork_example_us".into(), first.clone());
        }
    }
    if !fork_ex_them.is_empty() {
        let arr: Vec<serde_json::Value> = fork_ex_them
            .iter()
            .map(|(att, targets)| {
                let attacker = piece_square_name(board, *att);
                let tnames: Vec<serde_json::Value> = targets.iter().map(|&t| serde_json::Value::from(piece_square_name(board, t))).collect();
                let mut map = serde_json::Map::new();
                map.insert("attacker".into(), serde_json::Value::from(attacker));
                map.insert("targets".into(), serde_json::Value::Array(tnames));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("fork_examples_them".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("fork_example_them".into(), first.clone());
        }
    }

    // Skewers
    if !skew_ex_us.is_empty() {
        let arr: Vec<serde_json::Value> = skew_ex_us
            .iter()
            .map(|(att, f, b)| {
                let mut map = serde_json::Map::new();
                map.insert("attacker".into(), serde_json::Value::from(piece_square_name(board, *att)));
                map.insert("front".into(), serde_json::Value::from(piece_square_name(board, *f)));
                map.insert("back".into(), serde_json::Value::from(piece_square_name(board, *b)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("skewer_examples_us".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("skewer_example_us".into(), first.clone());
        }
    }
    if !skew_ex_them.is_empty() {
        let arr: Vec<serde_json::Value> = skew_ex_them
            .iter()
            .map(|(att, f, b)| {
                let mut map = serde_json::Map::new();
                map.insert("attacker".into(), serde_json::Value::from(piece_square_name(board, *att)));
                map.insert("front".into(), serde_json::Value::from(piece_square_name(board, *f)));
                map.insert("back".into(), serde_json::Value::from(piece_square_name(board, *b)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("skewer_examples_them".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("skewer_example_them".into(), first.clone());
        }
    }

    // Pins
    if !pin_ex_us.is_empty() {
        let arr: Vec<serde_json::Value> = pin_ex_us
            .iter()
            .map(|(pinner, pinned, king)| {
                let mut map = serde_json::Map::new();
                map.insert("pinner".into(), serde_json::Value::from(piece_square_name(board, *pinner)));
                map.insert("pinned".into(), serde_json::Value::from(piece_square_name(board, *pinned)));
                map.insert("king".into(), serde_json::Value::from(piece_square_name(board, *king)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("pin_examples_us".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("pin_example_us".into(), first.clone());
        }
    }
    if !pin_ex_them.is_empty() {
        let arr: Vec<serde_json::Value> = pin_ex_them
            .iter()
            .map(|(pinner, pinned, king)| {
                let mut map = serde_json::Map::new();
                map.insert("pinner".into(), serde_json::Value::from(piece_square_name(board, *pinner)));
                map.insert("pinned".into(), serde_json::Value::from(piece_square_name(board, *pinned)));
                map.insert("king".into(), serde_json::Value::from(piece_square_name(board, *king)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("pin_examples_them".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("pin_example_them".into(), first.clone());
        }
    }

    // Discovered
    if !disc_ex_us.is_empty() {
        let arr: Vec<serde_json::Value> = disc_ex_us
            .iter()
            .map(|(blocker, slider, target)| {
                let mut map = serde_json::Map::new();
                map.insert("blocker".into(), serde_json::Value::from(piece_square_name(board, *blocker)));
                map.insert("slider".into(), serde_json::Value::from(piece_square_name(board, *slider)));
                map.insert("target".into(), serde_json::Value::from(piece_square_name(board, *target)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("discovered_examples_us".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("discovered_example_us".into(), first.clone());
        }
    }
    if !disc_ex_them.is_empty() {
        let arr: Vec<serde_json::Value> = disc_ex_them
            .iter()
            .map(|(blocker, slider, target)| {
                let mut map = serde_json::Map::new();
                map.insert("blocker".into(), serde_json::Value::from(piece_square_name(board, *blocker)));
                map.insert("slider".into(), serde_json::Value::from(piece_square_name(board, *slider)));
                map.insert("target".into(), serde_json::Value::from(piece_square_name(board, *target)));
                serde_json::Value::Object(map)
            })
            .collect();
        terms.insert("discovered_examples_them".into(), serde_json::Value::Array(arr.clone()));
        if let Some(first) = arr.first() {
            terms.insert("discovered_example_them".into(), first.clone());
        }
    }

    let raw = TacticalRaw {
        pin_ex_us, pin_ex_them,
        skew_ex_us, skew_ex_them,
        disc_ex_us, disc_ex_them,
    };
    (GroupValue { mg, eg, blended, terms }, raw)
}

fn detect_outposts(board: &shakmaty::Board, graph: &ThreatGraph, color: Color) -> (i64, Vec<(Square, Role, Square)>) {
    // Detect outposts: own Knight/Bishop on an advanced square that is not attackable by opponent pawns
    // and is supported by an own pawn (preferred). Returns (count, vec![(sq, role, support_sq), ...]).
    // Reads `graph.attackers` (built once from ThreatGraph::attackers_to) instead of a
    // separate pawn_attack_mask/attacks_to call per square — the same "whose continuity
    // is this square in" question hanging-piece detection already answers from this
    // shared substrate.
    let mut count = 0_i64;
    let mut examples: Vec<(Square, Role, Square)> = Vec::new();
    let them = color.other();

    let pieces = board.by_color(color) & (board.by_role(Role::Knight) | board.by_role(Role::Bishop));

    for sq in pieces {
        // advanced condition: for white rank >= 4 (0-indexed >=3), for black rank <= 4
        let rank_idx = u32::from(sq.rank());
        if color.is_white() {
            if rank_idx < 3 {
                continue;
            }
        } else {
            if rank_idx > 4 {
                continue;
            }
        }

        // must not be attackable by enemy pawns
        let enemy_pawn_attackers = graph.attackers(sq, them) & board.by_role(Role::Pawn);
        if enemy_pawn_attackers.any() {
            continue;
        }

        // check if supported by own pawn (preferred)
        let supported_by_pawn = (graph.attackers(sq, color) & board.by_role(Role::Pawn)).into_iter().next();

        if let Some(p_support) = supported_by_pawn {
            count += 1;
            if let Some(piece) = board.piece_at(sq) {
                examples.push((sq, piece.role, p_support));
            }
        } else if graph.attackers(sq, color).any() {
            // as a fallback, allow squares defended by other pieces
            count += 1;
            if let Some(piece) = board.piece_at(sq) {
                examples.push((sq, piece.role, Square::E1));
                // support square unknown; placeholder E1 (we will prefer pawn support in examples)
            }
        }
    }

    (count, examples)
}

/// Center control score: presence and attacks on D4, D5, E4, E5 (centipawns).
fn center_control_score(board: &shakmaty::Board, color: Color) -> i64 {
    let center = [Square::D4, Square::D5, Square::E4, Square::E5];
    let occupied = board.occupied();
    let mut score = 0_i64;

    for &sq in &center {
        // Occupying a center square is worth more than just attacking it
        if (board.by_color(color) & Bitboard::from(sq)).any() {
            score += 20;
        }
        // Count attackers from this color (using attacks_to from the color's perspective)
        let attackers = board.attacks_to(sq, color, occupied);
        score += attackers.count() as i64 * 8;
    }
    score
}

/// Piece coordination: count own piece pairs within Manhattan distance ≤ 2.
fn piece_coordination_score(board: &shakmaty::Board, color: Color) -> i64 {
    let pieces = board.by_color(color);
    let mut score = 0_i64;
    let squares: Vec<Square> = pieces.into_iter().collect();
    for i in 0..squares.len() {
        for j in (i + 1)..squares.len() {
            let a = squares[i];
            let b = squares[j];
            let file_diff = (a.file() as i32 - b.file() as i32).unsigned_abs() as i64;
            let rank_diff = (a.rank() as i32 - b.rank() as i32).unsigned_abs() as i64;
            let manhattan = file_diff + rank_diff;
            if manhattan <= 2 {
                score += 5;
            }
        }
    }
    score
}

/// Tactical pressure: sliding pieces (R/Q on rank/file, B/Q on diagonal) aligned with enemy king.
fn tactical_pressure_score(board: &shakmaty::Board, color: Color) -> i64 {
    let enemy_king = match board.king_of(color.other()) {
        Some(sq) => sq,
        None => return 0,
    };
    let occupied = board.occupied();
    let mut score = 0_i64;

    // Rooks/Queens aligned on rank or file with enemy king
    for sq in board.by_color(color) & (board.by_role(Role::Rook) | board.by_role(Role::Queen)) {
        let reachable = attacks::rook_attacks(sq, occupied);
        if (reachable & Bitboard::from(enemy_king)).any() {
            score += 15;
        }
    }

    // Bishops/Queens aligned on diagonal with enemy king
    for sq in board.by_color(color) & (board.by_role(Role::Bishop) | board.by_role(Role::Queen)) {
        let reachable = attacks::bishop_attacks(sq, occupied);
        if (reachable & Bitboard::from(enemy_king)).any() {
            score += 12;
        }
    }

    score
}

/// Combined vector_features group (center_control + piece_coordination + tactical_pressure).
fn vector_features_score(board: &shakmaty::Board, color: Color, phase: u8) -> GroupValue {
    let cc_us = center_control_score(board, color);
    let cc_them = center_control_score(board, color.other());
    let pc_us = piece_coordination_score(board, color);
    let pc_them = piece_coordination_score(board, color.other());
    let tp_us = tactical_pressure_score(board, color);
    let tp_them = tactical_pressure_score(board, color.other());

    let center = cc_us - cc_them;
    let coordination = pc_us - pc_them;
    let pressure = tp_us - tp_them;
    let total = center + coordination + pressure;
    let (mg, eg) = phase_split(total, phase);

    let mut terms = serde_json::Map::new();
    terms.insert("center_control_us".into(), serde_json::Value::from(cc_us));
    terms.insert(
        "center_control_them".into(),
        serde_json::Value::from(cc_them),
    );
    terms.insert(
        "piece_coordination_us".into(),
        serde_json::Value::from(pc_us),
    );
    terms.insert(
        "piece_coordination_them".into(),
        serde_json::Value::from(pc_them),
    );
    terms.insert(
        "tactical_pressure_us".into(),
        serde_json::Value::from(tp_us),
    );
    terms.insert(
        "tactical_pressure_them".into(),
        serde_json::Value::from(tp_them),
    );

    GroupValue {
        mg,
        eg,
        blended: blend(mg, eg, phase),
        terms,
    }
}

/// Strategic evaluation: initiative, king-attack, safety, coordination.
/// Ported from chess-vector-engine/src/strategic_evaluator.rs (shakmaty translation).
fn strategic_score(
    board: &shakmaty::Board,
    us: Color,
    legal_move_count: usize,
    phase: u8,
) -> GroupValue {
    let them = us.other();
    let occupied = board.occupied();

    // --- initiative: mobility advantage + center control ---
    // Approximate opponent mobility by counting attacks on all squares from their pieces.
    let mut opp_mobility = 0i64;
    for sq in board.by_color(them) {
        opp_mobility += board.attacks_from(sq).count() as i64;
    }
    let our_moves = legal_move_count as i64;
    let mobility_advantage = our_moves - (opp_mobility / 3).max(1);
    let center = [Square::D4, Square::D5, Square::E4, Square::E5];
    let center_ctrl = center
        .iter()
        .filter(|&&sq| (board.by_color(us) & Bitboard::from(sq)).any())
        .count() as i64;
    let initiative = mobility_advantage * 2 + center_ctrl * 10;

    // --- attacking_bonus: pieces threatening enemy king area ---
    let enemy_king_area = board
        .king_of(them)
        .map(|ksq| attacks::king_attacks(ksq) | Bitboard::from(ksq))
        .unwrap_or(Bitboard::EMPTY);

    let mut attacking_pieces = 0i64;
    let mut controlled_king_sq = 0i64;
    for sq in board.by_color(us) {
        let piece_attacks = board.attacks_from(sq);
        if (piece_attacks & enemy_king_area).any() {
            attacking_pieces += 1;
        }
    }
    for sq in enemy_king_area {
        if board.attacks_to(sq, us, occupied).any() {
            controlled_king_sq += 1;
        }
    }
    let attacking_bonus = attacking_pieces * 10 + controlled_king_sq * 8;

    // --- safety_penalty: hanging our pieces + king exposure ---
    let mut hanging = 0i64;
    for sq in board.by_color(us) {
        let attacked = board.attacks_to(sq, them, occupied).any();
        if attacked {
            let defended = board
                .attacks_to(sq, us, occupied)
                .into_iter()
                .filter(|&def| def != sq)
                .count();
            if defended == 0 {
                hanging += 1;
            }
        }
    }
    let king_exposed = board
        .king_of(us)
        .map(|ksq| board.attacks_to(ksq, them, occupied).any())
        .unwrap_or(false);
    let safety_penalty = hanging * 40 + if king_exposed { 80 } else { 0 };

    // --- coordination_bonus: our pieces within attack range of each other ---
    let our_squares: Vec<Square> = board.by_color(us).into_iter().collect();
    let mut coordination = 0i64;
    for i in 0..our_squares.len() {
        for j in (i + 1)..our_squares.len() {
            let a = our_squares[i];
            let b = our_squares[j];
            // Pieces reachable from each other (one step of sliding/leaper attacks)
            let a_atk = board.attacks_from(a);
            if (a_atk & Bitboard::from(b)).any() {
                coordination += 5;
            }
        }
    }

    let total = initiative + attacking_bonus + coordination - safety_penalty;
    let (mg, eg) = phase_split(total, phase);

    let mut terms = serde_json::Map::new();
    terms.insert("initiative".into(), serde_json::Value::from(initiative));
    terms.insert(
        "attacking_bonus".into(),
        serde_json::Value::from(attacking_bonus),
    );
    terms.insert(
        "attacking_pieces".into(),
        serde_json::Value::from(attacking_pieces),
    );
    terms.insert(
        "controlled_king_sq".into(),
        serde_json::Value::from(controlled_king_sq),
    );
    terms.insert(
        "safety_penalty".into(),
        serde_json::Value::from(safety_penalty),
    );
    terms.insert("hanging".into(), serde_json::Value::from(hanging));
    terms.insert("king_exposed".into(), serde_json::Value::from(king_exposed));
    terms.insert("coordination".into(), serde_json::Value::from(coordination));

    GroupValue {
        mg,
        eg,
        blended: blend(mg, eg, phase),
        terms,
    }
}

/// Compute the chaos coefficient: how tactically unstable the position is.
/// Ranges from 0.0 (clean positional game) to 1.0 (multiple immediate threats).
/// Digital-switch sensors (forks, pins, checks) fire → chaos rises.
/// This gates the higher-tier analog sensors through the attenuation matrix.
fn chaos_coefficient(g: &EvalGroups) -> f64 {
    let t = &g.tactical.terms;
    let s = &g.strategic.terms;
    let ks = &g.king_safety.terms;

    let term_i64 = |key: &str| -> i64 {
        t.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
    };

    let forks = term_i64("forks_us") + term_i64("forks_them");
    let pins = term_i64("pins_us") + term_i64("pins_them");
    let skewers = term_i64("skewers_us") + term_i64("skewers_them");
    // discovered attacks fire too broadly (even in opening) — excluded from chaos

    let in_check = ks.get("in_check").and_then(|v| v.as_bool()).unwrap_or(false);
    let hanging = s.get("hanging").and_then(|v| v.as_i64()).unwrap_or(0);
    let king_exposed = s.get("king_exposed")
        .and_then(|v| v.as_bool()).unwrap_or(false);

    let threat_count = (forks + pins + skewers + hanging) as f64;
    let chaos_base = threat_count * 0.15;
    let chaos_bonus = if in_check { 0.4 } else { 0.0 }
                    + if king_exposed { 0.3 } else { 0.0 };

    (chaos_base + chaos_bonus).min(1.0)
}

fn compute_aggregates(g: &mut EvalGroups) {
    use crate::eval::concepts::{SensorTier, attenuation};

    let chaos = chaos_coefficient(g);
    let chaos_i64 = (chaos * 100.0) as i64; // store as 0-100

    // Material: always active (Survival tier, attenuation = 1.0)
    let material = g.material.blended;
    g.material_total = ScalarValue { value: material, factor: chaos_i64 };

    // Positional: structural components with tier-specific attenuation
    fn compute_attenuated(value: i64, chaos: f64, tier: SensorTier) -> i64 {
        let att = attenuation(tier, chaos);
        (value as f64 * att).round() as i64
    }

    // Pawn structure and passed pawns: Positional tier (half-attenuated)
    let pawn = compute_attenuated(g.pawn_structure.blended, chaos, SensorTier::Positional);
    // Piece activity (outposts, rooks): Positional tier
    let activity = compute_attenuated(g.piece_activity.blended, chaos, SensorTier::Positional);
    // King safety: Positional tier 
    let king_safe = compute_attenuated(g.king_safety.blended, chaos, SensorTier::Positional);
    // Development: Positional tier
    let dev = compute_attenuated(g.development.blended, chaos, SensorTier::Positional);
    // Vector features (center control, coordination): Positional tier
    let vectors = compute_attenuated(g.vector_features.blended, chaos, SensorTier::Positional);
    // Strategic (initiative, minority attack): Strategic tier (fully attenuated)
    let strategic = compute_attenuated(g.strategic.blended, chaos, SensorTier::Strategic);
    // Passed pawns: Positional tier
    let passed = compute_attenuated(g.passed_pawns.blended, chaos, SensorTier::Positional);
    // Scaling and drawishness are meta-concepts, not attenuated
    let scaling = g.scaling.value;
    let drawishness = g.drawishness.value;
    let override_ = g.override_.value;

    let attenuated_positional = pawn + activity + king_safe + passed + dev + vectors + strategic
        + scaling + drawishness + override_;
    g.positional_total = ScalarValue { value: attenuated_positional, factor: chaos_i64 };

    // Tactical: Threat tier — always active (attenuation = 1.0)
    let tactical = g.tactical.blended;
    g.tactical_total = ScalarValue { value: tactical, factor: 0 };
}

fn sum_groups(groups: &EvalGroups) -> i64 {
    groups.material.blended
        + groups.pawn_structure.blended
        + groups.piece_activity.blended
        + groups.king_safety.blended
        + groups.passed_pawns.blended
        + groups.development.blended
        + groups.vector_features.blended
        + groups.strategic.blended
        + groups.tactical.blended
        + groups.scaling.value
        + groups.drawishness.value
        + groups.override_.value
}

fn win_chance_scale(board: &shakmaty::Board, _mi: &GroupValue) -> i64 {
    let count_white = piece_count(board, Color::White, Role::Pawn);
    let count_black = piece_count(board, Color::Black, Role::Pawn);
    let pawn_cnt = count_white.max(count_black);
    let wpieces = piece_count(board, Color::White, Role::Knight)
        + piece_count(board, Color::White, Role::Bishop)
        + piece_count(board, Color::White, Role::Rook)
        + piece_count(board, Color::White, Role::Queen);
    let bpieces = piece_count(board, Color::Black, Role::Knight)
        + piece_count(board, Color::Black, Role::Bishop)
        + piece_count(board, Color::Black, Role::Rook)
        + piece_count(board, Color::Black, Role::Queen);

    if wpieces == 0 && bpieces == 0 {
        return 128;
    }

    if piece_count(board, Color::White, Role::Queen) + piece_count(board, Color::Black, Role::Queen)
        == 2
    {
        return 112 + pawn_cnt.min(8);
    }

    if piece_count(board, Color::White, Role::Rook) + piece_count(board, Color::Black, Role::Rook)
        == 2
    {
        return 96 + pawn_cnt.min(8) * 2;
    }

    if piece_count(board, Color::White, Role::Bishop)
        + piece_count(board, Color::Black, Role::Bishop)
        == 2
    {
        return 88 + pawn_cnt.min(8) * 2;
    }

    128
}

fn draw_weight(board: &shakmaty::Board, color: Color) -> i64 {
    let our = board.by_color(color) & board.by_role(Role::Pawn);
    let their = board.by_color(color.other()) & board.by_role(Role::Pawn);
    let mut open = 0_i64;
    let mut all = 0_i64;

    for file in 0..8 {
        let file_mask = Bitboard::from(File::new(file));
        let has_our = (our & file_mask).any();
        let has_their = (their & file_mask).any();
        if has_our {
            all += 1;
        }
        if has_our && !has_their {
            open += 1;
        }
    }

    let open_file_mult = [6_i64, 5, 4, 3, 2, 1, 0, 0, 0];
    let pawn_count_mult = [6_i64, 5, 4, 3, 2, 1, 0, 0, 0];
    open_file_mult[open as usize] * pawn_count_mult[all as usize]
}

/// Expects `chess` already canonical (White-to-move) — every score this
/// produces is computed relative to `chess.turn()` as "us," which is only
/// meaningful if that's actually the side to move in the real game. Current
/// callers uphold this two ways: `analyze_fen_with_engine_score` calls
/// `normalize_for_eval` first; `coach_derive_cmd.rs`'s direct calls read
/// `positions.fen`, which is already canonical by construction (see
/// FINDINGS.md's "Canonical position identity" section). A future caller that
/// evaluates a real, un-normalized position directly would get silently
/// wrong material/pawn/king-safety/development scores — nothing here
/// enforces the precondition, it just has to be upheld by the caller.
pub fn compute_groups(chess: &Chess, phase: u8, legal_move_count: usize) -> EvalGroups {
    let board = chess.board();
    let us = chess.turn();
    let them = us.other();
    // Built once, reused for detect_outposts's shared-substrate query below
    // and in_check (the same primitive shakmaty's own is_check()/checkers()
    // are built from, see ThreatGraph::is_in_check) instead of a second,
    // separate shakmaty call. build_sensor_report constructs its own,
    // separate graph later — these two functions don't yet share one across
    // a single evaluation (see FINDINGS.md).
    let graph = ThreatGraph::build(chess);
    let in_check = graph.is_in_check(us);

    let w = weights();
    let material = material_score(board, us, phase);
    let (pawn_us, pawn_us_terms) = pawn_structure_score(board, us, phase);
    let (pawn_them, pawn_them_terms) = pawn_structure_score(board, them, phase);
    let (passed_us, passed_us_terms) = passed_pawn_score(board, us);
    let (passed_them, passed_them_terms) = passed_pawn_score(board, them);
    let dev_diff = development_score(board, us) - development_score(board, them);
    let king_safety = king_safety_score(board, &graph, us, in_check, phase)
        - king_safety_score(board, &graph, them, false, phase);

    let mut pawn_structure = GroupValue::default();
    let pawn_total = pawn_us - pawn_them;
    let (pawn_mg, pawn_eg) = phase_split(pawn_total, biased_phase(phase, w.phase_bias_pawn_structure));
    pawn_structure.mg = pawn_mg;
    pawn_structure.eg = pawn_eg;
    pawn_structure.blended = blend(pawn_mg, pawn_eg, biased_phase(phase, w.phase_bias_pawn_structure));
    pawn_structure.terms = pawn_us_terms.clone();
    pawn_structure
        .terms
        .insert("opp_total".into(), serde_json::Value::from(pawn_them));
    pawn_structure.terms.insert(
        "opp_terms".into(),
        serde_json::Value::Object(pawn_them_terms.clone()),
    );

    // Synthesize per-color terms for concepts.rs extract_concepts
    for key in &["isolated", "doubled", "candidate", "weak", "chain", "passed", "islands",
                  "majority_queenside", "majority_center", "majority_kingside",
                  "minority_attack", "minority_attack_strength", "pawn_breaks"] {
        if let Some(v) = pawn_us_terms.get(*key) {
            pawn_structure.terms.insert(format!("{}_us", key), v.clone());
        }
        if let Some(v) = pawn_them_terms.get(*key) {
            pawn_structure.terms.insert(format!("{}_them", key), v.clone());
        }
    }
    // Synthesize aggregate majority counts
    let majority_us: i64 = ["majority_queenside", "majority_center", "majority_kingside"]
        .iter()
        .filter_map(|k| pawn_us_terms.get(*k).and_then(|v| v.as_i64()))
        .sum();
    let majority_them: i64 = ["majority_queenside", "majority_center", "majority_kingside"]
        .iter()
        .filter_map(|k| pawn_them_terms.get(*k).and_then(|v| v.as_i64()))
        .sum();
    pawn_structure.terms.insert("majority_us".into(), serde_json::Value::from(majority_us));
    pawn_structure.terms.insert("majority_them".into(), serde_json::Value::from(majority_them));

    let mut piece_activity = GroupValue::default();
    let (piece_us, piece_us_terms) = piece_activity_score(
        board,
        &graph,
        us,
        phase,
        !pawn_attack_mask(board, them),
        king_ring(board, us),
    );
    let (piece_them, piece_them_terms) = piece_activity_score(
        board,
        &graph,
        them,
        phase,
        !pawn_attack_mask(board, us),
        king_ring(board, them),
    );
    let piece_total = piece_us - piece_them;
    let (piece_mg, piece_eg) = phase_split(piece_total, biased_phase(phase, w.phase_bias_piece_activity));
    piece_activity.mg = piece_mg;
    piece_activity.eg = piece_eg;
    piece_activity.blended = blend(piece_mg, piece_eg, biased_phase(phase, w.phase_bias_piece_activity));
    piece_activity.terms = piece_us_terms;
    piece_activity.terms.insert(
        "legal_move_count".into(),
        serde_json::Value::from(legal_move_count as i64),
    );
    piece_activity
        .terms
        .insert("opp_total".into(), serde_json::Value::from(piece_them));
    piece_activity.terms.insert(
        "opp_terms".into(),
        serde_json::Value::Object(piece_them_terms),
    );

    // Outpost detection: add as a piece activity term, with example context
    let (outposts_us, out_ex_us) = detect_outposts(board, &graph, us);
    let (outposts_them, out_ex_them) = detect_outposts(board, &graph, them);
    let outpost_delta = outposts_us * w.outpost_weight - outposts_them * w.outpost_weight;
    piece_activity.blended += outpost_delta;
    piece_activity.terms.insert("outposts_us".into(), serde_json::Value::from(outposts_us));
    piece_activity.terms.insert("outposts_them".into(), serde_json::Value::from(outposts_them));

    // Insert all outpost examples (plural) and singular-first for compatibility
    if !out_ex_us.is_empty() {
        let arr: Vec<serde_json::Value> = out_ex_us
            .iter()
            .map(|(sq, role, support)| {
                let mut map = serde_json::Map::new();
                map.insert("square".into(), serde_json::Value::from(sq.to_string()));
                map.insert(
                    "role".into(),
                    serde_json::Value::from(match role { Role::Knight => "N", Role::Bishop => "B", _ => "?" }),
                );
                map.insert("support".into(), serde_json::Value::from(piece_square_name(board, *support)));
                serde_json::Value::Object(map)
            })
            .collect();
        piece_activity.terms.insert("outpost_examples_us".into(), serde_json::Value::Array(arr.clone()));
        if let Some((sq, role, support)) = out_ex_us.first() {
            piece_activity.terms.insert("outpost_example_us".into(), serde_json::Value::from(format!("{} on {} supported by {}", match role { Role::Knight => "N", Role::Bishop => "B", _ => "?" }, sq, piece_square_name(board, *support))));
        }
    }
    if !out_ex_them.is_empty() {
        let arr: Vec<serde_json::Value> = out_ex_them
            .iter()
            .map(|(sq, role, support)| {
                let mut map = serde_json::Map::new();
                map.insert("square".into(), serde_json::Value::from(sq.to_string()));
                map.insert(
                    "role".into(),
                    serde_json::Value::from(match role { Role::Knight => "N", Role::Bishop => "B", _ => "?" }),
                );
                map.insert("support".into(), serde_json::Value::from(piece_square_name(board, *support)));
                serde_json::Value::Object(map)
            })
            .collect();
        piece_activity.terms.insert("outpost_examples_them".into(), serde_json::Value::Array(arr.clone()));
        if let Some((sq, role, support)) = out_ex_them.first() {
            piece_activity.terms.insert("outpost_example_them".into(), serde_json::Value::from(format!("{} on {} supported by {}", match role { Role::Knight => "N", Role::Bishop => "B", _ => "?" }, sq, piece_square_name(board, *support))));
        }
    }

    let mut king_safety_group = GroupValue::default();
    let (king_mg, king_eg) = phase_split(king_safety, biased_phase(phase, w.phase_bias_king_safety));
    king_safety_group.mg = king_mg;
    king_safety_group.eg = king_eg;
    // augment king safety with king tropism (GUESS weights)
    let tropism_us = king_tropism_score(board, us);
    let tropism_them = king_tropism_score(board, them);
    king_safety_group.blended = blend(king_mg, king_eg, biased_phase(phase, w.phase_bias_king_safety))
        + (tropism_us - tropism_them);
    king_safety_group
        .terms
        .insert("in_check".into(), serde_json::Value::from(in_check));
    king_safety_group
        .terms
        .insert("tropism_us".into(), serde_json::Value::from(tropism_us));
    king_safety_group
        .terms
        .insert("tropism_them".into(), serde_json::Value::from(tropism_them));

    let mut passed_pawns = GroupValue::default();
    let passed_total = passed_us - passed_them;
    let (passed_mg, passed_eg) = phase_split(passed_total, biased_phase(phase, w.phase_bias_passed_pawns));
    passed_pawns.mg = passed_mg;
    passed_pawns.eg = passed_eg;
    passed_pawns.blended = blend(passed_mg, passed_eg, biased_phase(phase, w.phase_bias_passed_pawns));
    passed_pawns.terms = passed_us_terms;
    passed_pawns
        .terms
        .insert("opp_total".into(), serde_json::Value::from(passed_them));
    passed_pawns.terms.insert(
        "opp_terms".into(),
        serde_json::Value::Object(passed_them_terms),
    );

    let mut development = GroupValue::default();
    let dev_space_us = development_space_score(board, &graph, us, phase);
    let dev_space_them = development_space_score(board, &graph, them, phase);
    let dev_total = dev_diff + (dev_space_us - dev_space_them);
    let (dev_mg, dev_eg) = phase_split(dev_total, biased_phase(phase, w.phase_bias_development));
    development.mg = dev_mg;
    development.eg = dev_eg;
    development.blended = blend(dev_mg, dev_eg, biased_phase(phase, w.phase_bias_development));
    development
        .terms
        .insert("development_diff".into(), serde_json::Value::from(dev_diff));
    development
        .terms
        .insert("space_us".into(), serde_json::Value::from(dev_space_us));
    development
        .terms
        .insert("space_them".into(), serde_json::Value::from(dev_space_them));

    let vector_features = vector_features_score(board, us, biased_phase(phase, w.phase_bias_vector_features));
    let strategic = strategic_score(board, us, legal_move_count, biased_phase(phase, w.phase_bias_strategic));
    let (tactical, tactical_raw) = tactical_score(board, us, phase);

    let scaling_factor = win_chance_scale(board, &material);

    let mut groups = EvalGroups {
        material_total: ScalarValue::default(),
        positional_total: ScalarValue::default(),
        tactical_total: ScalarValue::default(),
        material,
        pawn_structure,
        piece_activity,
        king_safety: king_safety_group,
        passed_pawns,
        development,
        vector_features,
        strategic,
        tactical,
        tactical_raw,
        scaling: ScalarValue {
            value: 0,
            factor: scaling_factor,
        },
        drawishness: ScalarValue::default(),
        override_: ScalarValue::default(),
    };

    let linear_total = sum_groups(&groups);
    groups.scaling.value = if linear_total > 0 {
        linear_total * (groups.scaling.factor - 128) / 128
    } else if linear_total < 0 {
        linear_total * (128 - groups.scaling.factor) / 128
    } else {
        0
    };
    let draw_delta = if linear_total > 0 {
        -(draw_weight(board, Color::White) * linear_total.min(256)) / 64
    } else if linear_total < 0 {
        (draw_weight(board, Color::Black) * (-linear_total).min(256)) / 64
    } else {
        0
    };
    groups.drawishness.value = draw_delta;
    compute_aggregates(&mut groups);
    groups
}

fn get_term_i64(terms: &serde_json::Map<String, serde_json::Value>, key: &str) -> i64 {
    terms.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

fn build_material_balance(groups: &EvalGroups) -> Option<MaterialBalance> {
    let t = &groups.material.terms;
    let white = PieceCounts {
        queens: get_term_i64(t, "white_queens") as u8,
        rooks: get_term_i64(t, "white_rooks") as u8,
        bishops: get_term_i64(t, "white_bishops") as u8,
        knights: get_term_i64(t, "white_knights") as u8,
        pawns: get_term_i64(t, "white_pawns") as u8,
    };
    let black = PieceCounts {
        queens: get_term_i64(t, "black_queens") as u8,
        rooks: get_term_i64(t, "black_rooks") as u8,
        bishops: get_term_i64(t, "black_bishops") as u8,
        knights: get_term_i64(t, "black_knights") as u8,
        pawns: get_term_i64(t, "black_pawns") as u8,
    };
    let bishop_pair_white = get_term_i64(t, "white_bishops") >= 2;
    let bishop_pair_black = get_term_i64(t, "black_bishops") >= 2;
    let centipawns = groups.material_total.value;
    Some(MaterialBalance { white, black, centipawns, bishop_pair_white, bishop_pair_black })
}

pub fn build_sensor_report(board: &shakmaty::Board, fen: &str, groups: &EvalGroups, chess: &Chess, phase: u8, player_elo: Option<i32>) -> SensorReport {
    let us = chess.turn();
    let them = us.other();

    // Brute-force one-ply search, not a geometric pattern detector like
    // everything else here: does any legal move deliver checkmate. Computed
    // early and folded into `partial` below (not patched onto the return
    // value afterward) so it's part of the same data `extract_concepts`
    // sees when this function computes `gated_issues` itself, and so every
    // caller of this function gets it for free — previously this only ran
    // in `analyze_fen_with_engine_score`, so the two direct callers in
    // coach_derive_cmd.rs silently never saw it, and even
    // `analyze_fen_with_engine_score`'s own `gated_issues` never included it
    // either, since gated_issues was already computed before this got
    // patched on.
    let mate_in_1_exists = chess.legal_moves().iter().any(|m| {
        let mut c = chess.clone();
        c.play_unchecked(*m);
        c.is_checkmate()
    });

    // Build the unified threat graph — one pass over the board
    let graph = ThreatGraph::build(chess);

    // Forks with SEE: graph-derived, includes material consequence
    let mut evaluated_forks = graph.find_forks(us);
    evaluated_forks.extend(graph.find_forks(them));


    // Tactical report — reuse raw examples cached in groups.tactical_raw by tactical_score;
    // no need to re-run the detectors.
    let raw = &groups.tactical_raw;
    // Computed before the struct literal (not inline like the others) —
    // find_false_defense below needs it as an input, it's not just another
    // independent field.
    let pins: Vec<Pin> = { let mut v = pins_to_typed(board, &raw.pin_ex_us); v.extend(pins_to_typed(board, &raw.pin_ex_them)); v };
    // Computed before the struct literal too — find_false_safety below needs
    // both pins and overloaded as input, the same "cross-reference of
    // already-known facts" shape as false_defense's use of pins alone.
    let overloaded: Vec<Overloaded> = { let mut v = graph.find_overloaded(us); v.extend(graph.find_overloaded(them)); v };
    let false_safety: Vec<FalseSafety> = { let mut v = graph.find_false_safety(us, &pins, &overloaded); v.extend(graph.find_false_safety(them, &pins, &overloaded)); v };
    let tactical = TacticalReport {
        forks: evaluated_forks.iter().map(|ef| Fork {
            attacker: ef.attacker.clone(),
            targets: ef.targets.clone(),
            hangs: ef.hangs.clone(),
            see_cp: ef.see_cp,
            consequence: ef.consequence.clone(),
        }).collect(),
        skewers:    { let mut v = skewers_to_typed(board, &raw.skew_ex_us); v.extend(skewers_to_typed(board, &raw.skew_ex_them)); v },
        discovered: { let mut v = discovered_to_typed(board, &raw.disc_ex_us); v.extend(discovered_to_typed(board, &raw.disc_ex_them)); v },
        hanging: graph.find_hanging(),
        outnumbered: graph.find_outnumbered(),
        mover_favored: graph.find_mover_favored(),
        overloaded,
        false_defense: { let mut v = graph.find_false_defense(us, &pins); v.extend(graph.find_false_defense(them, &pins)); v },
        false_safety,
        pins,
    };

    let positional = PositionalReport {
        outposts: {
            let (_, out_ex_us) = detect_outposts(board, &graph, us);
            let (_, out_ex_them) = detect_outposts(board, &graph, them);
            let mut v = outposts_to_typed(board, &out_ex_us);
            v.extend(outposts_to_typed(board, &out_ex_them));
            v
        },
        open_files: extract_open_files(board),
        passed_pawns: extract_passed_pawns(board),
        doubled_pawns: extract_doubled_pawns(board),
        isolated_pawns: extract_isolated_pawns(board),
        pawn_islands: extract_pawn_islands(board),
        pawn_breaks: extract_pawn_breaks(groups, us, them),
        minority_attack: extract_minority_attack(groups, us),
        pawn_majority: extract_pawn_majority(groups, us, them),
        rook_on_seventh: extract_rook_on_seventh(groups, us, them),
        center_control: extract_center_control(board),
        king_exposure: {
            let exposures = extract_king_exposure(board);
            if exposures.len() >= 2 {
                exposures.into_iter().max_by_key(|k| k.attacker_count)
            } else {
                exposures.into_iter().next()
            }
        },
        development: {
            let dev_infos = extract_development_info(board, &graph);
            let target_color = Side::from(us);
            let dev_first = dev_infos.iter().find(|d| d.color == target_color);
            dev_first.cloned().or_else(|| dev_infos.first().cloned())
        },
    };

    let material = {
        let balance = build_material_balance(groups);
        MaterialConceptReport { balance }
    };

    let in_check = graph.is_in_check(us);

    // Four whole-position scalars with no per-concept typed home elsewhere —
    // read once, here, at position.rs's own conversion boundary, so no
    // downstream consumer (render_explanations included) needs to reach back
    // into groups.*.terms itself.
    let king_tropism_us = groups.king_safety.terms.get("tropism_us").and_then(|v| v.as_i64()).unwrap_or(0);
    let doubled_rooks_us = groups.piece_activity.terms.get("doubled_rooks").and_then(|v| v.as_i64()).unwrap_or(0);
    let development_score_diff = groups.development.terms.get("development_diff").and_then(|v| v.as_i64()).unwrap_or(0);
    let initiative_us = groups.strategic.terms.get("initiative").and_then(|v| v.as_i64()).unwrap_or(0);

    // Partial SensorReport — everything typed downstream consumers (encode_state,
    // extract_concepts) need is already built above. Reused for both rather than
    // re-deriving each from EvalGroups.terms independently.
    let partial = SensorReport {
        fen: fen.to_string(), state_id: 0,
        material: material.clone(), tactical: tactical.clone(), positional: positional.clone(),
        aggregated: AggregatedScores::default(),
        evaluated_forks: evaluated_forks.clone(),
        in_check,
        mate_in_1_exists,
        ..Default::default()
    };
    let state_id = encode_state(&partial, groups, phase).state_id;

    let gated_issues = if let Some(elo) = player_elo {
        let side = Side::from(us);
        let concepts = extract_concepts(&partial, groups, side);
        rank_issues_for_position(&concepts, elo)
    } else { Vec::new() };

    SensorReport {
        fen: fen.to_string(),
        state_id,
        material,
        tactical,
        positional,
        aggregated: AggregatedScores {
            material_cp: groups.material_total.value,
            positional_cp: groups.positional_total.value,
            tactical_cp: groups.tactical_total.value,
            total_cp: groups.material_total.value + groups.positional_total.value + groups.tactical_total.value,
            chaos: chaos_coefficient(groups),
        },
        in_check,
        evaluated_forks,
        gated_issues,
        mate_in_1_exists,
        king_tropism_us,
        doubled_rooks_us,
        development_score_diff,
        initiative_us,
    }
}

/// Normalize a position so White is always the side to move — see
/// `normalize_to_white_to_move`'s doc comment for why.
/// Every scoring function in this file already takes a `Color` and computes
/// `us − them` where `us = chess.turn()` — feed them a position where White
/// always *is* `chess.turn()`, and they need no changes at all; the ~25
/// scattered `if color.is_white() {..} else {..}` branches throughout this
/// file all correctly collapse to their White arm.
///
/// Returns `(normalized, was_flipped)`. When `was_flipped`, board-coordinate
/// output (squares/colors in `SensorReport`) must be un-flipped back before
/// it reaches a human — see `unflip_sensor_report` below (which uses
/// `Side::other()` for colors and `crate::canonical::unflip_square_str` for
/// squares). Scores need no such correction: after normalization they're
/// already relative to whoever is really to move, which is what every
/// consumer wants.
fn normalize_for_eval(chess: &Chess) -> Result<(Chess, bool)> {
    normalize_to_white_to_move(chess)
}

/// Un-flip every square/color in a `SensorReport` back to real board terms,
/// in place. Only called when `normalize_for_eval` actually flipped the
/// position. Covers every field that carries a color or square — including
/// ones with a color but no square at all (`OpenFile`, `KingExposure`,
/// `MinorityAttack`, etc. — vertical-only flipping never touches files, so
/// those don't need a square correction, but they're still labeled
/// White/Black and do need the color swap) — verified against every
/// struct in concept_types.rs/sensor.rs, not just the ones with PieceRefs.
/// `unflip_square_str` lives in `crate::canonical` (generic, no eval-specific
/// types); the color swap itself is just `Side::other()` now — no separate
/// `unflip_color` helper needed.
fn unflip_piece_ref(pr: &mut PieceRef) {
    pr.square = unflip_square_str(&pr.square);
    pr.color = pr.color.other();
}

fn unflip_sensor_report(sensor: &mut SensorReport) {
    // material: a real color/color split (white/black piece counts and
    // bishop-pair flags), not a score — swap both halves.
    if let Some(bal) = &mut sensor.material.balance {
        std::mem::swap(&mut bal.white, &mut bal.black);
        std::mem::swap(&mut bal.bishop_pair_white, &mut bal.bishop_pair_black);
        // bal.centipawns is a score (us-relative), not a board fact — leave it.
    }

    for f in &mut sensor.tactical.forks {
        unflip_piece_ref(&mut f.attacker);
        for t in &mut f.targets { unflip_piece_ref(t); }
    }
    for p in &mut sensor.tactical.pins {
        unflip_piece_ref(&mut p.attacker);
        unflip_piece_ref(&mut p.pinned);
        unflip_piece_ref(&mut p.shielded);
    }
    for s in &mut sensor.tactical.skewers {
        unflip_piece_ref(&mut s.attacker);
        unflip_piece_ref(&mut s.front);
        unflip_piece_ref(&mut s.behind);
    }
    for d in &mut sensor.tactical.discovered {
        unflip_piece_ref(&mut d.mover);
        unflip_piece_ref(&mut d.attacker);
        unflip_piece_ref(&mut d.target);
    }
    for h in &mut sensor.tactical.hanging {
        unflip_piece_ref(&mut h.piece);
    }
    for on in &mut sensor.tactical.outnumbered {
        unflip_piece_ref(&mut on.piece);
    }
    for be in &mut sensor.tactical.mover_favored {
        unflip_piece_ref(&mut be.piece);
    }
    for o in &mut sensor.tactical.overloaded {
        unflip_piece_ref(&mut o.piece);
        for t in &mut o.critical_for { unflip_piece_ref(t); }
    }
    for fd in &mut sensor.tactical.false_defense {
        unflip_piece_ref(&mut fd.piece);
        for d in &mut fd.pinned_defenders { unflip_piece_ref(d); }
    }
    for fs in &mut sensor.tactical.false_safety {
        unflip_piece_ref(&mut fs.piece);
        for d in &mut fs.compromised_defenders { unflip_piece_ref(d); }
    }
    for ef in &mut sensor.evaluated_forks {
        unflip_piece_ref(&mut ef.attacker);
        for t in &mut ef.targets { unflip_piece_ref(t); }
        if let Some(h) = &mut ef.hangs { unflip_piece_ref(h); }
    }

    for o in &mut sensor.positional.outposts {
        unflip_piece_ref(&mut o.piece);
        unflip_piece_ref(&mut o.supported_by);
    }
    for f in &mut sensor.positional.open_files {
        f.color = f.color.other(); // file letter unaffected by a vertical-only flip
    }
    for pp in &mut sensor.positional.passed_pawns {
        pp.square = unflip_square_str(&pp.square);
        pp.color = pp.color.other();
        // pp.rank is an already-orientation-invariant "distance to promotion"
        // (computed via extract_passed_pawns's own is_white() branch), not a
        // raw board rank — needs no correction.
    }
    for dp in &mut sensor.positional.doubled_pawns {
        dp.color = dp.color.other();
    }
    for ip in &mut sensor.positional.isolated_pawns {
        ip.square = unflip_square_str(&ip.square);
        ip.color = ip.color.other();
    }
    for pi in &mut sensor.positional.pawn_islands {
        pi.color = pi.color.other();
    }
    for pb in &mut sensor.positional.pawn_breaks {
        pb.square = unflip_square_str(&pb.square);
        pb.color = pb.color.other();
    }
    if let Some(ma) = &mut sensor.positional.minority_attack {
        ma.color = ma.color.other();
    }
    for pm in &mut sensor.positional.pawn_majority {
        pm.color = pm.color.other();
    }
    for r7 in &mut sensor.positional.rook_on_seventh {
        r7.color = r7.color.other();
    }
    if let Some(cc) = &mut sensor.positional.center_control {
        cc.color = cc.color.other();
    }
    if let Some(ke) = &mut sensor.positional.king_exposure {
        ke.color = ke.color.other();
    }
    if let Some(dev) = &mut sensor.positional.development {
        dev.color = dev.color.other();
        for p in &mut dev.undeveloped_pieces { unflip_piece_ref(p); }
    }

    // gated_issues carries no color at all (Concept/GatedIssue.mover is
    // `Mover::Us`/`Mover::Them` — see concept_types.rs's doc comment) and its
    // `.phrase` text is built from `Mover`'s Display ("the mover"/"the
    // opponent"), never a literal "White"/"Black" word — so unlike every
    // other field in this function, nothing here needs correcting for the
    // flip at all. This used to require a structural `.side.other()` swap
    // plus a blanket find/replace of "White"/"Black" inside already-rendered
    // phrase text (`unflip_phrase`, now deleted) — the single most fragile
    // part of this whole function, since any future phrase that didn't
    // route color through `us_color`/`them_color`, or that legitimately
    // needed the word "white"/"black" for something unrelated, would have
    // silently corrupted text headed straight into the `chess-coach` LLM
    // prompt. See `Mover`'s doc comment and FINDINGS.md's 2026-09-01 entry.
}

pub fn analyze_fen(fen: &str) -> Result<PositionRecord> {
    analyze_fen_with_engine_score(fen, None, None)
}

pub fn analyze_fen_with_engine_score(
    fen: &str,
    engine_score: Option<i64>,
    player_elo: Option<i32>,
) -> Result<PositionRecord> {
    let parsed = Fen::from_ascii(fen.as_bytes()).context("invalid FEN")?;
    let chess: Chess = parsed
        .into_position(shakmaty::CastlingMode::Standard)
        .context("could not convert FEN to chess position")?;

    let normalized_fen =
        Fen::from_position(&chess, shakmaty::EnPassantMode::Legal).to_string();
    let legal_move_count = chess.legal_moves().len();

    // Scoring/SensorReport work entirely in a normalized frame where White
    // is always the side to move — see normalize_for_eval's doc comment.
    // Scores that come out are already relative to whoever is really to
    // move, which is what every consumer wants; square/color-bearing output
    // gets un-flipped back below before it leaves this function.
    let (eval_chess, was_flipped) = normalize_for_eval(&chess)?;
    let phase = compute_phase(eval_chess.board());
    let groups = compute_groups(&eval_chess, phase, legal_move_count);
    // mate_in_1_exists is computed inside build_sensor_report itself now
    // (from whichever frame it's given — the boolean is color-symmetric, so
    // it doesn't matter which), not patched on here afterward.
    let mut sensor_report = build_sensor_report(eval_chess.board(), fen, &groups, &eval_chess, phase, player_elo);
    if was_flipped {
        unflip_sensor_report(&mut sensor_report);
    }
    let final_score = sum_groups(&groups);
    let delta = engine_score.map(|score| final_score - score);
    let sum_groups_match = delta.map(|d| d == 0).unwrap_or(true);

    Ok(PositionRecord {
        fen: fen.to_string(),
        normalized_fen,
        side_to_move: Side::from(chess.turn()),
        phase,
        final_score,
        engine_score,
        legal: LegalInfo {
            is_legal: true,
            is_check: chess.is_check(),
            is_checkmate: chess.is_checkmate(),
            is_stalemate: chess.is_stalemate(),
            is_insufficient_material: chess.is_insufficient_material(),
            legal_move_count,
        },
        groups,
        checks: Checks {
            sum_groups: final_score,
            matches_final: sum_groups_match,
            delta,
        },
        sensor_report,
    })
}

pub fn render_structured_explanations(record: &PositionRecord) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    let us_color = record.side_to_move;
    let side_cap = if us_color == Side::White { "White" } else { "Black" };
    let sensor = &record.sensor_report;

    let make_obj = |kind: &str, side: Side, severity: i64, phrase: String, details: serde_json::Map<String, serde_json::Value>| -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("kind".into(), serde_json::Value::from(kind));
        obj.insert("side".into(), serde_json::Value::from(side.to_string()));
        obj.insert("severity".into(), serde_json::Value::from(severity));
        obj.insert("phrase".into(), serde_json::Value::from(phrase));
        obj.insert("details".into(), serde_json::Value::Object(details));
        serde_json::Value::Object(obj)
    };

    // Checked first — see render_explanations's mate_in_1 comment (same
    // gap, same fix, both renderers).
    if sensor.mate_in_1_exists {
        out.push(make_obj("mate_in_1", us_color, 1000,
            format!("{side_cap} has a mate in 1! Check for a forced checkmate before anything else."),
            serde_json::Map::new()));
    }

    // Forks
    let forks_us: Vec<_> = sensor.tactical.forks.iter().filter(|f| f.attacker.color == us_color).collect();
    if !forks_us.is_empty() {
        let examples: Vec<serde_json::Value> = forks_us.iter().map(|f| {
            let mut m = serde_json::Map::new();
            m.insert("attacker".into(), serde_json::Value::from(f.attacker.notation()));
            m.insert("targets".into(), serde_json::Value::from(f.targets.iter().map(|t| t.notation()).collect::<Vec<_>>()));
            serde_json::Value::Object(m)
        }).collect();
        let phrase = format!("{} has {} fork(s) detected (e.g. {} -> {}).", side_cap, forks_us.len(),
            forks_us[0].attacker.notation(), forks_us[0].targets.iter().map(|t| t.notation()).collect::<Vec<_>>().join(", "));
        let mut details = serde_json::Map::new();
        details.insert("examples".into(), serde_json::Value::Array(examples));
        out.push(make_obj("fork", us_color, forks_us.len() as i64, phrase, details));
    }

    // Skewers
    let skewers_us: Vec<_> = sensor.tactical.skewers.iter().filter(|s| s.attacker.color == us_color).collect();
    if !skewers_us.is_empty() {
        let examples: Vec<serde_json::Value> = skewers_us.iter().map(|s| {
            let mut m = serde_json::Map::new();
            m.insert("attacker".into(), serde_json::Value::from(s.attacker.notation()));
            m.insert("front".into(), serde_json::Value::from(s.front.notation()));
            m.insert("back".into(), serde_json::Value::from(s.behind.notation()));
            serde_json::Value::Object(m)
        }).collect();
        let phrase = format!("{} has {} skewer(s) detected (e.g. {}: {} -> {}).", side_cap, skewers_us.len(),
            skewers_us[0].attacker.notation(), skewers_us[0].front.notation(), skewers_us[0].behind.notation());
        let mut details = serde_json::Map::new();
        details.insert("examples".into(), serde_json::Value::Array(examples));
        out.push(make_obj("skewer", us_color, skewers_us.len() as i64, phrase, details));
    }

    // Pins
    let pins_us: Vec<_> = sensor.tactical.pins.iter().filter(|p| p.attacker.color == us_color).collect();
    if !pins_us.is_empty() {
        let examples: Vec<serde_json::Value> = pins_us.iter().map(|p| {
            let mut m = serde_json::Map::new();
            m.insert("attacker".into(), serde_json::Value::from(p.attacker.notation()));
            m.insert("pinned".into(), serde_json::Value::from(p.pinned.notation()));
            m.insert("shielded".into(), serde_json::Value::from(p.shielded.notation()));
            serde_json::Value::Object(m)
        }).collect();
        let phrase = format!("{} has {} pin(s) (e.g. {} pins {} to {}).", side_cap, pins_us.len(),
            pins_us[0].attacker.notation(), pins_us[0].pinned.notation(), pins_us[0].shielded.notation());
        let mut details = serde_json::Map::new();
        details.insert("examples".into(), serde_json::Value::Array(examples));
        out.push(make_obj("pin", us_color, pins_us.len() as i64, phrase, details));
    }

    // Discovered attacks
    let disc_us: Vec<_> = sensor.tactical.discovered.iter().filter(|d| d.attacker.color == us_color).collect();
    if !disc_us.is_empty() {
        let examples: Vec<serde_json::Value> = disc_us.iter().map(|d| {
            let mut m = serde_json::Map::new();
            m.insert("mover".into(), serde_json::Value::from(d.mover.notation()));
            m.insert("attacker".into(), serde_json::Value::from(d.attacker.notation()));
            m.insert("target".into(), serde_json::Value::from(d.target.notation()));
            serde_json::Value::Object(m)
        }).collect();
        let phrase = format!("{} has {} discovered-attack opportunity(ies) (e.g. {} moves, {} attacks {}).", side_cap, disc_us.len(),
            disc_us[0].mover.notation(), disc_us[0].attacker.notation(), disc_us[0].target.notation());
        let mut details = serde_json::Map::new();
        details.insert("examples".into(), serde_json::Value::Array(examples));
        out.push(make_obj("discovered", us_color, disc_us.len() as i64, phrase, details));
    }

    // Outposts
    let outposts_us: Vec<_> = sensor.positional.outposts.iter().filter(|o| o.piece.color == us_color).collect();
    if !outposts_us.is_empty() {
        let examples: Vec<serde_json::Value> = outposts_us.iter().map(|o| {
            let mut m = serde_json::Map::new();
            m.insert("square".into(), serde_json::Value::from(o.piece.notation()));
            m.insert("support".into(), serde_json::Value::from(o.supported_by.notation()));
            serde_json::Value::Object(m)
        }).collect();
        let phrase = format!("{} has {} outpost(s) (e.g. {} supported by {}).", side_cap, outposts_us.len(),
            outposts_us[0].piece.notation(), outposts_us[0].supported_by.notation());
        let mut details = serde_json::Map::new();
        details.insert("examples".into(), serde_json::Value::Array(examples));
        out.push(make_obj("outpost", us_color, outposts_us.len() as i64, phrase, details));
    }

    // Rook activity
    let open_files_us: Vec<_> = sensor.positional.open_files.iter().filter(|f| f.color == us_color && f.rook_count > 0).collect();
    if !open_files_us.is_empty() {
        let n = open_files_us.len() as i64;
        let mut details = serde_json::Map::new();
        details.insert("count".into(), serde_json::Value::from(n));
        let phrase = format!("{} controls {} open file(s) with rooks.", side_cap, n);
        out.push(make_obj("rook_open_files", us_color, n, phrase, details));
    }

    if out.is_empty() {
        let mut details = serde_json::Map::new();
        details.insert("msg".into(), serde_json::Value::from("none"));
        out.push(make_obj("none", us_color, 0, "No immediate human-readable issues detected by static HUGM heuristics.".to_string(), details));
    }

    out
}

/// Plain-language verdict for a `Fork`/`Outnumbered`/`MoverFavored` SEE
/// result. `see_cp`/`consequence` are always mover-relative (see their own
/// doc comments), and this phrase deliberately names the mover explicitly
/// rather than leaving it implicit — "wins material" with no named subject
/// reads as attaching to whichever side the sentence is *about* ("White's
/// rook is outnumbered — wins material" easily misreads as White winning),
/// which is backwards whenever the mover isn't the sentence's grammatical
/// subject.
fn consequence_phrase(consequence: &Consequence, see_cp: i64, mover: &str) -> String {
    match consequence {
        Consequence::Winning => format!("a material win for {mover} (~{see_cp}cp)"),
        Consequence::Minor => format!("a small edge for {mover} (~{see_cp}cp)"),
        Consequence::Losing => format!("not currently favorable for {mover} (~{see_cp}cp)"),
        Consequence::Even => format!("roughly balanced for {mover} if captured"),
    }
}

/// Reads only `record.sensor_report` — no `groups.*.terms` access here.
pub fn render_explanations(record: &PositionRecord) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let us_color = record.side_to_move;
    let side = if us_color == Side::White { "White" } else { "Black" };
    let opp = if side == "White" { "Black" } else { "White" };
    let opp_color = us_color.other();
    let sensor = &record.sensor_report;

    // Checked first, ahead of every tactical/positional concept below: the
    // single most decisive fact a position can have (mate_in_1 outranks
    // everything in extract_concepts's own severity ordering too, 1000 vs a
    // max-realistic material_imbalance around 900). Was computed on
    // SensorReport all along (build_sensor_report) but never surfaced here —
    // a real gap: `.explanations` is what `--verbose true` callers actually
    // read (this session's own check_move.nu scratch tool included), and
    // `mate_in_1_exists` only ever reached the ELO-gated `gated_issues` path
    // via extract_concepts, which nothing in this session's live-play
    // checking called (`--player-elo` was never passed). Caught the hard way
    // (FINDINGS.md, 2026-09-01): played straight into `17...Qxh2#` with
    // `sensor_report.mate_in_1_exists` sitting `true` on the position one
    // move earlier, completely invisible to the tool actually being read.
    if sensor.mate_in_1_exists {
        out.push(format!("{side} has a mate in 1! Check for a forced checkmate before anything else."));
    }

    // Tactical explanations with examples when available
    let forks_us: Vec<_> = sensor.tactical.forks.iter().filter(|f| f.attacker.color == us_color).collect();
    if !forks_us.is_empty() {
        let f = forks_us[0];
        let t_str = f.targets.iter().map(|t| t.notation()).collect::<Vec<_>>().join(", ");
        out.push(format!("{} has {} fork(s) detected (e.g. {} -> {}) — {} — check for immediate tactical threats or trade opportunities.", side, forks_us.len(), f.attacker.notation(), t_str, consequence_phrase(&f.consequence, f.see_cp, side)));
    }
    let skewers_us: Vec<_> = sensor.tactical.skewers.iter().filter(|s| s.attacker.color == us_color).collect();
    if !skewers_us.is_empty() {
        let s = skewers_us[0];
        out.push(format!("{} has {} skewer(s) detected (e.g. {}: {} -> {}) — high-value piece may be attacked in-line.", side, skewers_us.len(), s.attacker.notation(), s.front.notation(), s.behind.notation()));
    }
    let pins_us: Vec<_> = sensor.tactical.pins.iter().filter(|p| p.attacker.color == us_color).collect();
    if !pins_us.is_empty() {
        let p = pins_us[0];
        out.push(format!("{} has {} pin(s) (e.g. {} pins {} to {}) — consider relieving pressure or trading pinned pieces.", side, pins_us.len(), p.attacker.notation(), p.pinned.notation(), p.shielded.notation()));
    }
    let disc_us: Vec<_> = sensor.tactical.discovered.iter().filter(|d| d.attacker.color == us_color).collect();
    if !disc_us.is_empty() {
        let d = disc_us[0];
        out.push(format!("{} has {} discovered-attack opportunity(ies) (e.g. {} moves, {} attacks {}) — watch for moves that uncover attacks.", side, disc_us.len(), d.mover.notation(), d.attacker.notation(), d.target.notation()));
    }

    // Opponent tactical warnings — `side` (the mover, who must actually
    // defend) is the sentence subject; `opp` names the threat's source. This
    // used to put `opp` in the subject position ("White has forks (by
    // opponent)"), which reads as if the opponent were the one forked —
    // backwards from what the sentence needs to communicate.
    let forks_them: Vec<_> = sensor.tactical.forks.iter().filter(|f| f.attacker.color == opp_color).collect();
    if !forks_them.is_empty() {
        let f = forks_them[0];
        let t_str = f.targets.iter().map(|t| t.notation()).collect::<Vec<_>>().join(", ");
        out.push(format!("{} faces {} fork(s) from {} (e.g. {} -> {}) — {} — consider defensive resources.", side, forks_them.len(), opp, f.attacker.notation(), t_str, consequence_phrase(&f.consequence, f.see_cp, opp)));
    }

    // Outnumbered pieces — real defenders exist (unlike hanging), but the
    // attacking side has more attackers than the defending side has
    // defenders. This block didn't exist before even though the structured
    // data (`sensor.tactical.outnumbered`) was already populated.
    let outnumbered_us: Vec<_> = sensor.tactical.outnumbered.iter().filter(|o| o.piece.color == us_color).collect();
    if !outnumbered_us.is_empty() {
        let o = outnumbered_us[0];
        // The outnumbered piece belongs to `us` (side); the side that would
        // actually do the capturing — and whose perspective `consequence`/
        // `see_cp` are computed from — is the opponent, not `side` itself.
        out.push(format!("{}'s {} on {} is outnumbered ({} attackers vs {} defenders) — {}.", side, o.piece.role, o.piece.square, o.attacker_count, o.defender_count, consequence_phrase(&o.consequence, o.see_cp, opp)));
    }

    // Pieces that look adequately defended by raw count (defenders >=
    // attackers — outside outnumbered's territory) but where the real
    // exchange still favors whoever would initiate it. This is the gap a
    // count comparison alone can never see — see `MoverFavored`'s doc
    // comment for the real game this was missed in.
    let mover_favored_us: Vec<_> = sensor.tactical.mover_favored.iter().filter(|m| m.piece.color == us_color).collect();
    if !mover_favored_us.is_empty() {
        let m = mover_favored_us[0];
        out.push(format!("{}'s {} on {} looks defended by count ({} attackers vs {} defenders) but {} — worth a second look before relying on it.", side, m.piece.role, m.piece.square, m.attacker_count, m.defender_count, consequence_phrase(&m.consequence, m.see_cp, opp)));
    }

    // King safety / tropism
    if sensor.king_tropism_us > 0 {
        out.push(format!("{} pieces show tropism toward the opponent king (score = {}) — attacking chances exist.", side, sensor.king_tropism_us));
    }

    // Rook activity
    let open_files_us = sensor.positional.open_files.iter().filter(|f| f.color == us_color && f.rook_count > 0).count();
    if open_files_us > 0 {
        out.push(format!("{} controls {} open file(s) with rooks — good rook activity.", side, open_files_us));
    }
    for r in sensor.positional.rook_on_seventh.iter().filter(|r| r.color == us_color) {
        out.push(format!("{} has {} rook(s) on the 7th rank — strong pressure on enemy pawns and king.", side, r.count));
    }
    // Doubled rooks
    if sensor.doubled_rooks_us > 0 {
        out.push(format!("{} has {} doubled-rook file(s) — potential for heavy-file pressure.", side, sensor.doubled_rooks_us));
    }

    // Pawn structure notes
    let isolated_us = sensor.positional.isolated_pawns.iter().filter(|p| p.color == us_color).count();
    if isolated_us > 0 {
        out.push(format!("{} has {} isolated pawn(s) — structural weakness to address.", side, isolated_us));
    }
    let passed_us = sensor.positional.passed_pawns.iter().filter(|p| p.color == us_color).count();
    if passed_us > 0 {
        out.push(format!("{} has {} passed pawn(s) — potential long-term advantage.", side, passed_us));
    }

    // Outpost explanation
    let outposts_us: Vec<_> = sensor.positional.outposts.iter().filter(|o| o.piece.color == us_color).collect();
    if !outposts_us.is_empty() {
        let o = outposts_us[0];
        out.push(format!("{} has {} outpost(s) (e.g. {} supported by {}) — strong squares often requiring specific plans to challenge.", side, outposts_us.len(), o.piece.notation(), o.supported_by.notation()));
    }

    // Development/space/initiative — whole-position scores
    if sensor.development_score_diff > 0 {
        out.push(format!("{} is ahead in development/space (diff = {}).", side, sensor.development_score_diff));
    }
    if sensor.initiative_us > 0 {
        out.push(format!("{} appears to have initiative ({}).", side, sensor.initiative_us));
    }

    if out.is_empty() {
        out.push("No immediate human-readable issues detected by static HUGM heuristics.".to_string());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::analyze_fen;
    use super::analyze_fen_with_engine_score;
    use super::Side;

    #[test]
    fn parses_starting_position() {
        let record = analyze_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("FEN should parse");
        assert_eq!(record.side_to_move, Side::White);
        assert!(record.legal.is_legal);
        assert!(!record.normalized_fen.is_empty());
    }

    #[test]
    fn handles_drawish_king_endgame() {
        let record = analyze_fen("8/8/8/8/8/7k/8/6K1 w - - 0 1").expect("FEN should parse");
        assert!(record.legal.is_insufficient_material || record.legal.is_stalemate);
        assert!(record.groups.drawishness.value <= 0);
    }

    #[test]
    fn handles_tactical_position() {
        let record =
            analyze_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/2N5/PPPP1PPP/R1BQKBNR w KQkq - 2 3")
                .expect("FEN should parse");
        assert!(record.phase > 0);
        assert!(record.legal.legal_move_count > 0);
    }

    #[test]
    fn compares_engine_score_when_provided() {
        let base = analyze_fen("8/8/8/8/8/7k/8/6K1 w - - 0 1").expect("FEN should parse");
        let record =
            analyze_fen_with_engine_score("8/8/8/8/8/7k/8/6K1 w - - 0 1", Some(base.final_score), None)
                .expect("FEN should parse");
        assert_eq!(record.engine_score, Some(base.final_score));
        assert!(record.checks.matches_final);
        assert_eq!(record.checks.delta, Some(0));
    }

    #[test]
    fn vector_features_present() {
        let record = analyze_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("FEN should parse");
        assert!(record
            .groups
            .vector_features
            .terms
            .contains_key("center_control_us"));
        assert!(record
            .groups
            .vector_features
            .terms
            .contains_key("tactical_pressure_us"));
    }

    #[test]
    fn tactical_pins_detected() {
        // Black bishop on b4 pins white piece on d2 to king on e1
        let fen = "7k/8/8/8/1b6/8/3N4/4K3 w - - 0 1";
        let record = analyze_fen(fen).expect("FEN should parse");
        assert!(record.groups.tactical.terms.contains_key("pins_us"));
        let pins_us = record
            .groups
            .tactical
            .terms
            .get("pins_us")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(pins_us >= 1);
        // sensor.tactical.pins must also be populated (not left as Vec::new())
        assert!(!record.sensor_report.tactical.pins.is_empty(), "sensor.tactical.pins was empty — bridge from detect_pins is broken");
    }

    #[test]
    fn rook_open_file_terms() {
        // White rook on a1 with no pawns on file -> open file controlled
        let fen = "8/8/8/8/8/8/8/R3K2k w - - 0 1";
        let record = analyze_fen(fen).expect("FEN should parse");
        assert!(record
            .groups
            .piece_activity
            .terms
            .contains_key("open_files_controlled"));
        let open_files = record
            .groups
            .piece_activity
            .terms
            .get("open_files_controlled")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(open_files >= 1);
    }

    #[test]
    fn king_tropism_present() {
        // Ensure king_tropism term present in a normal position (non-zero phase)
        let record = analyze_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").expect("FEN should parse");
        assert!(record.groups.king_safety.terms.contains_key("tropism_us"));
    }

    #[test]
    fn detects_skewer() {
        // White rook on a1 attacking black queen on a2 with black rook on a3 behind -> skewer
        let fen = "7k/8/8/8/8/r7/q7/R3K3 w - - 0 1";
        let record = analyze_fen(fen).expect("FEN should parse");
        let skewers_us = record
            .groups
            .tactical
            .terms
            .get("skewers_us")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(skewers_us >= 1);
    }

    #[test]
    fn detects_fork() {
        // White knight on d5 attacking black rook on b6 and black queen on f6
        let fen = "7k/8/1r3q2/3N4/8/8/8/4K3 w - - 0 1";
        let record = analyze_fen(fen).expect("FEN should parse");
        let forks_us = record
            .groups
            .tactical
            .terms
            .get("forks_us")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(forks_us >= 1);
    }

    #[test]
    fn detects_outpost() {
        // White knight on d5 supported by pawn on c4; no black pawn attacks d5
        let fen = "k7/8/8/3N4/2P5/8/8/4K3 w - - 0 1";
        let record = analyze_fen(fen).expect("FEN should parse");
        let outposts = record
            .groups
            .piece_activity
            .terms
            .get("outposts_us")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert!(outposts >= 1);
        // example string present
        let ex = record
            .groups
            .piece_activity
            .terms
            .get("outpost_example_us")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(!ex.is_empty());
    }
}


