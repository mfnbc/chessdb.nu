// Verifies the tablebase-style canonical position identity stored by
// `core::pgn_to_fens` (used by both `process_corpus.rs`'s real sync pipeline
// and the `PgnToBatch` command via `pgn_to_batch_record`): every position's
// `fen`/`zobrist` is normalized so White is always to move. `san` stays real
// (as actually played, for human-facing single-game review) while
// `canonical_san` carries the same move translated into that canonical
// frame (for cross-game aggregation by position identity); `uci` is left in
// real terms. See PLAN.md's "Canonical position identity (tablebase-style
// dedup)" section.

use nu_plugin_chessdb::canonical::{flip_colors, normalize_to_white_to_move};
use nu_plugin_chessdb::chess::fen_to_chess;
use nu_plugin_chessdb::core::{canonicalize_fen, pgn_to_fens};
use nu_protocol::Span;
use shakmaty::zobrist::{Zobrist64, ZobristHash};
use shakmaty::{fen::Fen, EnPassantMode, Color, Position};

fn hash_of(fen: &str) -> String {
    let pos = fen_to_chess(fen, Span::test_data()).expect("FEN should parse");
    let hash: Zobrist64 = pos.zobrist_hash(EnPassantMode::Legal);
    format!("{:016x}", hash.0)
}

#[test]
fn white_move_row_stores_canonical_fen_zobrist_and_unflipped_san() {
    // After 1. Nf3, the real position has Black to move, so the stored row
    // must be the color-flipped/vertically-mirrored (White-to-move) frame.
    // Hand-derived independently of canonical.rs's own implementation:
    // White's back rank + pawns (unmoved) mirror onto the canonical "Black"
    // side with the knight landing on f6; Black's untouched army mirrors
    // onto the canonical "White" side.
    let rows = pgn_to_fens("1. Nf3", Span::test_data()).expect("PGN should parse");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];

    assert_eq!(row.color, "white");
    assert_eq!(
        row.fen,
        "rnbqkb1r/pppppppp/5n2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 1 1"
    );
    assert_eq!(row.zobrist, hash_of(&row.fen));
    // The pre-move position (start position) already has White to move, so
    // no SAN translation applies — real and canonical SAN coincide.
    assert_eq!(row.san, "Nf3");
    assert_eq!(row.canonical_san, "Nf3");
    // uci stays in real (un-flipped) terms: g1-f3, not the canonical g1-f3
    // mirror onto rank 6.
    assert_eq!(row.uci, "g1f3");
}

#[test]
fn black_move_row_keeps_real_san_and_translates_canonical_san() {
    // 1. Nf3 Nf6: Black's reply is played from a real Black-to-move
    // position. `san` must stay the real, as-played notation ("Nf6") — this
    // is what a human reviewing this specific game (`chess-review`) must
    // see. `canonical_san` is regenerated in the canonical frame instead:
    // Black's g8-f6 knight move mirrors onto canonical White's g1-f3 (the
    // same square pair as the first test, by construction of this
    // symmetric opening), so canonical_san is "Nf3", not "Nf6".
    let rows = pgn_to_fens("1. Nf3 Nf6", Span::test_data()).expect("PGN should parse");
    assert_eq!(rows.len(), 2);
    let row = &rows[1];

    assert_eq!(row.color, "black");
    assert_eq!(row.san, "Nf6", "san must stay the real, as-played move");
    assert_eq!(row.canonical_san, "Nf3", "canonical_san must be translated into canonical frame");
    // uci stays real: Black really played g8-f6.
    assert_eq!(row.uci, "g8f6");
    // Both knights developed & mirrored -> the real position already has
    // White to move, so canonical fen/zobrist equal the real ones untouched.
    assert_eq!(
        row.fen,
        "rnbqkb1r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 2 2"
    );
    assert_eq!(row.zobrist, hash_of(&row.fen));
}

