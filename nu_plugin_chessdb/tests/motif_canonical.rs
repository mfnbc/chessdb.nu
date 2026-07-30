// Canonical motif examples for HUGM validation
// Sources:
// - chessprogramming.org (tactical/positional motif pages)
// - Wikipedia: Positional play / pawn structures (examples adapted)
// - Lichess public puzzles / study examples (representative)

use nu_plugin_chessdb::eval::{analyze_fen, analyze_fen_with_engine_score, extract_concepts};

#[test]
fn wikipedia_pawn_break_detected() {
    // Source: Wikipedia / positional play (example adapted)
    // White pawn on c4 with no opposing pawns ahead should show a pawn-break opportunity c4->c5
    let fen = "4k3/8/8/8/2P5/8/8/4K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let pb = rec
        .groups
        .pawn_structure
        .terms
        .get("pawn_breaks")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(pb >= 1, "expected at least one pawn break opportunity");
    // structured example present when verbose; we still expect the plural array in terms when generated
}

#[test]
fn chessprogramming_minority_example() {
    // Source: chessprogramming.org / pawn majority discussion (adapted)
    // White has queenside majority (a2,b2,c2) vs Black (a7,b7)
    let fen = "4k3/pp6/8/8/8/8/PPP5/4K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let minority = rec
        .groups
        .pawn_structure
        .terms
        .get("minority_attack")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let strength = rec
        .groups
        .pawn_structure
        .terms
        .get("minority_attack_strength")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(minority == 1 || strength > 0, "expected minority attack signal");
}

#[test]
fn lichess_outpost_example() {
    // Source: Lichess / typical outpost positions (adapted)
    // White knight on d5 supported by pawn on c4; no black pawn attacks d5
    let fen = "k7/8/8/3N4/2P5/8/8/4K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let outposts = rec
        .groups
        .piece_activity
        .terms
        .get("outposts_us")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(outposts >= 1, "expected at least one outpost detected");
}

// Negative / near-miss tests to guard against hallucinations

#[test]
fn fork_negative_no_second_target() {
    // Similar to a fork position but only one attacked piece -> should NOT detect a fork
    let fen = "7k/8/8/3N2q1/8/8/8/4K3 w - - 0 1"; // knight on d5 attacks queen on f6 only
    let rec = analyze_fen(fen).expect("FEN should parse");
    let forks_us = rec
        .groups
        .tactical
        .terms
        .get("forks_us")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(forks_us, 0, "expected no fork detected");
}

#[test]
fn skewer_negative_no_back_piece() {
    // Rook aligned with queen but no piece behind -> not a skewer
    let fen = "7k/8/8/8/8/8/q7/R3K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let skewers_us = rec
        .groups
        .tactical
        .terms
        .get("skewers_us")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(skewers_us, 0, "expected no skewer detected");
}

#[test]
fn discovered_negative_no_target() {
    // Sliding piece is blocked but there is no enemy target behind -> not a discovered attack
    let fen = "7k/8/8/8/8/8/P7/R3K3 w - - 0 1"; // rook a1, pawn a2, no enemy on a3
    let rec = analyze_fen(fen).expect("FEN should parse");
    let disc_us = rec
        .groups
        .tactical
        .terms
        .get("discovered_us")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(disc_us, 0, "expected no discovered attack detected");
}

#[test]
fn discovered_negative_starting_position() {
    // Regression: detect_discovered previously flagged 3 "discovered attacks" per side on the
    // plain starting position (e.g. Ra1 "attacking" a7 if the a2 pawn moves). That's just
    // ordinary blocked-slider geometry mirrored on both sides, not a tactical discovered
    // attack — the revealed target (a7/d7/h7 pawn) is adequately defended and worth no more
    // than the attacking rook/queen. Found via the terms-bag -> typed SensorReport migration
    // (see PLAN.md), which was the first time this concept actually reached gated_issues output.
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let disc_us = rec.groups.tactical.terms.get("discovered_us").and_then(|v| v.as_i64()).unwrap_or(0);
    let disc_them = rec.groups.tactical.terms.get("discovered_them").and_then(|v| v.as_i64()).unwrap_or(0);
    assert_eq!(disc_us, 0, "expected no discovered attack detected for white in the starting position");
    assert_eq!(disc_them, 0, "expected no discovered attack detected for black in the starting position");
    assert!(rec.sensor_report.tactical.discovered.is_empty(), "typed SensorReport should agree with groups.terms");
}

#[test]
fn outpost_negative_attacked_by_pawn() {
    // Knight on d5 but attacked by an enemy pawn on c4 -> not an outpost
    let fen = "k7/8/8/3N4/2p5/8/8/4K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    let outposts = rec
        .groups
        .piece_activity
        .terms
        .get("outposts_us")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(outposts, 0, "expected no outpost detected when attacked by pawn");
}

