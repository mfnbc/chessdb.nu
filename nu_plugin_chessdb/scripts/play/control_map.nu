#!/usr/bin/env nu
# Usage: nu control_map.nu "<FEN>" "<square>"
#
# Renders the board as an 8x8 grid with every square the piece on <square>
# geometrically controls marked, instead of a flat list of algebraic square
# names to mentally place. Built after a real blunder (FINDINGS.md,
# 2026-09-02) traced to exactly this kind of mental-arithmetic slip:
# checking "does this diagonal reach that square" by hand and getting it
# wrong. The actual attack geometry comes entirely from
# `chessdb square-control` (shakmaty's own move generation, occupancy-aware)
# — this script only places characters on a grid, it does no geometric
# computation of its own, on purpose: that's the class of code that caused
# the blunder in the first place.
#
# Cell legend:
#   .    empty, uncontrolled
#   x    empty, controlled
#   P/p  occupied, uncontrolled by this piece (shown as-is from the FEN)
#  (P)  occupied by this piece's own side, controlled (defended)
#  [p]  occupied by the opponent, controlled (attacked)
#  *N*  the piece being inspected, on its own square
const FILES = [a b c d e f g h]

def main [fen: string, square: string] {
    let board_part = ($fen | split row " " | get 0)
    let ranks = ($board_part | split row "/")
    if ($ranks | length) != 8 {
        print $"malformed FEN board section: ($board_part)"
        return
    }

    # rows top-to-bottom are rank 8..1; expand digit run-lengths into '.'
    mut grid = {}
    for rank_idx in 0..7 {
        let rank_num = 8 - $rank_idx
        let row = ($ranks | get $rank_idx)
        mut file_idx = 0
        for ch in ($row | split chars) {
            if ($ch =~ '^[0-9]$') {
                let n = ($ch | into int)
                for _ in 0..<$n {
                    let sq = $"($FILES | get $file_idx)($rank_num)"
                    $grid = ($grid | insert $sq ".")
                    $file_idx = $file_idx + 1
                }
            } else {
                let sq = $"($FILES | get $file_idx)($rank_num)"
                $grid = ($grid | insert $sq $ch)
                $file_idx = $file_idx + 1
            }
        }
    }

    let info = ($fen | chessdb square-control --square $square)
    if $info.piece == null {
        print $"($square) is empty — nothing to compute control from."
        return
    }
    let is_white_piece = ($info.piece.color == "white")
    let controlled_set = $info.controls

    print $"($info.piece.color) ($info.piece.role) on ($square) controls ($controlled_set | length) squares:"
    print ""

    for rank_idx in 0..7 {
        let rank_num = 8 - $rank_idx
        mut row_str = $"($rank_num) "
        for file_idx in 0..7 {
            let sq = $"($FILES | get $file_idx)($rank_num)"
            let occupant = ($grid | get $sq)
            let is_controlled = ($sq in $controlled_set)
            let cell = if $sq == $square {
                $"*($occupant)*"
            } else if $occupant == "." {
                if $is_controlled { " x " } else { " . " }
            } else if $is_controlled {
                let occupant_is_white = ($occupant =~ '^[A-Z]$')
                if $occupant_is_white == $is_white_piece {
                    $"\(($occupant)\)"
                } else {
                    $"[($occupant)]"
                }
            } else {
                $" ($occupant) "
            }
            $row_str = $row_str + $cell + " "
        }
        print $row_str
    }
    print "    a   b   c   d   e   f   g   h"
}
