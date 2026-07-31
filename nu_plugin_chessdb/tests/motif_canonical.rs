// Canonical motif examples for HUGM validation
// Sources:
// - chessprogramming.org (tactical/positional motif pages)
// - Wikipedia: Positional play / pawn structures (examples adapted)
// - Lichess public puzzles / study examples (representative)

use nu_plugin_chessdb::eval::{analyze_fen, analyze_fen_with_engine_score, extract_concepts, rank_issues_for_position, Side};

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
        assert!(breaks.iter().all(|b| b.color == Side::White), "pawn break color must stay White regardless of side to move (stm={stm}), got {breaks:?}");
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
        let concepts = extract_concepts(&rec.sensor_report, &rec.groups, rec.side_to_move);
        let king_exposed = concepts.iter().find(|c| c.name == "king_exposed");
        if let Some(c) = king_exposed {
            sides.push(c.side);
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
    assert_eq!(f1.attacker.color, Side::Black);

    assert_eq!(rec2.sensor_report.tactical.forks.len(), 1);
    let f2 = &rec2.sensor_report.tactical.forks[0];
    assert_eq!(f2.attacker.notation(), "Qf7", "attacker square must be the real (un-flipped) board square");
    assert_eq!(f2.attacker.color, Side::White, "attacker color must be the real (un-flipped) color");
    let target_squares: Vec<String> = f2.targets.iter().map(|t| t.notation()).collect();
    for expected in ["Ke8", "Bf8", "d7", "g7", "Nf6"] {
        assert!(target_squares.contains(&expected.to_string()), "expected target {expected} in {target_squares:?}");
    }

    // Same physical fact mirrored — the mover-relative score must match.
    assert_eq!(rec1.final_score, rec2.final_score, "final_score should be identical for mirrored positions (mover-relative, not White-relative)");

    // GatedIssue.phrase embeds color words as free text, built before the
    // un-flip pass runs — this must be corrected too, not just .side.
    let mat2 = rec2.sensor_report.gated_issues.iter().find(|g| g.name == "material_imbalance").expect("material_imbalance issue");
    assert_eq!(mat2.side, Side::White);
    assert!(mat2.phrase.starts_with("White"), "phrase should say White, got: {}", mat2.phrase);
}

// Regression (found 2026-07-30 in a "what's detected but never surfaced"
// audit): sensor_report.mate_in_1_exists was a real, fully-computed legal-move
// scan that never became a Concept, so a live mate-in-1 opportunity produced
// no coaching signal at all — the same bug class as the hanging_piece gap
// fixed earlier this session. Also locks in that its severity (1000) is high
// enough to outrank material_imbalance in the ranked gated_issues output —
// finding a forced mate should never lose to a material-swing comment.
#[test]
fn mate_in_1_is_detected_and_ranks_above_material_imbalance() {
    // Back-rank mate: Ra1-a8# for White (Black king on g8 boxed in by its own
    // f7/g7/h7 pawns). Also a large material lead (R vs 3P), which is exactly
    // the scenario that would previously have outranked mate_in_1 entirely.
    let fen = "6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    assert!(rec.sensor_report.mate_in_1_exists, "sensor should detect the mate-in-1");

    let concepts = extract_concepts(&rec.sensor_report, &rec.groups, rec.side_to_move);
    let mate = concepts.iter().find(|c| c.name == "mate_in_1").expect("mate_in_1 concept should be present");
    assert_eq!(mate.side, Side::White);

    let issues = rank_issues_for_position(&concepts, 400);
    assert_eq!(issues.first().map(|i| i.name.as_str()), Some("mate_in_1"), "mate_in_1 should rank first, got {issues:?}");

    // The actual bug: `sensor_report.gated_issues` (the field a real
    // `chessdb hugm-eval --player-elo N` call returns) is computed *inside*
    // `build_sensor_report`, from a `partial` SensorReport built before this
    // function returns. The first version of this fix only wired mate_in_1
    // into `extract_concepts`, leaving `mate_in_1_exists` computed by the
    // *caller* (`analyze_fen_with_engine_score`) and patched onto the result
    // afterward — too late for build_sensor_report's own internal
    // extract_concepts call to see it. So `concepts`/`rank_issues_for_position`
    // above (called directly, bypassing that ordering bug) could pass while
    // the real API still silently produced no mate_in_1 issue at all. Now
    // fixed by computing mate_in_1_exists inside build_sensor_report itself.
    let rec_elo = analyze_fen_with_engine_score(fen, None, Some(400)).expect("FEN should parse");
    assert_eq!(
        rec_elo.sensor_report.gated_issues.first().map(|g| g.name.as_str()),
        Some("mate_in_1"),
        "sensor_report.gated_issues (the real API surface) should rank mate_in_1 first, got {:?}",
        rec_elo.sensor_report.gated_issues
    );

    // Flip-invariance: the same physical mate-in-1 fact, reached via a
    // Black-to-move mirror (rank-flipped, case-swapped, side-to-move
    // flipped) that goes through normalize_for_eval's flip path internally —
    // must produce the identical result, proving mate_in_1_exists doesn't
    // depend on which frame build_sensor_report happens to be given.
    let mirrored_fen = "r3k3/8/8/8/8/8/5PPP/6K1 b - - 0 1";
    let rec_mirrored = analyze_fen_with_engine_score(mirrored_fen, None, Some(400)).expect("FEN should parse");
    assert!(rec_mirrored.sensor_report.mate_in_1_exists, "mirrored position should also detect the mate-in-1");
    assert_eq!(
        rec_mirrored.sensor_report.gated_issues.first().map(|g| g.name.as_str()),
        Some("mate_in_1"),
        "mirrored position's gated_issues should also rank mate_in_1 first"
    );
}

