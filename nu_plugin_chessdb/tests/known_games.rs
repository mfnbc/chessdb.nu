// Regression suite: real, named, well-documented chess traps ("beginner
// traps" / "how to beat your dad at chess" style games), run through the
// full analysis pipeline (analyze_fen -> sensor_report.tactical) at their
// key tactical moments. Every game's moves were cross-checked against at
// least one independent source before being used here — see FINDINGS.md's
// "regression set of known games" entry for exactly which sources and what
// each one's confidence level was. Every specific finding asserted below
// was hand-derived against the real chess position *before* being written
// as an assertion, then confirmed live against the actual nu-plugin
// pipeline (`chess-tactical-events`) — this file encodes those same,
// already-verified facts permanently, so they run under `cargo test`
// instead of needing a live plugin round-trip and a scratch database every
// time.
//
// Sources:
// - Scholar's Mate: standard, no citation needed.
// - Fried Liver Attack: standard opening theory, no citation needed.
// - ICBM Gambit (Tennison Gambit, ICBM variation): chess.com
//   (TimeChess146, "Learning the ICBM Gambit"); Wikipedia, "Tennison
//   Gambit" (for the opening moves).
// - Alien Gambit: chess-teacher.com, "Alien Gambit" (confirmed only through
//   the sacrifice itself, 6.Nxf7 — no source gave the follow-up, so this
//   suite doesn't either).
// - Légal's Trap: cross-checked against a Quora answer quoting the exact
//   line and general chess literature; extremely well documented (18th
//   century, Sire de Légal).
// - Blackburne Shilling Gambit: chess.com/chesstrapguide.com search
//   results agreeing precisely on the full mating line.
// - Queen's Gambit Declined, Elephant Trap: Wikipedia,
//   "Queen's Gambit Declined, Elephant Trap" (historical game Mayet–
//   Harrwitz, Berlin 1848).
// - Ruy Lopez, Noah's Ark Trap: Wikipedia, "Ruy Lopez, Noah's Ark Trap"
//   (historical game Steiner–Capablanca, Budapest 1929).
// - Drunken Bishops Gambit ("Main Trap" line): raw PGN pulled directly from
//   a real lichess study export (lichess.org/study/xUxinbKO/mKqaYH15.pgn,
//   study by Adam_Prikler) — not a video summary or blog paraphrase.
// - Alekhine's Gun (Alekhine vs. Nimzowitsch, San Remo 1930): raw PGN
//   pulled directly from a real lichess study export
//   (lichess.org/study/cpuqVg6h/u0Ppk2Ul.pgn, study by CPCurley), matching
//   a chess.com blog's independently-quoted first 26 moves exactly.

use nu_plugin_chessdb::eval::{analyze_fen, Consequence, Side};
use shakmaty::fen::Fen;
use shakmaty::san::San;
use shakmaty::{Chess, EnPassantMode, Position};

/// Replay SAN moves from the starting position, returning the *real* FEN
/// after each ply (`out[0]` is after move 1, `out[i]` is after ply `i+1`).
/// Deliberately never canonicalized — these tests assert on actual
/// square/color labels a player watching the real game would see, and
/// `positions.fen`-style canonicalization would silently mirror them for
/// any ply where Black is to move (see FINDINGS.md's "tactical_events fed
/// canonical FEN" entry — the exact mistake this helper avoids by
/// construction, replaying with shakmaty directly instead of touching
/// anything canonical).
fn replay_san(moves: &[&str]) -> Vec<String> {
    let mut pos = Chess::default();
    let mut fens = Vec::new();
    for mv_str in moves {
        let bare = mv_str.trim_end_matches(['+', '#']);
        let san: San = bare.parse().unwrap_or_else(|e| panic!("bad SAN '{bare}': {e}"));
        let mv = san.to_move(&pos).unwrap_or_else(|e| panic!("illegal move '{bare}' after {fens:?}: {e}"));
        pos = pos.play(mv).unwrap_or_else(|e| panic!("play failed for '{bare}': {e}"));
        fens.push(Fen::from_position(&pos, EnPassantMode::Legal).to_string());
    }
    fens
}

