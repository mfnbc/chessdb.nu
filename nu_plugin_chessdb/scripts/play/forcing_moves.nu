#!/usr/bin/env nu
# Usage: nu forcing_moves.nu '[e2e4 e7e5 ...]'   (nuon list literal of uci
# moves, parsed with `from nuon`; omit for the start position)
#
# Every legal move for the side to move, tagged as capture/check/quiet from
# mobility_san's own notation (x = capture, +/# = check) -- no ranking, no
# score, just the structural fact of which branches are forcing and which
# aren't. This is deliberately dumb: it doesn't decide what's *good*, only
# what's *forcing* (a check or capture demands a response; a quiet move
# doesn't), which is the first thing a human calculates from -- follow
# forcing lines to a quiet position, then judge that position on its own
# merits.
#
# 2026-09-03: converted to nuon in/out (move-history nuon string in, never
# a hand-typed FEN; one nuon record out, no print, no ascii grid) per the
# same-date nuon-everything decision -- see `board_overlay.nu`.
use ./board_overlay.nu *

def main [moves: string = "[]"] {
    let fen = (history-to-fen ($moves | from nuon))
    let lm = ($fen | chessdb legal-moves)

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
    let quiet_count = ($tagged | where { |t| not $t.capture and not $t.check } | length)
    let mates = ($checks | where { |t| $t.mate } | get san)

    {
        board: (fen-to-board $fen),
        side_to_move: $lm.side_to_move,
        legal_move_count: $lm.legal_move_count,
        checkmate_available: $mates,
        checks: $checks,
        captures_non_checking: $captures,
        quiet_move_count: $quiet_count,
    } | to nuon --indent 2
}
