use std::collections::BTreeMap;
use std::ops::ControlFlow;

use nu_protocol::{LabeledError, Span};
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{
    attacks,
    fen::Fen,
    san::San,
    uci::UciMove,
    zobrist::Zobrist64,
    Bitboard, Chess, Color, EnPassantMode, Piece, Position, Role, Square,
};

use crate::canonical::{flip_move, normalize_to_white_to_move};
use crate::chess::fen_to_chess;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FenInfoData {
    pub fen: String,
    pub turn: String,
    pub castling: String,
    pub ep_square: String,
    pub halfmoves: i64,
    pub fullmoves: i64,
    pub material_white: i64,
    pub material_black: i64,
    pub material_diff: i64,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub is_stalemate: bool,
    pub is_insufficient_material: bool,
    pub legal_move_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MoveRow {
    pub game_index: u32,
    pub ply: u32,
    pub move_number: u32,
    pub color: String,
    /// The move exactly as played, real perspective — what a human reviewing
    /// this specific game should see (see `chess-review` in `chessdb/sync.nu`).
    pub san: String,
    /// The same move translated into the canonical (White-to-move) frame —
    /// what cross-game aggregation by position identity should group on
    /// (see `chess-explore`), since mixing real-frame SAN from either side
    /// under one canonical position is meaningless.
    pub canonical_san: String,
    pub uci: String,
    pub fen: String,
    pub zobrist: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchGameRow {
    pub game_index: u32,
    pub source_game_id: String,
    pub headers: Vec<(String, String)>,
    pub result: String,
    pub moves: Vec<MoveRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniquePositionRow {
    pub zobrist: String,
    pub fen: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchSummary {
    pub source: String,
    pub games: Vec<BatchGameRow>,
    pub positions: Vec<MoveRow>,
    pub unique_positions: Vec<UniquePositionRow>,
}

pub fn encode_move(mv: &shakmaty::Move) -> u16 {
    let from = mv.from().map(|s| s as u16).unwrap_or(0);
    let to = mv.to() as u16;
    let promo = match mv.promotion() {
        Some(shakmaty::Role::Knight) => 1,
        Some(shakmaty::Role::Bishop) => 2,
        Some(shakmaty::Role::Rook) => 3,
        Some(shakmaty::Role::Queen) => 4,
        _ => 0,
    };
    (from & 0x3F) | ((to & 0x3F) << 6) | ((promo & 0x07) << 12)
}

fn get_canonical_hash(pos: &Chess) -> String {
    let hash: Zobrist64 = pos.zobrist_hash(EnPassantMode::Legal);
    format!("{:016x}", hash.0)
}

/// The standard starting position's FEN and its canonical zobrist hash — the
/// one place both are computed, so `pgn_to_batch_record` and
/// `process_corpus.rs`'s streaming ingest path (which needs "position zero"
/// before any moves are replayed) can't silently drift apart on the value.
pub fn initial_position() -> (String, String) {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string();
    let hash = get_canonical_hash(&Chess::default());
    (fen, hash)
}

struct GameVisitor {
    game_index: u32,
    headers: Vec<(String, String)>,
    pos: Chess,
    rows: Vec<MoveRow>,
    ply: u32,
    error: Option<String>,
}

impl GameVisitor {
    fn new(game_index: u32) -> Self {
        Self {
            game_index,
            headers: Vec::new(),
            pos: Chess::default(),
            rows: Vec::new(),
            ply: 0,
            error: None,
        }
    }
}

impl Visitor for GameVisitor {
    type Tags = ();
    type Movetext = ();
    type Output = Vec<MoveRow>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(())
    }

    fn tag(
        &mut self,
        _tags: &mut Self::Tags,
        key: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        if let (Ok(key), Ok(value)) = (
            std::str::from_utf8(key),
            std::str::from_utf8(value.as_bytes()),
        ) {
            self.headers
                .push((key.to_string(), value.trim_matches('"').to_string()));
        }
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, _tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        ControlFlow::Continue(())
    }

    fn san(&mut self, _movetext: &mut Self::Movetext, san_plus: SanPlus) -> ControlFlow<Self::Output> {
        if self.error.is_some() {
            return ControlFlow::Continue(());
        }

        let san_str = san_plus.to_string();
        let bare = san_str.trim_end_matches(['+', '#']);

        let san: San = match bare.parse() {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(format!("SAN parse error '{bare}': {e}"));
                return ControlFlow::Continue(());
            }
        };

        let mv = match san.to_move(&self.pos) {
            Ok(m) => m,
            Err(e) => {
                self.error = Some(format!("Illegal move '{bare}': {e}"));
                return ControlFlow::Continue(());
            }
        };

        let uci = UciMove::from_move(mv, shakmaty::CastlingMode::Standard).to_string();

        let new_pos = match self.pos.clone().play(mv) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("Play error: {e}"));
                return ControlFlow::Continue(());
            }
        };

        // Store this position's identity in canonical (White-to-move) frame
        // so real color-mirror positions from different games collapse onto
        // one row (see canonical.rs). `uci` is left in real terms since
        // nothing downstream consumes it.
        let (canonical_pos, _) = match normalize_to_white_to_move(&new_pos) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("Canonicalization error: {e}"));
                return ControlFlow::Continue(());
            }
        };
        let fen = Fen::from_position(&canonical_pos, EnPassantMode::Legal).to_string();
        let zobrist = get_canonical_hash(&canonical_pos);

        // `san` stays real (as actually played) — human-facing single-game
        // review (`chess-review`) must show what really happened, not a
        // color-mirrored move. `canonical_san` is the separate translated
        // value cross-game aggregation by canonical position should group
        // on instead (see `chess-explore`), since mixing real-frame SAN from
        // either side under one canonical position is meaningless there.
        let canonical_san = if self.pos.turn() == Color::Black {
            let (canonical_pre_pos, _) = match normalize_to_white_to_move(&self.pos) {
                Ok(p) => p,
                Err(e) => {
                    self.error = Some(format!("Canonicalization error: {e}"));
                    return ControlFlow::Continue(());
                }
            };
            let flipped_mv = flip_move(&mv);
            shakmaty::san::SanPlus::from_move(canonical_pre_pos, flipped_mv).to_string()
        } else {
            san_str.clone()
        };

        let move_number = (self.ply / 2) + 1;
        let color = if self.ply.is_multiple_of(2) { "white" } else { "black" };
        self.ply += 1;

        self.rows.push(MoveRow {
            game_index: self.game_index,
            ply: self.ply,
            move_number,
            color: color.to_string(),
            san: san_str,
            canonical_san,
            uci,
            fen,
            zobrist,
        });

        self.pos = new_pos;

        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _movetext: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }

    fn end_game(&mut self, _movetext: Self::Movetext) -> Self::Output {
        std::mem::take(&mut self.rows)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttackSummary {
    pub attacked_by_white: Vec<String>,
    pub attacked_by_black: Vec<String>,
    pub white_attack_count: i64,
    pub black_attack_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MobilitySummary {
    pub side_to_move: String,
    pub legal_move_count: i64,
    pub mobility_san: Vec<String>,
    /// Same legal moves, same order, in UCI form — needed by any caller that
    /// wants to actually apply one via `chessdb apply-uci` (which only
    /// accepts UCI), e.g. a breadth-first "what does the opponent have here"
    /// visualizer that enumerates real replies without searching/ranking
    /// them (FINDINGS.md, 2026-09-01).
    pub mobility_uci: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PieceOnSquare {
    pub role: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CheckerSummary {
    pub side_to_move: String,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub checker_squares: Vec<String>,
}

fn play_and_serialize(pos: Chess, mv: &shakmaty::Move) -> Result<String, LabeledError> {
    let new_pos = pos
        .play(*mv)
        .map_err(|e| LabeledError::new(format!("Cannot play move: {e}")))?;
    Ok(Fen::from_position(&new_pos, EnPassantMode::Legal).to_string())
}

fn side_to_move_string(pos: &Chess) -> String {
    match pos.turn() {
        Color::White => "white",
        Color::Black => "black",
    }
    .to_string()
}

fn castling_string(pos: &Chess) -> String {
    let cr = pos.castles();
    let mut s = String::new();
    if cr.has(Color::White, shakmaty::CastlingSide::KingSide) {
        s.push('K');
    }
    if cr.has(Color::White, shakmaty::CastlingSide::QueenSide) {
        s.push('Q');
    }
    if cr.has(Color::Black, shakmaty::CastlingSide::KingSide) {
        s.push('k');
    }
    if cr.has(Color::Black, shakmaty::CastlingSide::QueenSide) {
        s.push('q');
    }
    if s.is_empty() {
        s.push('-');
    }
    s
}

fn material_score(pos: &Chess, color: Color) -> i64 {
    let count = |role: Role| -> i64 { pos.board().by_piece(Piece { color, role }).count() as i64 };

    count(Role::Pawn)
        + count(Role::Knight) * 3
        + count(Role::Bishop) * 3
        + count(Role::Rook) * 5
        + count(Role::Queen) * 9
}

pub fn apply_san(fen_str: &str, san_str: &str, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let san: San = san_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid SAN: {e}")).with_label("failed to parse SAN move", span)
    })?;

    let mv = san.to_move(&pos).map_err(|e| {
        LabeledError::new(format!("Illegal move: {e}"))
            .with_label("move is not legal in this position", span)
    })?;

    play_and_serialize(pos, &mv)
}

pub fn normalize_fen(fen_str: &str, span: Span) -> Result<String, LabeledError> {
    let fen: Fen = fen_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid FEN: {e}")).with_label("failed to parse FEN", span)
    })?;

    let pos: Chess = fen
        .into_position(shakmaty::CastlingMode::Standard)
        .map_err(|e| {
            LabeledError::new(format!("Invalid position: {e}"))
                .with_label("position is illegal", span)
        })?;

    Ok(Fen::from_position(&pos, EnPassantMode::Legal).to_string())
}

