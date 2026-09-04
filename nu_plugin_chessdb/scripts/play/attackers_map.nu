#!/usr/bin/env nu
# Usage: nu attackers_map.nu '[e2e4 e7e5 ...]' <square>   (nuon list literal
# of uci moves, parsed with `from nuon`; omit the move list for the start
# position)
#
# Every piece that attacks <square>, with each attacker's own identity
# (color + role) attached -- the reverse question from control_map.nu's
# "what does this piece see": this one answers "is it safe to put a piece
# here," directly, for a square whether or not it's currently occupied.
# Built on shakmaty's `Board::attacks_to`, composed in nu from the
# geom-attacks/board-pieces leaf commands via `shakmaty_compose.nu` --
# this script does no geometric computation of its own, same discipline as
# control_map.nu and for the same reason (FINDINGS.md, 2026-09-02).
#
# For the full occupancy-aware exchange picture on a square, ply by ply
# with x-rays revealed, use `square_swap_list.nu` instead -- this script
# only answers "who attacks it right now," not "what's revealed once those
# attackers are gone."
#
# 2026-09-03: converted to nuon in/out per the nuon-everything decision.
# 2026-09-03 (later same day): rewired onto `shakmaty_compose.nu` after
# `chessdb square-attackers` was removed in favor of the shakmaty-1:1
# architecture -- see `board_overlay.nu`/`shakmaty_compose.nu`.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, square: string] {
    let fen = (history-to-fen ($moves | from nuon))
    let board = (fen-to-board $fen)
    let occ = ($fen | chessdb board-pieces).squares

    let with_identity = { |squares| $squares | each { |sq| {square: $sq} | merge ($board | get $sq) } }

    {
        square: $square,
        occupant: (if $square in $board { $board | get $square } else { null }),
        attacked_by_white: (do $with_identity (attacks-to $fen $square white $occ)),
        attacked_by_black: (do $with_identity (attacks-to $fen $square black $occ)),
    } | to nuon --indent 2
}
