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
use shakmaty::{Chess, Color, FromSetup, Move, Position, Square};

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
/// `Setup::mirror()` is shakmaty's own "swap a position's colors" call —
/// composed internally from exactly this project's own hand-rolled version
/// (`Board::mirror` for the board: vertical flip + `swap_colors`, plus the
/// same castling-rights/en-passant-square vertical flips and turn swap)
/// (2026-09-04, A/B-verified byte-identical across a real-position battery —
/// full/no/partial castling rights, an active en passant square, both
/// colors to move — before this hand-rolled version was replaced; see
/// `FINDINGS.md`). Applying it twice returns the original position — it's
/// its own inverse.
pub fn flip_colors(chess: &Chess) -> Result<Chess> {
    let setup = chess
        .clone()
        .to_setup(shakmaty::EnPassantMode::Legal)
        .into_mirrored();

    Chess::from_setup(setup, shakmaty::CastlingMode::Standard)
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

#[cfg(test)]
mod ab_diff {
    use super::flip_colors;
    use shakmaty::fen::Fen;
    use shakmaty::{EnPassantMode, Position};

    fn fen(s: &str) -> shakmaty::Chess {
        let setup = Fen::from_ascii(s.as_bytes()).expect("valid FEN");
        setup
            .into_position(shakmaty::CastlingMode::Standard)
            .expect("legal position")
    }

    #[test]
    fn flip_colors_is_an_involution_across_a_real_battery() {
        // Battery deliberately covers cases `tests/canonical_identity.rs`
        // doesn't: partial (one-side, one-flank) castling rights and an
        // active en passant square, alongside the full/no-rights and
        // both-colors-to-move cases already covered there. This battery was
        // originally built to A/B-diff `flip_colors`'s hand-rolled
        // bitboard-rebuild against `Setup::mirror()` before the swap to the
        // latter (2026-09-04, see `FINDINGS.md`); kept as a standing
        // involution regression now that only one implementation remains.
        let battery = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
            "r1bqr1k1/ppp2p1p/2n5/3p2p1/3Pn3/P3BN2/1PQ1PPPP/2KR1B1R b - - 3 12",
            "r2qkb1r/ppp1pppp/1n6/3PN3/2P3b1/2N5/PP3PPP/R1BQKB1R b KQkq - 0 8",
            "r3k2r/8/8/8/8/8/8/R3K2R w Kk - 0 1",
            "rnbqkbnr/1pp1pppp/p7/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        ];

        for fen_str in battery {
            let pos = fen(fen_str);
            let flipped = flip_colors(&pos).expect("flip should succeed");
            assert_ne!(pos.turn(), flipped.turn(), "turn must swap for {fen_str}");
            let twice = flip_colors(&flipped).expect("double-flip should succeed");
            assert_eq!(
                Fen::from_position(&pos, EnPassantMode::Legal).to_string(),
                Fen::from_position(&twice, EnPassantMode::Legal).to_string(),
                "flip_colors is not its own inverse for {fen_str}"
            );
        }
    }
}