pub fn apply_uci(fen_str: &str, uci_str: &str, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let uci: UciMove = uci_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid UCI move: {e}"))
            .with_label("failed to parse UCI move", span)
    })?;

    let mv = uci.to_move(&pos).map_err(|e| {
        LabeledError::new(format!("Illegal UCI move: {e}"))
            .with_label("move is not legal in this position", span)
    })?;

    play_and_serialize(pos, &mv)
}

pub fn uci_to_san(fen_str: &str, uci_str: &str, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let uci: UciMove = uci_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid UCI: {e}")).with_label("failed to parse UCI", span)
    })?;

    let mv = uci.to_move(&pos).map_err(|e| {
        LabeledError::new(format!("Illegal move: {e}"))
            .with_label("move is not legal in this position", span)
    })?;

    Ok(San::from_move(&pos, mv).to_string())
}

pub fn san_to_uci(fen_str: &str, san_str: &str, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let san: San = san_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid SAN: {e}")).with_label("failed to parse SAN", span)
    })?;

    let mv = san.to_move(&pos).map_err(|e| {
        LabeledError::new(format!("Illegal move: {e}"))
            .with_label("move is not legal in this position", span)
    })?;

    Ok(UciMove::from_move(mv, shakmaty::CastlingMode::Standard).to_string())
}

