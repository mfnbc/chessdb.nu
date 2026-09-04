#!/usr/bin/env nu
# Usage: nu calc_line.nu '[e2e4 e7e5 ...]' '[candidate uci moves ...]'   (both
# nuon list literals, parsed with `from nuon`)
#
# Walks a full calculated variation move by move (not just one candidate
# move like check_move.nu) and returns structural facts -- hanging pieces,
# forks, king exposure, raw material by piece count, check/mate -- at every
# node. No score anywhere. This exists to make real multi-ply calculation
# reliable: write out the line you're actually calculating (my move, their
# forced or expected reply, my follow-up, ...) and verify every position in
# it is what you think it is, in one call, instead of losing track between
# separate single-move checks. Stops and flags the exact ply where an
# illegal move or a miscalculation (something you didn't expect to hang) is
# found.
#
# 2026-09-03: converted to nuon in/out (move-history nuon strings in, never
# a hand-typed FEN; one nuon record out, no print, no ascii grid) per the
# same-date nuon-everything decision -- see `board_overlay.nu`.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, line: string] {
    let history = ($moves | from nuon)
    let candidate_line = ($line | from nuon)

    mut fen = (history-to-fen $history)
    let starting_board = (fen-to-board $fen)

    mut plies = []
    mut ply = 0
    mut stopped_illegal_at: any = null
    for m in $candidate_line {
        $ply = $ply + 1
        let applied = (try { $fen | chessdb apply-uci --uci $m } catch { null })
        if $applied == null {
            $stopped_illegal_at = {ply: $ply, move: $m}
            break
        }
        $fen = $applied
        let ev = ($fen | chessdb hugm-eval --verbose true)
        # strip-scores: material.balance carries a centipawns field and
        # tactical entries can carry consequence/see_cp -- neither belongs
        # in this output, matching check_move.nu's own promise.
        let s = (strip-scores $ev.sensor_report)
        let mover_just_played = if $ev.side_to_move == "white" { "black" } else { "white" }
        let dest_square = ($m | str substring 2..3)

        $plies = ($plies | append {
            ply: $ply,
            move: $m,
            mover: $mover_just_played,
            destination_square: $dest_square,
            board: (fen-to-board $fen),
            mate_in_1: {exists: $s.mate_in_1_exists, side_to_move: $ev.side_to_move},
            in_check: {value: $s.in_check, side_to_move: $ev.side_to_move},
            material: $s.material.balance,
            hanging: $s.tactical.hanging,
            forks: ($s.tactical.forks | each { |f| {attacker: $f.attacker, targets: $f.targets} }),
            king_exposure: $s.positional.king_exposure,
        })
    }

    {
        starting_board: $starting_board,
        plies: $plies,
        stopped_illegal_at: $stopped_illegal_at,
    } | to nuon --indent 2
}
