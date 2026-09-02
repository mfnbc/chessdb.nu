use std::collections::BTreeMap;
use std::ops::ControlFlow;

use nu_protocol::{LabeledError, Span};
use pgn_reader::{RawTag, Reader, SanPlus, Skip, Visitor};
use shakmaty::{
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
pub struct SquareControl {
    pub square: String,
    /// None if the square is empty — there's nothing to compute control
    /// from, geometric attack generation always starts from an occupied
    /// square.
    pub piece: Option<PieceOnSquare>,
    /// Every square this one piece geometrically controls — occupied-aware
    /// for sliding pieces (a piece behind a blocker doesn't count), but
    /// deliberately independent of whose turn it is, check, and pins. This
    /// is raw board control ("what does this piece see"), not legal
    /// mobility ("what can this piece actually play right now") — the two
    /// differ whenever the piece is pinned or it isn't its side's move.
    /// Includes squares held by the piece's own side (what it defends) and
    /// by the opponent (what it attacks) alike — the caller distinguishes
    /// those by cross-referencing the board itself.
    pub controls: Vec<String>,
    /// True for a light square (a1, h8, ...), false for dark — a genuine
    /// geometric fact (bishop color-complex reasoning: a light-squared
    /// bishop can never contest a dark square) present regardless of
    /// whether the square is occupied.
    pub is_light: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SquareAttackers {
    pub square: String,
    /// Every white/black piece (of either side) that attacks this square —
    /// the reverse question from `SquareControl::controls` ("what attacks
    /// this square" vs. "what does the piece on this square see"), and the
    /// more directly useful one for "is it safe to move a piece here":
    /// occupancy-aware, turn-independent, works on an empty square just as
    /// well as an occupied one (`Board::attacks_to` takes the target square
    /// and an explicit attacking color, not a piece that has to be there).
    pub attacked_by_white: Vec<String>,
    pub attacked_by_black: Vec<String>,
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
    let mobility_san = legal_moves
        .iter()
        .map(|mv| San::from_move(&pos, *mv).to_string())
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

/// Every square one specific piece geometrically controls, board-occupancy
/// aware — the primitive behind a "what does this piece actually see"
/// spatial view, so that question is answered by the same tested move-
/// generation the rest of this crate relies on rather than by re-deriving
/// file/rank/diagonal offsets by hand at the call site (the exact class of
/// arithmetic slip that hung a bishop in live play, FINDINGS.md,
/// 2026-09-02 — "reasoning based on visibility" instead of mental
/// geometry, per the user's own framing of that request).
pub fn square_control(fen_str: &str, square_str: &str, span: Span) -> Result<SquareControl, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let sq: Square = square_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid square '{square_str}': {e}"))
            .with_label("expected algebraic notation, e.g. 'e4'", span)
    })?;

    let board = pos.board();
    let piece = board.piece_at(sq).map(|p| PieceOnSquare {
        role: role_name(p.role).to_string(),
        color: match p.color {
            Color::White => "white",
            Color::Black => "black",
        }
        .to_string(),
    });
    let controls = board
        .attacks_from(sq)
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Ok(SquareControl { square: square_str.to_string(), piece, controls, is_light: sq.is_light() })
}

