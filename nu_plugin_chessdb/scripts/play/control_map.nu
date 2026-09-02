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
# Renders through board_overlay.nu's shared convention (2026-09-02) instead
# of a bespoke legend: the one "controls" set is split into 3 layers by what
# occupies each square — own pieces (defended), enemy pieces (attacked),
# empty (controlled space) — so the fixed (),[],{} bracket grammar applies
# unchanged. These three are mutually exclusive by construction (a square
# is exactly one of own/enemy/empty), so the overlap marker <> never fires
# here — that's expected, not a bug; see control_overlap.nu for a case
# where two layers genuinely can overlap on the same square.
use ./board_overlay.nu *

def main [fen: string, square: string] {
    let info = ($fen | chessdb square-control --square $square)
    if $info.piece == null {
        print $"($square) is empty — nothing to compute control from."
        return
    }
    let is_white_piece = ($info.piece.color == "white")

    # Split controls by what's actually on each square, read straight from
    # the FEN's own board text (uppercase = white) rather than a second
    # plugin round trip — cheap and this is display-only classification,
    # not geometry.
    let board_part = ($fen | split row " " | get 0)
    let files = [a b c d e f g h]
    mut occupant_of = {}
    let ranks = ($board_part | split row "/")
    for rank_idx in 0..7 {
        let rank_num = 8 - $rank_idx
        let row = ($ranks | get $rank_idx)
        mut file_idx = 0
        for ch in ($row | split chars) {
            if ($ch =~ '^[0-9]$') {
                let n = ($ch | into int)
                for _ in 0..<$n {
                    $occupant_of = ($occupant_of | insert $"($files | get $file_idx)($rank_num)" ".")
                    $file_idx = $file_idx + 1
                }
            } else {
                $occupant_of = ($occupant_of | insert $"($files | get $file_idx)($rank_num)" $ch)
                $file_idx = $file_idx + 1
            }
        }
    }

    let own_controlled = ($info.controls | where { |sq|
        let occ = ($occupant_of | get $sq)
        $occ != "." and (($occ =~ '^[A-Z]$') == $is_white_piece)
    })
    let enemy_controlled = ($info.controls | where { |sq|
        let occ = ($occupant_of | get $sq)
        $occ != "." and (($occ =~ '^[A-Z]$') != $is_white_piece)
    })
    let empty_controlled = ($info.controls | where { |sq| ($occupant_of | get $sq) == "." })

    print $"($info.piece.color) ($info.piece.role) on ($square) controls ($info.controls | length) squares:"
    print ""
    render-board-grid $fen [
        {name: "own piece defended", squares: $own_controlled}
        {name: "enemy piece attacked", squares: $enemy_controlled}
        {name: "empty square controlled", squares: $empty_controlled}
    ] --highlight $square
}