pub fn is_legal(fen_str: &str, move_str: &str, span: Span) -> Result<bool, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;

    Ok(if let Ok(san) = move_str.parse::<San>() {
        san.to_move(&pos).is_ok()
    } else if let Ok(uci) = move_str.parse::<UciMove>() {
        uci.to_move(&pos).is_ok()
    } else {
        false
    })
}

pub fn fen_info(fen_str: &str, span: Span) -> Result<FenInfoData, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;

    let ep_square = pos
        .ep_square(shakmaty::EnPassantMode::Legal)
        .map(|sq| sq.to_string())
        .unwrap_or_else(|| "-".into());

    let halfmoves = pos.halfmoves() as i64;
    let fullmoves = pos.fullmoves().get() as i64;

    let material_white = material_score(&pos, Color::White);
    let material_black = material_score(&pos, Color::Black);

    Ok(FenInfoData {
        fen: fen_str.to_string(),
        turn: side_to_move_string(&pos),
        castling: castling_string(&pos),
        ep_square,
        halfmoves,
        fullmoves,
        material_white,
        material_black,
        material_diff: material_white - material_black,
        is_check: pos.is_check(),
        is_checkmate: pos.is_checkmate(),
        is_stalemate: pos.is_stalemate(),
        is_insufficient_material: pos.is_insufficient_material(),
        legal_move_count: pos.legal_moves().len() as i64,
    })
}