#[test]
fn pawn_break_color_is_invariant_to_side_to_move() {
    // Regression: extract_pawn_breaks used to hardcode "white" for the
    // us-side break and "black" for the them-side break, regardless of
    // which color was actually to move. The same physical White pawn break
    // candidate must be reported as color="white" whether White or Black
    // is to move in this exact position.
    let board = "4k3/pp6/8/8/8/8/PPP5/4K3";
    for stm in ["w", "b"] {
        let fen = format!("{board} {stm} - - 0 1");
        let rec = analyze_fen(&fen).expect("FEN should parse");
        let breaks = &rec.sensor_report.positional.pawn_breaks;
        assert!(!breaks.is_empty(), "expected a pawn break candidate (stm={stm})");
        assert!(breaks.iter().all(|b| b.color == "white"), "pawn break color must stay \"white\" regardless of side to move (stm={stm}), got {breaks:?}");
    }
}

#[test]
fn king_exposed_concept_is_invariant_to_side_to_move() {
    // Regression: the king_exposed concept used to hardcode "white"/"black"
    // based on the sign of king_safety.blended, rather than mapping through
    // us_color/them_color the way every sibling concept (e.g. development)
    // already did. Whichever king is actually less safe on this board must
    // be reported the same way regardless of whose turn it is.
    let board = "5rk1/5ppp/8/8/8/8/8/4K3";
    let mut sides = Vec::new();
    for stm in ["w", "b"] {
        let fen = format!("{board} {stm} - - 0 1");
        let rec = analyze_fen(&fen).expect("FEN should parse");
        let concepts = extract_concepts(&rec.sensor_report, &rec.groups, &rec.side_to_move);
        let king_exposed = concepts.iter().find(|c| c.name == "king_exposed");
        if let Some(c) = king_exposed {
            sides.push(c.side.clone());
        }
    }
    assert_eq!(sides.len(), 2, "expected king_exposed to fire both times, got {sides:?}");
    assert_eq!(sides[0], sides[1], "king_exposed side must be invariant to side to move, got {sides:?}");
}

#[test]
fn board_normalization_reports_real_squares_and_colors_for_black_to_move() {
    // Regression for the board-normalization change: evaluation internally
    // flips any Black-to-move position so White is always to move (see
    // normalize_for_eval's doc comment in position.rs), then un-flips
    // square/color output before it reaches SensorReport. This is the one
    // new failure mode that change introduces, so it needs its own explicit
    // correctness test, not just an invariance check.
    //
    // These two FENs are hand-verified mirrors of each other (rank-flipped,
    // case-swapped, side-to-move flipped), computed independently of
    // normalize_for_eval's own implementation: Black's queen on f2 forking
    // White's Ke1/Bf1/d2/g2/Nf3 (White to move) becomes White's queen on f7
    // forking Black's Ke8/Bf8/d7/g7/Nf6 (Black to move).
    let original = "rnb1kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1qPP/RNBQKB1R w KQkq - 0 1";
    let mirrored = "rnbqkb1r/pppp1Qpp/5n2/4p3/4P3/8/PPPP1PPP/RNB1KBNR b KQkq - 0 1";

    let rec1 = analyze_fen_with_engine_score(original, None, Some(1900)).expect("original FEN should parse");
    let rec2 = analyze_fen_with_engine_score(mirrored, None, Some(1900)).expect("mirrored FEN should parse");

    assert_eq!(rec1.sensor_report.tactical.forks.len(), 1);
    let f1 = &rec1.sensor_report.tactical.forks[0];
    assert_eq!(f1.attacker.notation(), "Qf2");
    assert_eq!(f1.attacker.color, "black");

    assert_eq!(rec2.sensor_report.tactical.forks.len(), 1);
    let f2 = &rec2.sensor_report.tactical.forks[0];
    assert_eq!(f2.attacker.notation(), "Qf7", "attacker square must be the real (un-flipped) board square");
    assert_eq!(f2.attacker.color, "white", "attacker color must be the real (un-flipped) color");
    let target_squares: Vec<String> = f2.targets.iter().map(|t| t.notation()).collect();
    for expected in ["Ke8", "Bf8", "d7", "g7", "Nf6"] {
        assert!(target_squares.contains(&expected.to_string()), "expected target {expected} in {target_squares:?}");
    }

    // Same physical fact mirrored — the mover-relative score must match.
    assert_eq!(rec1.final_score, rec2.final_score, "final_score should be identical for mirrored positions (mover-relative, not White-relative)");

    // GatedIssue.phrase embeds color words as free text, built before the
    // un-flip pass runs — this must be corrected too, not just .side.
    let mat2 = rec2.sensor_report.gated_issues.iter().find(|g| g.name == "material_imbalance").expect("material_imbalance issue");
    assert_eq!(mat2.side, "white");
    assert!(mat2.phrase.starts_with("White"), "phrase should say White, got: {}", mat2.phrase);
}
