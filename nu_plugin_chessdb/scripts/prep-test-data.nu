# Fetches external test-position corpora used by dev-only tests
# (tests/sts_positional.rs). Not run as part of `cargo test` or CI — those
# tests are #[ignore]'d and skip with a message if this hasn't been run.
# testdata/ is gitignored; this script is the source of truth for what's in
# it and where it came from. Re-run any time to refresh.
#
# Usage: nu scripts/prep-test-data.nu   (from nu_plugin_chessdb/, or anywhere)
#
# ── Sources & licensing ──
#
# Strategic Test Suite (STS) — themed positional test positions (undermining,
# center control, pawn play in the center, etc.), 100 positions x 15 themes.
# Original suite by Dann Corbit & Swaminathan Natarajan (2008); their
# original site (sites.google.com/site/strategictestsuite) is no longer
# live. Vendored here from github.com/fsmosca/STS-Rating, which is
# MIT-licensed (verified 2026-07-28: see LICENSE in that repo). Redistribution
# of the underlying positions rests on that MIT grant plus 15+ years of open
# reuse across the FOSS chess-engine community with no known restriction
# claims — reasonable confidence, not independently confirmed with the
# original authors.
#
# Note: STS grades move choice (bm + weighted alternatives), not concept
# presence/absence — it's not a ground-truth "this position has property X"
# label set. Useful for realistic, human-curated on-theme positions to
# sanity-check detector behavior against; not a replacement for the
# hand-labeled canonical positions in tests/motif_canonical.rs.

let root = ($env.FILE_PWD | path join "..")
let dest = ($root | path join "testdata" "sts")
mkdir $dest

let url = "https://raw.githubusercontent.com/fsmosca/STS-Rating/master/STS1-STS15_LAN_v3.epd"
let out = ($dest | path join "STS1-STS15_LAN_v3.epd")

print $"Fetching STS EPD from ($url)..."
http get $url | save -f $out
print $"Saved to ($out)"