fn attacked_squares(pos: &Chess, attacker: Color) -> Vec<String> {
    let mut attacked = Bitboard::EMPTY;

    for sq in pos.board().by_color(attacker) {
        attacked |= pos.board().attacks_from(sq);
    }

    attacked.into_iter().map(|sq| sq.to_string()).collect()
}

pub fn attack_summary(fen_str: &str, span: Span) -> Result<AttackSummary, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;

    let white = attacked_squares(&pos, Color::White);
    let black = attacked_squares(&pos, Color::Black);

    let white_attack_count = white.len() as i64;
    let black_attack_count = black.len() as i64;

    Ok(AttackSummary {
        attacked_by_white: white,
        attacked_by_black: black,
        white_attack_count,
        black_attack_count,
    })
}

pub fn mobility_summary(fen_str: &str, span: Span) -> Result<MobilitySummary, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let side_to_move = side_to_move_string(&pos);

    let legal_moves = pos.legal_moves();
    // `San::from_move` deliberately omits the +/# suffix; `SanPlus::from_move`
    // plays the move on a clone to compute it. A bare `San` here silently
    // made every forcing_moves.nu "CHECKS"/"CHECKMATE AVAILABLE" query return
    // empty forever (2026-09-03, found while cross-verifying the nuon
    // migration against a known Fool's Mate position — see FINDINGS.md).
    let mobility_san = legal_moves
        .iter()
        .map(|mv| shakmaty::san::SanPlus::from_move(pos.clone(), *mv).to_string())
        .collect::<Vec<_>>();
    let mobility_uci = legal_moves
        .iter()
        .map(|mv| UciMove::from_move(*mv, shakmaty::CastlingMode::Standard).to_string())
        .collect::<Vec<_>>();

    Ok(MobilitySummary {
        side_to_move,
        legal_move_count: mobility_san.len() as i64,
        mobility_san,
        mobility_uci,
    })
}

#[cfg(test)]
mod mobility_summary_tests {
    use super::*;

    #[test]
    fn checking_and_mating_moves_carry_their_san_suffix() {
        // Fool's Mate: 1.f4 e5 2.g4 Qh4# -- a bare `San::from_move` (the
        // bug this test guards against) renders this "Qh4" with no "#",
        // which silently made forcing_moves.nu's whole CHECKS/CHECKMATE
        // detection return empty forever (found 2026-09-03 while
        // cross-verifying its nuon-migration output against this exact
        // known position).
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/5PP1/8/PPPPP2P/RNBQKBNR b KQkq - 0 2";
        let result = mobility_summary(fen, Span::test_data()).expect("valid fen");
        assert!(result.mobility_san.contains(&"Qh4#".to_string()), "{:?}", result.mobility_san);

        // A non-mating check also needs its suffix: 1.e4 d6 2.Bb5+ (the
        // d-pawn move vacates d7, opening the whole b5-e8 diagonal).
        let fen2 = "rnbqkbnr/ppp1pppp/3p4/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        let result2 = mobility_summary(fen2, Span::test_data()).expect("valid fen");
        assert!(result2.mobility_san.contains(&"Bb5+".to_string()), "{:?}", result2.mobility_san);
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Pawn => "Pawn",
        Role::Knight => "Knight",
        Role::Bishop => "Bishop",
        Role::Rook => "Rook",
        Role::Queen => "Queen",
        Role::King => "King",
    }
}