#[test]
fn scholars_mate_hangs_e5_then_the_queen_itself() {
    let fens = replay_san(&["e4", "e5", "Qh5", "Nc6", "Bc4", "Nf6", "Qxf7#"]);

    // Ply 3 (2.Qh5): Qh5 attacks e5 along the rank; nothing defends it yet.
    let rec = analyze_fen(&fens[2]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "e5" && h.piece.color == Side::Black),
        "e5 should be hanging after 2.Qh5, got {hanging:?}");

    // Ply 6 (3...Nf6): Nf6 attacks both e4 and h5 -- the queen itself
    // becomes hanging (the real reason the mate needs immediate
    // follow-through), and Bc4+Qh5 together outnumber f7's lone defender.
    let rec = analyze_fen(&fens[5]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "h5" && h.piece.color == Side::White && h.value == 900),
        "White's queen on h5 should be hanging to Nf6, got {hanging:?}");
    let outnumbered = &rec.sensor_report.tactical.outnumbered;
    assert!(outnumbered.iter().any(|o| o.piece.square == "f7" && o.piece.color == Side::Black),
        "f7 should be outnumbered (Bc4+Qh5 vs. king only), got {outnumbered:?}");
}

#[test]
fn fried_liver_e4_outnumbered_not_hanging_because_the_attacking_knight_also_defends_it() {
    let fens = replay_san(&["e4", "e5", "Nf3", "Nc6", "Bc4", "Nf6", "Ng5", "d5"]);

    // Ply 8 (4...d5): e4 is attacked by both Nf6 and the d5 pawn, but Ng5
    // (the same knight that vacated f3) defends it via a second knight-move
    // line while simultaneously attacking f7 -- 2 attackers, 1 defender,
    // outnumbered rather than hanging. Verified by hand and initially
    // mis-derived (expected 0 defenders) before re-checking the position.
    let rec = analyze_fen(&fens[7]).expect("valid FEN");
    let outnumbered = &rec.sensor_report.tactical.outnumbered;
    let e4 = outnumbered.iter().find(|o| o.piece.square == "e4" && o.piece.color == Side::White)
        .unwrap_or_else(|| panic!("e4 should be outnumbered, got {outnumbered:?}"));
    assert_eq!(e4.attacker_count, 2);
    assert_eq!(e4.defender_count, 1);
    assert!(rec.sensor_report.tactical.hanging.iter().all(|h| h.piece.square != "e4"),
        "e4 must not also read as hanging -- it has a real defender (Ng5)");
}

#[test]
fn icbm_gambit_queen_only_hangs_once_the_check_vacates_the_open_file() {
    let fens = replay_san(&[
        "e4", "d5", "Nf3", "dxe4", "Ng5", "Nf6", "d3", "exd3", "Bxd3", "h6",
        "Nxf7", "Kxf7", "Bg6+",
    ]);

    // Ply 13 (7.Bg6+): the d-file has been open since ...dxe4/exd3 cleared
    // it and Bxd3 blocked it again — the moment the bishop steps off d3 to
    // check, Qd1 already had a completely clear line to d8. The check
    // itself is a free tempo, not what actually wins the queen.
    let rec = analyze_fen(&fens[12]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "d8" && h.piece.color == Side::Black && h.value == 900),
        "Black's queen on d8 should be hanging the moment Bg6+ vacates d3, got {hanging:?}");
}

#[test]
fn alien_gambit_knight_sac_forks_the_rook_and_is_itself_hanging() {
    let fens = replay_san(&["e4", "c6", "d4", "d5", "Nc3", "dxe4", "Nxe4", "Nf6", "Ng5", "h6", "Nxf7"]);

    // Ply 11 (6.Nxf7): the knight forks h8 and d8, but the king (still on
    // e8, hasn't moved) counts as an adjacent defender of d8 -- only h8
    // reads as genuinely hanging. The knight itself, now on f7, is also
    // hanging to the king -- exactly why 6...Kxf7 is the natural reply,
    // with the gambit's actual justification (the ensuing attack) being
    // outside what this system calculates.
    let rec = analyze_fen(&fens[10]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "h8" && h.piece.color == Side::Black && h.value == 500),
        "Black's rook on h8 should be hanging to the fork, got {hanging:?}");
    assert!(hanging.iter().any(|h| h.piece.square == "f7" && h.piece.color == Side::White && h.value == 320),
        "White's own knight on f7 should be hanging to the king, got {hanging:?}");
    assert!(hanging.iter().all(|h| h.piece.square != "d8"),
        "d8 should not read as hanging -- the king still defends it, got {hanging:?}");
}

