#!/usr/bin/env nu
# Usage: nu test_blunder_corpus.nu
#
# Re-runs `full_report.nu`/`shakmaty_compose.nu` against every position in
# `blunder_corpus.nuon` and prints the CURRENT raw fact for each entry's
# `expected_field`, alongside what was recorded when the corpus was built.
# Deliberately does not print a pass/fail verdict -- per this whole
# project's own standing discipline (never surface a computed judgment),
# the reader compares "current" against "recorded" and decides whether a
# future tooling change preserved, improved, or regressed the signal.
# Re-run this after any change to core.rs/shakmaty_compose.nu that could
# plausibly touch tactical detection, x-ray/swap-list logic, or the leaf
# commands -- these are real historical positions, not synthetic tests, so
# a regression here is a real regression, not a hypothetical one.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [] {
    let corpus = (open blunder_corpus.nuon)

    for entry in $corpus {
        print $"=== game ($entry.game), ($entry.move) ==="
        # Each entry carries exactly one `verified_<date>` column, dated to
        # when that entry was built -- looked up by name prefix rather than
        # a hardcoded date so new entries never need this script touched
        # (2026-09-04 game 19 addition found this exact fragility: a
        # hardcoded `verified_2026_09_03` broke the moment an entry dated a
        # day later was added).
        let verified_col = ($entry | columns | where { |c| $c | str starts-with "verified_" } | get 0?)
        let verified = if $verified_col == null { null } else { $entry | get $verified_col }
        if $verified == null {
            print $"  SKIPPED at build time: ($entry.note? | default 'no reason recorded')"
            print ""
            continue
        }

        let fen = if $entry.fen? != null {
            $entry.fen
        } else if $entry.fen_before_move? != null {
            $entry.fen_before_move
        } else if $entry.history? != null {
            history-to-fen $entry.history
        } else {
            print "  SKIPPED: no fen/fen_before_move/history to reconstruct from"
            print ""
            continue
        }

        let report = (full-report $fen)
        print $"  failure: ($entry.failure)"
        print $"  recorded at corpus build time: ($verified.result)"

        match $entry.game {
            6 => { print $"  current mate_in_1_exists: ($report.mate_in_1_exists)" }
            10 => { print $"  current tactical.hanging: ($report.tactical.hanging | each { |h| $h.piece })" }
            11 => { print $"  current tactical.hanging: ($report.tactical.hanging | each { |h| $h.piece })" }
            13 => {
                print $"  current hanging/forks/pins counts: (($report.tactical.hanging | length))/(($report.tactical.forks | length))/(($report.tactical.pins | length))"
                let km = (king-mobility $fen)
                print $"  current king-mobility: ($km.destinations)"
            }
            15 => {
                let a7 = (piece-mobility-safety $fen "a7")
                let summary = ($a7.destinations | each { |d| $"($d.square):($d.attacked_by_opponent)" })
                print $"  current piece-mobility-safety for a7: ($summary)"
            }
            16 => {
                print $"  current tactical.mover_favored: ($report.tactical.mover_favored)"
                let sl = (swap-list $fen "e5")
                print $"  current swap-list notation for e5: ($sl.notation)"
            }
            _ => { print "  (no specific re-check wired for this game -- see full-report above)" }
        }
        print ""
    }
}