// ===========================================================================
// Leaf layer (2026-09-03): a close-to-1:1 translation of shakmaty's own
// geometry/board-state functions into nuon, replacing rust-side composition
// (square_control/square_attackers/square_swap_list/board_probe above) with
// nushell-side composition built from these leaves. Explicit user direction:
// "basically translating [shakmaty's] output to nuon, and accepting their
// output as nuon ... instead of a skill asking the plugin for the right
// questions, a tree of reports [is] compiled ... in nushell." See
// `chessdb_shakmaty_1to1` memory / FINDINGS.md for the full rationale.
//
// `occupied` is always an explicit input here, exactly matching shakmaty's
// own function signatures — never implicitly "the real board's current
// occupancy." That's what lets a caller do square_swap_list's recursive
// x-ray removal in pure nu: call geom-attacks/board-pieces with the full
// occupancy, subtract squares with an ordinary `where` filter, call again.
// ===========================================================================

fn parse_square(square_str: &str, span: Span) -> Result<Square, LabeledError> {
    square_str
        .parse()
        .map_err(|e| LabeledError::new(format!("Invalid square '{square_str}': {e}")).with_label("expected algebraic notation, e.g. 'e4'", span))
}

fn parse_color(color_str: &str, span: Span) -> Result<Color, LabeledError> {
    match color_str.to_ascii_lowercase().as_str() {
        "white" => Ok(Color::White),
        "black" => Ok(Color::Black),
        _ => Err(LabeledError::new(format!("Invalid color '{color_str}'")).with_label("expected 'white' or 'black'", span)),
    }
}

fn parse_role(role_str: &str, span: Span) -> Result<Role, LabeledError> {
    match role_str.to_ascii_lowercase().as_str() {
        "pawn" => Ok(Role::Pawn),
        "knight" => Ok(Role::Knight),
        "bishop" => Ok(Role::Bishop),
        "rook" => Ok(Role::Rook),
        "queen" => Ok(Role::Queen),
        "king" => Ok(Role::King),
        _ => Err(LabeledError::new(format!("Invalid role '{role_str}'")).with_label("expected pawn/knight/bishop/rook/queen/king", span)),
    }
}

fn parse_squares_to_bitboard(squares: &[String], span: Span) -> Result<Bitboard, LabeledError> {
    squares.iter().map(|s| parse_square(s, span)).collect::<Result<Bitboard, _>>()
}