#[test]
fn legals_trap_bishop_hangs_via_the_diagonal_the_knight_move_opened() {
    let fens = replay_san(&["e4", "e5", "Nf3", "Nc6", "Bc4", "d6", "Nc3", "Bg4", "Nxe5"]);

    // Ply 9 (5.Nxe5): the knight leaving f3 opens Qd1's diagonal onto Bg4 --
    // a real, subtle consequence of the capture, not just "the knight
    // grabbed a pawn." Also the classic double-attack on f7 (Bc4 + the
    // knight, now on e5) appears in the same position.
    let rec = analyze_fen(&fens[8]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "g4" && h.piece.color == Side::Black && h.value == 330),
        "Black's bishop on g4 should be hanging via the newly-opened d1-g4 diagonal, got {hanging:?}");
    let outnumbered = &rec.sensor_report.tactical.outnumbered;
    assert!(outnumbered.iter().any(|o| o.piece.square == "f7" && o.piece.color == Side::Black),
        "f7 should be outnumbered (Bc4 + Ne5 vs. king only), got {outnumbered:?}");
}

#[test]
fn blackburne_shilling_gambit_queen_forks_rook_and_pawn() {
    let fens = replay_san(&["e4", "e5", "Nf3", "Nc6", "Bc4", "Nd4", "Nxe5", "Qg5", "Nxf7"]);

    // Ply 9 (5.Nxf7): the knight on f7 forks Black's own queen-vacated d8
    // (empty, irrelevant) but more importantly Black's own prior move
    // (4...Qg5) now finds White's rook h8 *and* pawn g2 both hanging to the
    // queen, while the knight on f7 simultaneously hangs the rook on h8 too
    // -- a genuinely busy position with several real facts at once.
    let rec = analyze_fen(&fens[8]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "h8" && h.piece.color == Side::Black && h.value == 500),
        "Black's rook on h8 should be hanging to the knight fork, got {hanging:?}");
    assert!(hanging.iter().any(|h| h.piece.square == "g5" && h.piece.color == Side::Black && h.value == 900),
        "Black's own queen on g5 should be hanging to the knight fork too, got {hanging:?}");
    assert!(hanging.iter().any(|h| h.piece.square == "g2" && h.piece.color == Side::White && h.value == 100),
        "White's g2 pawn should be hanging to the queen (setting up ...Qxg2 next), got {hanging:?}");
}

#[test]
fn elephant_trap_bishop_hangs_once_the_knight_recapture_clears_the_diagonal() {
    let fens = replay_san(&[
        "d4", "d5", "c4", "e6", "Nc3", "Nf6", "Bg5", "Nbd7", "cxd5", "exd5", "Nxd5", "Nxd5",
    ]);

    // Ply 12 (6...Nxd5): the knight that was on f6 (blocking Black's own
    // queen from reaching g5) has just moved to recapture on d5 -- clearing
    // the d8-g5 diagonal and leaving White's own bishop hanging to the
    // queen it never saw coming. This is the exact moment that provokes
    // White's actual mistake (7.Bxd8??) in the real trap.
    let rec = analyze_fen(&fens[11]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "g5" && h.piece.color == Side::White && h.value == 330),
        "White's bishop on g5 should be hanging via the newly-cleared d8-g5 diagonal, got {hanging:?}");
}

#[test]
fn noahs_ark_trap_bishop_hangs_before_it_retreats() {
    let fens = replay_san(&["e4", "e5", "Nf3", "Nc6", "Bb5", "a6"]);

    // Ply 6 (3...a6): the a6 pawn attacks b5 directly; nothing defends the
    // bishop there -- exactly why the real trap line continues 4.Ba4
    // (retreating) rather than staying put. The trap itself (the bishop
    // getting permanently boxed in on b3 several moves later) is a
    // positional entrapment fact this tactical ladder isn't designed to
    // catch -- a good, honest example of the boundary noted when this
    // suite was scoped, not a gap introduced by this test.
    let rec = analyze_fen(&fens[5]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "b5" && h.piece.color == Side::White && h.value == 330),
        "White's bishop on b5 should be hanging to the a6 pawn, got {hanging:?}");
}