// Regression (found in the same audit): sensor_report.positional.pawn_islands
// was computed by extract_pawn_islands (only populated once a side has 2+
// islands — fragmented pawn groups) but never turned into a Concept.
#[test]
fn pawn_islands_is_detected() {
    // White pawns on a2/h2 (2 islands, opposite edges of the board); Black
    // pawns on e7/f7 (adjacent files, 1 island — correctly not recorded).
    let fen = "4k3/4pp2/8/8/8/8/P6P/4K3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");
    assert_eq!(rec.sensor_report.positional.pawn_islands.len(), 1, "only White should have a recorded pawn-islands entry");
    assert_eq!(rec.sensor_report.positional.pawn_islands[0].color, Side::White);
    assert_eq!(rec.sensor_report.positional.pawn_islands[0].count, 2);

    let concepts = extract_concepts(&rec.sensor_report, &rec.groups, rec.side_to_move);
    let islands = concepts.iter().find(|c| c.name == "pawn_islands").expect("pawn_islands concept should be present");
    assert_eq!(islands.side, Side::White);
    assert_eq!(islands.elo_min, 1600);
}

// Regression (2026-07-30, same audit): hanging_piece severity used to be a
// flat count * weight (e.g. two hanging pawns scored identically to two
// hanging queens). Now anchored on the single biggest piece at risk (the
// real immediate danger — only one capture happens per move), with a
// smaller weight for additional simultaneously-hanging pieces, since a
// position with several pieces hanging at once is genuinely worse than one
// with a single hanging piece of the same size. HangingPiece.value is exact
// for this detection (zero defenders means no recapture, so it equals the
// full SEE result, not an approximation of it).
#[test]
fn hanging_piece_severity_is_anchored_on_the_biggest_at_risk() {
    // Black queen d8 (attacked by White Ra1d1's Rd1, open d-file) and knight
    // a4 (attacked by White Ra1, open a-file, blocked at a4) -- verified
    // independently that neither black piece defends the other and the
    // black king isn't adjacent to either.
    let fen = "k2q4/8/8/8/n7/8/8/R2RK3 w - - 0 1";
    let rec = analyze_fen(fen).expect("FEN should parse");

    let hanging = &rec.sensor_report.tactical.hanging;
    assert_eq!(hanging.len(), 2, "expected both the queen and knight to be detected hanging, got {hanging:?}");
    let queen = hanging.iter().find(|h| h.piece.role == "Queen").expect("queen should be hanging");
    assert_eq!(queen.value, 900, "HangingPiece.value should be the real piece value, not a flat placeholder");
    let knight = hanging.iter().find(|h| h.piece.role == "Knight").expect("knight should be hanging");
    assert_eq!(knight.value, 320);

    let concepts = extract_concepts(&rec.sensor_report, &rec.groups, rec.side_to_move);
    let concept = concepts.iter().find(|c| c.name == "hanging_piece").expect("hanging_piece concept should be present");
    assert_eq!(concept.side, Side::Black);
    // max (900) + 0.3 * rest (320) = 996, not a flat 60*2=120 or a naive sum of 1220.
    assert_eq!(concept.severity, 996, "severity should be max-anchored with a smaller weight for the second piece, got {}", concept.severity);
}