fn color_name(color: Color) -> String {
    match color {
        Color::White => "white",
        Color::Black => "black",
    }
    .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeomAttacksResult {
    pub square: String,
    pub color: String,
    pub role: String,
    pub occupied: Vec<String>,
    pub squares: Vec<String>,
}

/// `attacks::attacks(square, piece, occupied)` — pure geometry, no board or
/// position involved at all. One dispatcher for every role (shakmaty's own
/// design, not a chessdb composition): pawn/knight/king ignore `occupied`
/// entirely, bishop/rook/queen use it for blocking. `occupied` is required
/// (pass `[]` for a piece with nothing blocking it, e.g. a lone bishop).
pub fn geom_attacks(square_str: &str, color_str: &str, role_str: &str, occupied_squares: &[String], span: Span) -> Result<GeomAttacksResult, LabeledError> {
    let sq = parse_square(square_str, span)?;
    let color = parse_color(color_str, span)?;
    let role = parse_role(role_str, span)?;
    let occupied = parse_squares_to_bitboard(occupied_squares, span)?;
    let result = attacks::attacks(sq, Piece { color, role }, occupied);
    Ok(GeomAttacksResult {
        square: square_str.to_string(),
        color: color_str.to_string(),
        role: role_str.to_string(),
        occupied: occupied_squares.to_vec(),
        squares: result.into_iter().map(|s| s.to_string()).collect(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeomRayResult {
    pub a: String,
    pub b: String,
    pub squares: Vec<String>,
}

/// `attacks::ray(a, b)` — every square on the rank/file/diagonal through
/// both `a` and `b` (the whole line, both directions, `a`/`b` included), or
/// empty if they don't share one. No board needed.
pub fn geom_ray(a_str: &str, b_str: &str, span: Span) -> Result<GeomRayResult, LabeledError> {
    let a = parse_square(a_str, span)?;
    let b = parse_square(b_str, span)?;
    let result = attacks::ray(a, b);
    Ok(GeomRayResult { a: a_str.to_string(), b: b_str.to_string(), squares: result.into_iter().map(|s| s.to_string()).collect() })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeomBetweenResult {
    pub a: String,
    pub b: String,
    pub squares: Vec<String>,
}

/// `attacks::between(a, b)` — squares strictly between `a` and `b` on a
/// shared rank/file/diagonal (`a`/`b` excluded), empty if not aligned or
/// adjacent. No board needed.
pub fn geom_between(a_str: &str, b_str: &str, span: Span) -> Result<GeomBetweenResult, LabeledError> {
    let a = parse_square(a_str, span)?;
    let b = parse_square(b_str, span)?;
    let result = attacks::between(a, b);
    Ok(GeomBetweenResult { a: a_str.to_string(), b: b_str.to_string(), squares: result.into_iter().map(|s| s.to_string()).collect() })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeomAlignedResult {
    pub a: String,
    pub b: String,
    pub c: String,
    pub aligned: bool,
}

/// `attacks::aligned(a, b, c)` — true if all three squares share a
/// rank/file/diagonal. No board needed.
pub fn geom_aligned(a_str: &str, b_str: &str, c_str: &str, span: Span) -> Result<GeomAlignedResult, LabeledError> {
    let a = parse_square(a_str, span)?;
    let b = parse_square(b_str, span)?;
    let c = parse_square(c_str, span)?;
    Ok(GeomAlignedResult { a: a_str.to_string(), b: b_str.to_string(), c: c_str.to_string(), aligned: attacks::aligned(a, b, c) })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BoardPiecesResult {
    pub color: Option<String>,
    pub role: Option<String>,
    pub squares: Vec<String>,
}

/// `Board::occupied`/`by_color`/`by_role`/`by_piece` — the board's own
/// piece-placement bitboards, filtered by whichever of `color`/`role` is
/// given (neither -> every occupied square, both -> exactly `by_piece`).
pub fn board_pieces(fen_str: &str, color_str: Option<&str>, role_str: Option<&str>, span: Span) -> Result<BoardPiecesResult, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let board = pos.board();
    let color = color_str.map(|c| parse_color(c, span)).transpose()?;
    let role = role_str.map(|r| parse_role(r, span)).transpose()?;

    let bb = match (color, role) {
        (Some(c), Some(r)) => board.by_piece(Piece { color: c, role: r }),
        (Some(c), None) => board.by_color(c),
        (None, Some(r)) => board.by_role(r),
        (None, None) => board.occupied(),
    };

    Ok(BoardPiecesResult {
        color: color_str.map(|s| s.to_string()),
        role: role_str.map(|s| s.to_string()),
        squares: bb.into_iter().map(|s| s.to_string()).collect(),
    })
}

/// `Board::piece_at` — the single piece on one square, or `None` if empty.
pub fn board_piece_at(fen_str: &str, square_str: &str, span: Span) -> Result<Option<PieceOnSquare>, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let sq = parse_square(square_str, span)?;
    Ok(pos.board().piece_at(sq).map(|p| PieceOnSquare { role: role_name(p.role).to_string(), color: color_name(p.color) }))
}

/// `Square::is_light` — a pure geometric fact about the square itself, no
/// board or position dependency at all (bishop color-complex reasoning:
/// a light-squared bishop can never contest a dark square).
pub fn square_is_light(square_str: &str, span: Span) -> Result<bool, LabeledError> {
    Ok(parse_square(square_str, span)?.is_light())
}

#[cfg(test)]
mod leaf_layer_tests {
    use super::*;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn geom_attacks_knight_matches_the_independently_established_square_control_fact() {
        // Cross-checked against square_control_tests::
        // knight_in_the_corner_controls_exactly_its_three_reachable_squares
        // (an older, pre-existing test) rather than trusting this new
        // function's own idea of the right answer. Occupied is irrelevant
        // to a knight, passed empty on purpose.
        let result = geom_attacks("b1", "white", "knight", &[], Span::test_data()).expect("valid input");
        let mut squares = result.squares.clone();
        squares.sort();
        assert_eq!(squares, vec!["a3".to_string(), "c3".to_string(), "d2".to_string()]);
    }

    #[test]
    fn geom_attacks_rook_on_an_open_board_covers_the_whole_rank_and_file() {
        // A rook with nothing on the board but itself: well-known chess
        // geometry, not something derived from this crate's own code — 7
        // squares along the file + 7 along the rank = 14.
        let result = geom_attacks("d4", "white", "rook", &[], Span::test_data()).expect("valid input");
        assert_eq!(result.squares.len(), 14);
        assert!(result.squares.contains(&"d8".to_string()));
        assert!(result.squares.contains(&"a4".to_string()));
        assert!(result.squares.contains(&"h4".to_string()));
        assert!(result.squares.contains(&"d1".to_string()));
    }

    #[test]
    fn geom_attacks_rook_stops_at_the_given_occupied_blocker() {
        let open = geom_attacks("d1", "white", "rook", &[], Span::test_data()).expect("valid input");
        assert!(open.squares.contains(&"d8".to_string()));
        let blocked = geom_attacks("d1", "white", "rook", &["d4".to_string()], Span::test_data()).expect("valid input");
        assert!(blocked.squares.contains(&"d4".to_string()), "the blocker square itself is still reachable/capturable");
        assert!(!blocked.squares.contains(&"d5".to_string()), "nothing past the blocker");
        assert!(!blocked.squares.contains(&"d8".to_string()));
    }

    #[test]
    fn geom_ray_and_between_match_shakmatys_own_doc_examples() {
        // Sourced directly from shakmaty::attacks::between's own doc
        // comment (Square::B1 to Square::B7 -> b2..b6), not invented here.
        let between = geom_between("b1", "b7", Span::test_data()).expect("valid input");
        let mut squares = between.squares.clone();
        squares.sort();
        assert_eq!(squares, vec!["b2", "b3", "b4", "b5", "b6"].into_iter().map(str::to_string).collect::<Vec<_>>());

        // A ray includes both endpoints and extends the full line both ways.
        let ray = geom_ray("b1", "b7", Span::test_data()).expect("valid input");
        assert!(ray.squares.contains(&"b1".to_string()));
        assert!(ray.squares.contains(&"b8".to_string()), "ray extends past b7 to the board edge");
    }

    #[test]
    fn geom_aligned_matches_shakmatys_own_doc_example() {
        // shakmaty::attacks::aligned's own doc comment: A1, B2, C3 are aligned.
        let result = geom_aligned("a1", "b2", "c3", Span::test_data()).expect("valid input");
        assert!(result.aligned);
        let not_aligned = geom_aligned("a1", "b2", "d3", Span::test_data()).expect("valid input");
        assert!(!not_aligned.aligned);
    }

    #[test]
    fn board_pieces_filters_match_known_start_position_placement() {
        let knights = board_pieces(STARTPOS, Some("white"), Some("knight"), Span::test_data()).expect("valid input");
        let mut squares = knights.squares.clone();
        squares.sort();
        assert_eq!(squares, vec!["b1".to_string(), "g1".to_string()]);

        let all_white = board_pieces(STARTPOS, Some("white"), None, Span::test_data()).expect("valid input");
        assert_eq!(all_white.squares.len(), 16);

        let all_occupied = board_pieces(STARTPOS, None, None, Span::test_data()).expect("valid input");
        assert_eq!(all_occupied.squares.len(), 32);
    }

    #[test]
    fn board_piece_at_matches_the_independently_established_square_control_fact() {
        // Cross-checked against square_control_tests::sliding_piece_control_stops_at_the_first_blocker.
        let result = board_piece_at(STARTPOS, "c1", Span::test_data()).expect("valid input");
        let piece = result.expect("c1 is occupied in the start position");
        assert_eq!(piece.role, "Bishop");
        assert_eq!(piece.color, "white");

        let empty = board_piece_at(STARTPOS, "e4", Span::test_data()).expect("valid input");
        assert!(empty.is_none());
    }

    #[test]
    fn square_is_light_matches_shakmatys_own_doc_verified_convention() {
        // Same fact square_control_tests::control_reports_square_color
        // already established: b1 light, c1 dark.
        assert!(square_is_light("b1", Span::test_data()).expect("valid square"));
        assert!(!square_is_light("c1", Span::test_data()).expect("valid square"));
    }
}

pub fn checker_summary(fen_str: &str, span: Span) -> Result<CheckerSummary, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let side_to_move = side_to_move_string(&pos);

    let checker_squares = pos
        .checkers()
        .into_iter()
        .map(|sq| sq.to_string())
        .collect::<Vec<_>>();

    Ok(CheckerSummary {
        side_to_move,
        is_check: pos.is_check(),
        is_checkmate: pos.is_checkmate(),
        checker_squares,
    })
}

pub fn pgn_to_fens(pgn_str: &str, span: Span) -> Result<Vec<MoveRow>, LabeledError> {
    let mut reader = Reader::new(pgn_str.as_bytes());
    let mut visitor = GameVisitor::new(0);

    let rows = reader
        .read_game(&mut visitor)
        .map_err(|e| {
            LabeledError::new(format!("PGN parse error: {e}"))
                .with_label("failed to parse PGN", span)
        })?
        .unwrap_or_default();

    if let Some(err) = visitor.error {
        return Err(LabeledError::new(err).with_label("error during move replay", span));
    }

    Ok(rows)
}

pub fn pgn_to_batch_record(pgn_str: &str, span: Span) -> Result<BatchSummary, LabeledError> {
    let (initial_fen, initial_hash) = initial_position();

    let mut reader = Reader::new(pgn_str.as_bytes());
    let mut games = Vec::new();
    let mut positions = Vec::new();
    let mut unique_map: BTreeMap<String, String> = BTreeMap::new();
    unique_map.insert(initial_hash, initial_fen);
    let mut game_index: u32 = 0;

    loop {
        let mut visitor = GameVisitor::new(game_index);
        let game_rows = match reader.read_game(&mut visitor) {
            Ok(Some(rows)) => rows,
            Ok(None) => break,
            Err(e) => {
                return Err(LabeledError::new(format!("PGN parse error: {e}"))
                    .with_label("failed to parse PGN", span))
            }
        };

        if let Some(err) = visitor.error {
            eprintln!("Skipping game {game_index}: {err}");
            game_index += 1;
            continue;
        }

        for row in &game_rows {
            unique_map
                .entry(row.zobrist.clone())
                .or_insert_with(|| row.fen.clone());
        }

        positions.extend(game_rows.clone());

        games.push(BatchGameRow {
            game_index,
            source_game_id: visitor
                .headers
                .iter()
                .find(|(k, _)| k == "Event")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| format!("game-{game_index}")),
            headers: visitor.headers.clone(),
            result: visitor
                .headers
                .iter()
                .find(|(k, _)| k == "Result")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "*".into()),
            moves: game_rows,
        });

        game_index += 1;
    }

    let unique_positions: Vec<UniquePositionRow> = unique_map
        .into_iter()
        .map(|(zobrist, fen)| UniquePositionRow { zobrist, fen })
        .collect();

    Ok(BatchSummary {
        source: "pgn".into(),
        games,
        positions,
        unique_positions,
    })
}

pub fn zobrist(fen_str: &str, as_int: bool, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let hash: Zobrist64 = pos.zobrist_hash(EnPassantMode::Legal);
    let hash_value: u64 = hash.0;

    Ok(if as_int {
        hash_value.to_string()
    } else {
        format!("{:016x}", hash_value)
    })
}

/// Normalize an arbitrary FEN to the same canonical (White-always-to-move)
/// frame `positions.zobrist`/`.fen` use. Needed by `chessdb/db.nu`'s
/// `fetch-and-seed-eco`: ECO opening data (JeffML/eco.json) is keyed by real
/// FENs at whatever ply/side they were recorded at, but `enrich-openings`
/// joins against `positions.fen`, which is canonical — without converting
/// the ECO side first, matching silently fails for every opening recorded
/// at a Black-to-move ply.
pub fn canonicalize_fen(fen_str: &str, span: Span) -> Result<String, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let (canonical_pos, _) = normalize_to_white_to_move(&pos)
        .map_err(|e| LabeledError::new(format!("Canonicalization error: {e}")))?;
    Ok(Fen::from_position(&canonical_pos, EnPassantMode::Legal).to_string())
}