/// Every piece (either side) that attacks a given square — the reverse of
/// `square_control`'s "what does this piece see." Answers "is it safe to
/// move a piece here" directly, without needing a piece to already be on
/// the target square: `Board::attacks_to` takes the target and an explicit
/// attacking color, occupancy-aware, turn-independent.
pub fn square_attackers(fen_str: &str, square_str: &str, span: Span) -> Result<SquareAttackers, LabeledError> {
    let pos = fen_to_chess(fen_str, span)?;
    let sq: Square = square_str.parse().map_err(|e| {
        LabeledError::new(format!("Invalid square '{square_str}': {e}"))
            .with_label("expected algebraic notation, e.g. 'e4'", span)
    })?;

    let board = pos.board();
    let occupied = board.occupied();
    let attacked_by_white = board
        .attacks_to(sq, Color::White, occupied)
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let attacked_by_black = board
        .attacks_to(sq, Color::Black, occupied)
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Ok(SquareAttackers { square: square_str.to_string(), attacked_by_white, attacked_by_black })
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

#[cfg(test)]
mod square_control_tests {
    use super::*;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn empty_square_has_no_piece_and_no_control() {
        let result = square_control(STARTPOS, "e4", Span::test_data()).expect("valid square");
        assert!(result.piece.is_none());
        assert!(result.controls.is_empty());
    }

    #[test]
    fn knight_in_the_corner_controls_exactly_its_three_reachable_squares() {
        // Nb1 in the start position: only 3 of a knight's usual 8
        // destinations fit on the board from here, and all 3 are its own
        // side's pawns/empty squares — control includes defended own
        // pieces, not just enemy-facing attacks (see the field doc comment
        // on SquareControl::controls).
        let result = square_control(STARTPOS, "b1", Span::test_data()).expect("valid square");
        let piece = result.piece.expect("b1 is occupied in the start position");
        assert_eq!(piece.role, "Knight");
        assert_eq!(piece.color, "white");
        let mut controls = result.controls.clone();
        controls.sort();
        assert_eq!(controls, vec!["a3".to_string(), "c3".to_string(), "d2".to_string()]);
    }

    #[test]
    fn sliding_piece_control_stops_at_the_first_blocker() {
        // Bc1 in the start position is boxed in by its own pawns on b2 and
        // d2 — it controls exactly those two squares (where it stops) and
        // nothing past them, the blocker-awareness a hand-rolled diagonal
        // check would need to get right on its own.
        let result = square_control(STARTPOS, "c1", Span::test_data()).expect("valid square");
        let piece = result.piece.expect("c1 is occupied in the start position");
        assert_eq!(piece.role, "Bishop");
        let mut controls = result.controls.clone();
        controls.sort();
        assert_eq!(controls, vec!["b2".to_string(), "d2".to_string()]);
    }

    #[test]
    fn invalid_square_is_a_labeled_error_not_a_panic() {
        let result = square_control(STARTPOS, "z9", Span::test_data());
        assert!(result.is_err());
    }

    #[test]
    fn control_reports_square_color() {
        // b1 is a light square, c1 is dark (shakmaty's own convention,
        // doc-verified against Square::D1.is_light()/E1.is_dark()) — spot
        // check both so the field isn't accidentally inverted.
        assert!(square_control(STARTPOS, "b1", Span::test_data()).expect("valid square").is_light);
        assert!(!square_control(STARTPOS, "c1", Span::test_data()).expect("valid square").is_light);
    }
}

#[cfg(test)]
mod square_attackers_tests {
    use super::*;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn empty_square_attacked_by_neither_side() {
        let result = square_attackers(STARTPOS, "e4", Span::test_data()).expect("valid square");
        assert!(result.attacked_by_white.is_empty());
        assert!(result.attacked_by_black.is_empty());
    }

    #[test]
    fn square_attacked_by_exactly_one_side() {
        // c3 is reachable by White's Nb1 (b1->c3) and the b2/d2 pawns
        // (diagonal capture squares) but nothing Black owns reaches it yet
        // in the start position.
        let result = square_attackers(STARTPOS, "c3", Span::test_data()).expect("valid square");
        let mut white = result.attacked_by_white.clone();
        white.sort();
        assert_eq!(white, vec!["b1".to_string(), "b2".to_string(), "d2".to_string()]);
        assert!(result.attacked_by_black.is_empty());
    }

    #[test]
    fn square_attacked_by_both_sides() {
        // White knight a1 and Black knight a3 both reach c2 — the exact
        // question `square_control` alone can't answer (it only reports
        // what a piece already sitting on the target square would see).
        let fen = "4k3/8/8/8/8/n7/8/N3K3 w - - 0 1";
        let result = square_attackers(fen, "c2", Span::test_data()).expect("valid square");
        assert_eq!(result.attacked_by_white, vec!["a1".to_string()]);
        assert_eq!(result.attacked_by_black, vec!["a3".to_string()]);
    }

    #[test]
    fn invalid_square_is_a_labeled_error_not_a_panic() {
        let result = square_attackers(STARTPOS, "z9", Span::test_data());
        assert!(result.is_err());
    }
}
