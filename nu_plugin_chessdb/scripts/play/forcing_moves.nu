#!/usr/bin/env nu
# Usage: nu forcing_moves.nu "<space-separated uci move history>"
#
# Lists every legal move for the side to move, tagged as capture/check/quiet
# from mobility_san's own notation (x = capture, +/# = check) — no ranking,
# no score, just the structural fact of which branches are forcing and which
# aren't. This is deliberately dumb: it doesn't decide what's *good*, only
# what's *forcing* (a check or capture demands a response; a quiet move
# doesn't), which is the first thing a human calculates from — follow forcing
# lines to a quiet position, then judge that position on its own merits.
#
# No raw FEN printed (2026-09-02, user feedback) -- rendered as a grid
# instead, via board_overlay.nu's shared convention.
use ./board_overlay.nu *

def main [moves: string] {
    let move_list = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $move_list {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    let lm = ($fen | chessdb legal-moves)
    render-board-grid $fen []
    print ""
    print $"side to move: ($lm.side_to_move)  \(($lm.legal_move_count) legal moves\)"
    print ""

    let tagged = ($lm.mobility_san | enumerate | each { |row|
        let san = $row.item
        let uci = ($lm.mobility_uci | get $row.index)
        let is_capture = ($san | str contains "x")
        let is_check = ($san | str ends-with "+") or ($san | str ends-with "#")
        let is_mate = ($san | str ends-with "#")
        {uci: $uci, san: $san, capture: $is_capture, check: $is_check, mate: $is_mate}
    })

    let checks = ($tagged | where { |t| $t.check })
    let captures = ($tagged | where { |t| $t.capture and not $t.check })
    let quiet = ($tagged | where { |t| not $t.capture and not $t.check })

    if ($checks | any { |t| $t.mate }) {
        let mates = ($checks | where { |t| $t.mate } | get san | str join ', ')
        print $"!!! CHECKMATE AVAILABLE: ($mates) !!!"
        print ""
    }

    print $"CHECKS \(($checks | length)\):"
    for t in $checks { print $"  ($t.san)  \(($t.uci)\)" }
    print ""
    print $"CAPTURES, non-checking \(($captures | length)\):"
    for t in $captures { print $"  ($t.san)  \(($t.uci)\)" }
    print ""
    print $"quiet moves: ($quiet | length) \(not listed — calculate forcing lines first\)"
}
