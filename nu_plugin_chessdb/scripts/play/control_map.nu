#!/usr/bin/env nu
# Usage: nu control_map.nu '[e2e4 e7e5 ...]' <square>   (nuon list literal of
# uci moves, parsed with `from nuon`; omit the move list for the start
# position)
#
# Every square the piece on <square> geometrically controls, split by what
# occupies each controlled square (own piece defended / enemy piece
# attacked / empty square controlled). Built after a real blunder
# (FINDINGS.md, 2026-09-02) traced to exactly this kind of mental-
# arithmetic slip: checking "does this diagonal reach that square" by hand
# and getting it wrong. The actual attack geometry comes entirely from
# shakmaty's own move generation (occupancy-aware) via
# `shakmaty_compose.nu`'s `attacks-from` -- this script does no geometric
# computation of its own, on purpose: that's the class of code that caused
# the blunder in the first place.
#
# 2026-09-03: converted to nuon in/out (move-history nuon string in, never
# a hand-typed FEN; one nuon record out, no print, no ascii grid) per the
# nuon-everything decision. 2026-09-03 (later same day): rewired onto
# `shakmaty_compose.nu` (nu-composed from the geom-attacks/board-pieces
# leaf commands) after `chessdb square-control` was removed in favor of the
# shakmaty-1:1 architecture -- see `board_overlay.nu`/`shakmaty_compose.nu`.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, square: string] {
    let fen = (history-to-fen ($moves | from nuon))
    let board = (fen-to-board $fen)
    let piece = (if $square in $board { $board | get $square } else { null })

    if $piece == null {
        return ({square: $square, piece: null, controls: []} | to nuon --indent 2)
    }

    let occ = ($fen | chessdb board-pieces).squares
    let controls = (attacks-from $fen $square $occ)
    let own_controlled = ($controls | where { |sq| $sq in $board and ($board | get $sq).color == $piece.color })
    let enemy_controlled = ($controls | where { |sq| $sq in $board and ($board | get $sq).color != $piece.color })
    let empty_controlled = ($controls | where { |sq| $sq not-in $board })

    {
        square: $square,
        piece: $piece,
        own_piece_defended: $own_controlled,
        enemy_piece_attacked: $enemy_controlled,
        empty_square_controlled: $empty_controlled,
    } | to nuon --indent 2
}