#[test]
fn drunken_bishops_gambit_pinned_interposer_is_a_false_defender() {
    let fens = replay_san(&[
        "e4", "d5", "Nf3", "e5", "exd5", "Nf6", "Nxe5", "Qxd5", "Nf3", "Nc6", "Nc3", "Qh5",
        "Nb5", "Bb4", "Nxc7+", "Kd8", "Nxa8", "Bh3", "a3", "Re8+", "Be2", "Qxf3", "gxf3", "Nd4",
    ]);

    // Ply 21 (11.Be2): forced by 10...Re8+ to interpose on the e-file,
    // pinning the bishop to the king. Ply 24 (12...Nd4): the knight now
    // attacks the f3 pawn Be2 nominally defends (1 attacker, 1 defender --
    // false_safety's own precondition, raw count says safe). But Be2 can't
    // actually recapture on f3 without stepping off the e-file it's pinned
    // to -- exactly why 13...Nxf3 next is checkmate, not just a piece won.
    let rec = analyze_fen(&fens[23]).expect("valid FEN");
    let false_defense = &rec.sensor_report.tactical.false_defense;
    assert!(false_defense.iter().any(|f| f.piece.square == "f3" && f.piece.color == Side::White),
        "f3 pawn should read as falsely defended (Be2 is pinned off the recapture square), got {false_defense:?}");
    let false_safety = &rec.sensor_report.tactical.false_safety;
    assert!(false_safety.iter().any(|f| f.piece.square == "f3" && f.piece.color == Side::White),
        "f3 pawn should also read as false_safety (raw count said safe, the pin reverses it), got {false_safety:?}");
}

#[test]
fn alekhines_gun_mutual_hang_after_the_queenside_pawn_break() {
    // The full 27-move lead-up to the famous "Alekhine's gun" formation
    // (Rc2+Rc3+Qc1, completed at move 26) — included in full because the
    // interesting local tactic (below) only shows up one move after the
    // formation completes, not at the formation itself.
    let fens = replay_san(&[
        "e4", "e6", "d4", "d5", "Nc3", "Bb4", "e5", "c5", "Bd2", "Ne7", "Nb5", "Bxd2+", "Qxd2",
        "O-O", "c3", "b6", "f4", "Ba6", "Nf3", "Qd7", "a4", "Nbc6", "b4", "cxb4", "cxb4", "Bb7",
        "Nd6", "f5", "a5", "Nc8", "Nxb7", "Qxb7", "a6", "Qf7", "Bb5", "N8e7", "O-O", "h6",
        "Rfc1", "Rfc8", "Rc2", "Qe8", "Rac1", "Rab8", "Qe3", "Rc7", "Rc3", "Qd7", "R1c2", "Kf8",
        "Qc1", "Rbc8", "Ba4", "b5",
    ]);

    // Note, not asserted: the "gun" itself (Rc2+Rc3+Qc1 aimed down the
    // c-file, completed at ply 51/move 26.Qc1) doesn't trigger anything in
    // this material-only ladder -- no outnumbered/overloaded shows up on
    // Black's c-file stack. That's expected, not a gap this test papers
    // over: Alekhine's gun is a slow positional squeeze building over many
    // moves, exactly the kind of thing that needs the *future* positional
    // sensors (file/zone control accumulated over time), not this ladder.
    // What the ladder *does* correctly find is a genuine local tactic one
    // move later: right after 27...b5, White's bishop on a4 and Black's
    // pawn on b5 mutually attack each other, neither defended -- exactly
    // why 28.Bxb5 follows in the real game.
    let rec = analyze_fen(&fens[53]).expect("valid FEN");
    let hanging = &rec.sensor_report.tactical.hanging;
    assert!(hanging.iter().any(|h| h.piece.square == "a4" && h.piece.color == Side::White && h.value == 330),
        "White's bishop on a4 should be hanging to the b5 pawn, got {hanging:?}");
    assert!(hanging.iter().any(|h| h.piece.square == "b5" && h.piece.color == Side::Black && h.value == 100),
        "Black's pawn on b5 should be hanging right back to the bishop, got {hanging:?}");
}

#[test]
fn fruit_game_knight_fork_wins_material_even_though_both_targets_are_defended() {
    // From a real session-logged game against the Fruit UCI engine: 17...Ne5
    // forks White's rook on d3 and queen on f3. Both targets are
    // individually defended once (c2 pawn on d3, g2 pawn on f3) — exactly
    // the case `find_forks` used to miss entirely, since its target
    // selection only ran SEE on strictly-undefended targets and silently
    // reported this fork as `consequence: Even, see_cp: 0` even though the
    // real continuation (...Nxf3+ gxf3) wins the queen for a knight.
    let fen = "r1b3k1/p1p2ppp/1p2p3/3rn3/2q5/2PR1Q2/P1P2PP1/R5K1 w - - 2 18";
    let rec = analyze_fen(fen).expect("valid FEN");
    let forks = &rec.sensor_report.tactical.forks;
    let knight_fork = forks
        .iter()
        .find(|f| f.attacker.square == "e5" && f.attacker.color == Side::Black)
        .expect("Ne5 should be detected as a forking piece");
    assert_eq!(knight_fork.consequence, Consequence::Winning,
        "Ne5's fork on Rd3/Qf3 should read as Winning for Black even though both targets are defended, got {knight_fork:?}");
    assert!(knight_fork.see_cp > 0, "see_cp should be positive, got {}", knight_fork.see_cp);
    let hangs = knight_fork.hangs.as_ref().expect("fork should identify a real target");
    assert_eq!(hangs.square, "f3",
        "the fork's real point is the queen on f3, not just the lower-value rook, got {hangs:?}");
}

