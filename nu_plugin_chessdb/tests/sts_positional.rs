// Dev-only checks against the Strategic Test Suite (STS). Requires
// `nu scripts/prep-test-data.nu` to have been run first (testdata/ is
// gitignored, not committed) — these tests are #[ignore]'d by default so a
// fresh clone / normal `cargo test` never depends on it or the network. Run
// explicitly with `cargo test --test sts_positional -- --ignored --nocapture`.
//
// STS positions are puzzles: the given FEN is deliberately the *undecided*
// moment before the thematic move — a "Center Control" puzzle exists because
// center control is contested and up for grabs there, not because the
// position already embodies it. So "does our concept fire on the raw
// pre-move FEN more often than on an unrelated theme" tests the wrong
// hypothesis and was retracted from here after producing a false-looking
// signal (see PLAN.md's "STS calibration" section for that retraction).
//
// STS's own "best move" ranking isn't used for validation here either —
// that's an engine/human judgment call entangled with search and opponent
// response, a different and much harder problem than "does this position
// have property X." See PLAN.md's "definitive ground truth" section for the
// actual methodology: only trust positions that are unambiguous by
// construction (hand-labeled, or derived by pushing our own side's move(s)
// forward with no opponent-response search — a 2-ply lookahead is a hint to
// flag for separate validation, not an assertion). STS is kept here only as
// a crash-safety smoke test below, nothing more.

use nu_plugin_chessdb::eval::analyze_fen_with_engine_score;
use std::fs;
use std::path::PathBuf;

fn sts_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/sts/STS1-STS15_LAN_v3.epd")
}

fn load_sts() -> Option<String> {
    fs::read_to_string(sts_path()).ok()
}

#[test]
#[ignore]
fn sts_full_suite_evaluates_without_error() {
    let Some(epd) = load_sts() else {
        eprintln!("skipping: run `nu scripts/prep-test-data.nu` first");
        return;
    };
    let mut evaluated = 0;
    for line in epd.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 { continue; }
        let fen = format!("{} {} {} {} 0 1", fields[0], fields[1], fields[2], fields[3]);
        analyze_fen_with_engine_score(&fen, None, Some(1200))
            .unwrap_or_else(|e| panic!("failed to evaluate {fen}: {e}"));
        evaluated += 1;
    }
    assert!(evaluated > 1400, "expected ~1499 STS positions, got {evaluated}");
}
