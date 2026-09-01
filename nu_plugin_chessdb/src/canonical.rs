//! Canonical (White-always-to-move) position identity.
//!
//! Shared by `core.rs` (position/zobrist/FEN identity stored in the
//! `positions` table, and move-notation translation during PGN replay) and
//! `eval::position` (internal evaluation normalization). This is the one
//! place the color-flip transform is implemented — extracted out of
//! `eval::position::normalize_for_eval` when `core.rs` also needed it, so
//! the two don't duplicate the same bitboard/`Setup` manipulation.
//!
//! Why this exists: every scoring function in `eval::position` already
//! computes `us − them` where `us = chess.turn()`, and `positions.zobrist`/
//! `.fen` as a database identity benefits from the same trick chess endgame
//! tablebases use — collapsing a position and its exact color-mirror (from
//! a different real game) onto one canonical entry, evaluated and stored
//! once instead of twice.

use anyhow::{Context, Result};
use shakmaty::{Board, Chess, Color, FromSetup, Move, Position, Setup, Square};

/// Unconditionally mirror a position: flip the board vertically, swap every
/// piece's color, swap castling rights, and mirror the en passant square.
/// This is the actual transform both directions of canonicalization need —
/// `normalize_to_white_to_move` calls it when a real position needs
/// flipping *to* canonical frame, and it's also the correct (and only)
/// way to de-canonicalize a position you already know is canonical (e.g.
/// a `positions.fen` row reached via a Black-color `moves` row) back to
/// real terms: you can't just call `normalize_to_white_to_move` again,
/// since a canonical position always reads "White to move" and that
/// function would see nothing to flip.
///
/// `shakmaty` has no single "swap a position's colors" call — `Board`'s
/// `flip_vertical` only repositions pieces, it doesn't recolor them — so
/// this rebuilds the board from its role/color bitboards, applying the
/// vertical flip to each one (position and color swap happen in the same
/// pass: `by_color.white` becomes what `by_color.black` was, vertically
/// mirrored, and vice versa). Applying it twice returns the original
/// position — it's its own inverse.
pub fn flip_colors(chess: &Chess) -> Result<Chess> {
    let setup = chess.clone().to_setup(shakmaty::EnPassantMode::Legal);

    let by_role = shakmaty::ByRole::new_with(|role| setup.board.by_role(role).flip_vertical());
    let by_color = shakmaty::ByColor {
        white: setup.board.by_color(Color::Black).flip_vertical(),
        black: setup.board.by_color(Color::White).flip_vertical(),
    };
    let normalized_board = Board::try_from_bitboards(by_role, by_color)
        .expect("role/color bitboards built from a flip of a valid board are always consistent");

    let normalized_setup = Setup {
        board: normalized_board,
        promoted: shakmaty::Bitboard::EMPTY,
        pockets: None,
        turn: chess.turn().other(),
        castling_rights: setup.castling_rights.flip_vertical(),
        ep_square: setup.ep_square.map(|sq| sq.flip_vertical()),
        remaining_checks: None,
        halfmoves: setup.halfmoves,
        fullmoves: setup.fullmoves,
    };

    Chess::from_setup(normalized_setup, shakmaty::CastlingMode::Standard)
        .context("could not build color-flipped position")
}

/// Normalize a position so White is always the side to move. Returns
/// `(normalized, was_flipped)`.
pub fn normalize_to_white_to_move(chess: &Chess) -> Result<(Chess, bool)> {
    if chess.turn() == Color::White {
        return Ok((chess.clone(), false));
    }
    Ok((flip_colors(chess)?, true))
}

/// Undo the vertical flip on a single square (its own inverse).
pub fn unflip_square(sq: Square) -> Square {
    sq.flip_vertical()
}

/// `PieceRef.square`/`PassedPawn.square`/etc. are already-formatted strings
/// (e.g. "d5"), not `Square` values, by the time they reach typed output
/// structs — parse, flip, reformat.
pub fn unflip_square_str(s: &str) -> String {
    match s.parse::<Square>() {
        Ok(sq) => unflip_square(sq).to_string(),
        Err(_) => s.to_string(), // not a plain square (e.g. a notation string) — leave as-is
    }
}

/// Swap "White"/"Black"/"white"/"black" words embedded in free text (e.g.
/// `GatedIssue.phrase`, built via `format!` before any un-flip pass runs, so
/// there's no structured field to derive a corrected phrase from). Both
/// capitalizations are swapped via a sentinel so a phrase containing both
/// words doesn't get double-flipped by a naive sequential replace.
pub fn unflip_phrase(phrase: &str) -> String {
    const SENTINEL: &str = "\u{0}";
    phrase
        .replace("White", SENTINEL)
        .replace("Black", "White")
        .replace(SENTINEL, "Black")
        .replace("white", SENTINEL)
        .replace("black", "white")
        .replace(SENTINEL, "black")
}

/// Translate a move into the canonical (White-to-move) frame. `Move`'s
/// variants reference only `Square`/`Role` fields — no color at all — so
/// this is purely a square flip; the role/capture/promotion fields carry no
/// orientation and pass through unchanged.
pub fn flip_move(mv: &Move) -> Move {
    match *mv {
        Move::Normal { role, from, capture, to, promotion } => Move::Normal {
            role, capture, promotion,
            from: from.flip_vertical(),
            to: to.flip_vertical(),
        },
        Move::EnPassant { from, to } => Move::EnPassant {
            from: from.flip_vertical(),
            to: to.flip_vertical(),
        },
        Move::Castle { king, rook } => Move::Castle {
            king: king.flip_vertical(),
            rook: rook.flip_vertical(),
        },
        Move::Put { role, to } => Move::Put { role, to: to.flip_vertical() },
    }
}