#[test]
fn fruit_game_three_queen_lost_to_a_bishop_despite_two_defenders() {
    // From a third real session-logged game against Fruit: right after
    // 8.Nxe5, White's queen on d1 is attacked by a single bishop on g4 (the
    // knight that used to block that diagonal, pinning it to the queen all
    // along, just moved away to capture on e5). The queen has TWO
    // defenders — the king on e1 and a knight on c3 (missed by eye at the
    // board during the actual game) — so neither find_hanging (needs zero
    // defenders) nor find_outnumbered (needs attackers > defenders, and
    // here it's the reverse: 1 attacker, 2 defenders) can ever flag this.
    // The trade is still catastrophic for White: a bishop (330) is winning
    // a queen (900) outright, regardless of how many pieces could
    // eventually recapture. This is exactly why find_mover_favored had to
    // be generalized beyond its first, narrower "exactly 1-vs-1" version —
    // see FINDINGS.md's 2026-09-01 entries.
    let fen = "r2qkb1r/ppp1pppp/1n6/3PN3/2P3b1/2N5/PP3PPP/R1BQKB1R b KQkq - 0 8";
    let rec = analyze_fen(fen).expect("valid FEN");
    let mf = &rec.sensor_report.tactical.mover_favored;
    let queen = mf.iter().find(|m| m.piece.square == "d1" && m.piece.color == Side::White)
        .unwrap_or_else(|| panic!("Qd1 should be flagged mover-favored despite 2 defenders, got {mf:?}"));
    assert_eq!(queen.attacker_count, 1);
    assert_eq!(queen.defender_count, 2);
    assert_eq!(queen.consequence, Consequence::Winning, "got {queen:?}");
    assert_eq!(queen.see_cp, 570, "queen(900) - bishop(330) = 570, got {}", queen.see_cp);
}

#[test]
fn fruit_game_four_outnumbered_knight_was_mislabeled_safe_by_the_buggy_see_chain() {
    // From a fourth real session-logged game against Fruit: a candidate
    // 19.Nd4 was checked before playing it. White's knight would land on
    // d4, attacked by 2 (pawn on e5, bishop on c5) and defended by only 1
    // (pawn on c3) -- a real 2-vs-1 outnumbered piece, and the pawn is the
    // cheaper attacker, so it plainly just wins the knight (exd4, cxd4 nets
    // Black a knight for a pawn). At the time, `find_outnumbered` priced
    // this via `self.see()` and reported `consequence: Losing, see_cp:
    // -360` -- read by the coaching script as "bad for the attacker, i.e.
    // safe for me" and filtered out of the danger list entirely, even
    // though directly simulating exd4 crashed the eval by nearly 1300cp.
    // `find_outnumbered` was switched to the same direct-subtraction,
    // first-exchange-only pricing `find_mover_favored` uses instead of the
    // buggy `see()` call; this anchors that fix to the exact position that
    // caught it. See FINDINGS.md's 2026-09-01 entries.
    let fen = "r4rk1/2p1q1pp/p3b3/2b1pp2/2PNn3/2P5/P1Q1BPPP/1R3RK1 b - - 1 19";
    let rec = analyze_fen(fen).expect("valid FEN");
    let outnumbered = &rec.sensor_report.tactical.outnumbered;
    let knight = outnumbered.iter().find(|o| o.piece.square == "d4" && o.piece.color == Side::White)
        .unwrap_or_else(|| panic!("Nd4 should be flagged outnumbered (2v1), got {outnumbered:?}"));
    assert_eq!(knight.attacker_count, 2);
    assert_eq!(knight.defender_count, 1);
    assert_eq!(knight.consequence, Consequence::Winning, "must read as dangerous for White (winning for the mover), got {knight:?}");
    assert_eq!(knight.see_cp, 220, "knight(320) - pawn(100) = 220, got {}", knight.see_cp);
}