#[test]
fn canonical_zobrist_matches_independently_constructed_mirror_position() {
    // The core dedup claim: a position reached mid-game with Black to move
    // (via real PGN replay) and an independently-specified position that IS
    // its exact color-mirror (however that position might be reached in a
    // different game) must hash identically once both are put in canonical
    // (White-to-move) form. This is what lets two real games that reach
    // color-mirror-image positions collapse onto one stored row.
    let rows = pgn_to_fens("1. Nf3", Span::test_data()).expect("PGN should parse");
    let row = &rows[0];

    // Constructed independently of core.rs/canonical.rs: parse the real
    // (Black-to-move) position directly, and normalize it ourselves via the
    // same public helper `process_corpus.rs`'s pipeline relies on.
    let real_pos = fen_to_chess(
        "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
        Span::test_data(),
    )
    .expect("FEN should parse");
    let (canonical_pos, was_flipped) =
        normalize_to_white_to_move(&real_pos).expect("normalization should succeed");
    assert!(was_flipped, "Black-to-move position must be flipped");

    let hash: Zobrist64 = canonical_pos.zobrist_hash(EnPassantMode::Legal);
    let zobrist = format!("{:016x}", hash.0);

    assert_eq!(
        row.zobrist, zobrist,
        "pgn_to_fens's stored identity must match independently-normalized identity for the same real position"
    );
}

#[test]
fn canonicalize_fen_matches_pgn_to_fens_for_the_same_position() {
    // `canonicalize_fen` backs `fetch-and-seed-eco`'s fix for ECO opening
    // matching: ECO data is keyed by real FENs, but `enrich-openings` joins
    // against canonical `positions.fen` — ECO's real FENs must be converted
    // through this exact same transform at seed time, or matching silently
    // fails for every opening recorded at a Black-to-move ply. Verify it
    // agrees with what pgn_to_fens actually stores for an equivalent
    // Black-to-move real position (1. Nf3, from the tests above).
    let canonical = canonicalize_fen(
        "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
        Span::test_data(),
    )
    .expect("canonicalization should succeed");

    let rows = pgn_to_fens("1. Nf3", Span::test_data()).expect("PGN should parse");
    assert_eq!(canonical, rows[0].fen);

    // A position already White-to-move is a no-op (matches
    // normalize_to_white_to_move's fast path).
    let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    assert_eq!(canonicalize_fen(start, Span::test_data()).unwrap(), start);
}

#[test]
fn flip_colors_is_an_involution_and_de_canonicalizes() {
    // flip_colors is the unconditional primitive extracted so it can serve
    // both directions: normalize_to_white_to_move's forward flip, and
    // de-canonicalizing a known-canonical position (e.g. a positions.fen
    // row known — from moves.color — to have been stored via a flip) back
    // to real terms, which normalize_to_white_to_move itself can't do
    // (a canonical position always reads "White to move", so it would see
    // nothing to flip).
    let real_black_to_move = fen_to_chess(
        "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
        Span::test_data(),
    )
    .unwrap();
    assert_eq!(real_black_to_move.turn(), Color::Black);

    let canonical = flip_colors(&real_black_to_move).expect("flip should succeed");
    assert_eq!(canonical.turn(), Color::White);
    // Matches what normalize_to_white_to_move produces for the same input.
    let (via_normalize, was_flipped) = normalize_to_white_to_move(&real_black_to_move).unwrap();
    assert!(was_flipped);
    assert_eq!(
        Fen::from_position(canonical.clone(), EnPassantMode::Legal).to_string(),
        Fen::from_position(via_normalize, EnPassantMode::Legal).to_string()
    );

    // De-canonicalize: flipping the canonical result again must recover the
    // original real (Black-to-move) position exactly — the actual use case
    // this primitive was extracted for.
    let de_canonicalized = flip_colors(&canonical).expect("reverse flip should succeed");
    assert_eq!(de_canonicalized.turn(), Color::Black);
    assert_eq!(
        Fen::from_position(de_canonicalized, EnPassantMode::Legal).to_string(),
        Fen::from_position(real_black_to_move, EnPassantMode::Legal).to_string()
    );
}
