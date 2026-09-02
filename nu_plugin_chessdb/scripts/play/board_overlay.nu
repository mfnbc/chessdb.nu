#!/usr/bin/env nu
# Shared board-overlay renderer — the convention `control_map.nu`,
# `attackers_map.nu`, and `control_overlap.nu` all render through, instead
# of each inventing its own bespoke legend (which is exactly what the first
# two did until this file existed, and got confusing across scripts:
# `()` meant something different in each one).
#
# The convention: every layer you'd want to show is just a *square set* —
# a `list<string>` of algebraic squares, exactly what `controls`,
# `attacked_by_white`, `attacked_by_black`, etc. already return from the
# plugin. That's the interoperability point with the real 64-bit
# `shakmaty::Bitboard` these come from: no adapter, a plain list of square
# names round-trips to a real bitboard on the Rust side already, and any
# future command that returns one becomes a valid layer here for free.
#
# Fixed glyph convention (not per-script, not caller-chosen — the whole
# point is one predictable grammar):
#   layer 1 (first in the list) → own name shown, cell wrapped in  ( )
#   layer 2                     → cell wrapped in  [ ]
#   layer 3                     → cell wrapped in  { }
#   2 or more layers land on the same square → a STACK, cell wrapped in
#     < >, regardless of *which* layers are in it — same idea as NetHack
#     showing a pile as one glyph instead of trying to draw every item:
#     distinguishing the exact combination on the map would need a symbol
#     per combination, illegible past 2 layers anyway. The per-layer
#     squares lists printed in the header are the ": look" for a stacked
#     square — they answer "what's actually in it" precisely if needed.
#   0 layers active → the occupant shown plain, or `.` if empty
#   --highlight <square>, if given → shown as `*X*`, overriding all of the
#     above for that one cell (the square being asked about, not a layer)
#
# More than 3 layers is accepted but not recommended — beyond 3 independent
# boolean layers, superposition on one glyph per square stops being
# legible; prefer separate calls / a count-based view at that point.
const FILES = [a b c d e f g h]

def parse-board-grid [fen: string] {
    let board_part = ($fen | split row " " | get 0)
    let ranks = ($board_part | split row "/")
    if ($ranks | length) != 8 {
        error make {msg: $"malformed FEN board section: ($board_part)"}
    }
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
    $grid
}

# layers: list<record<name: string, squares: list<string>>>
export def render-board-grid [fen: string, layers: list, --highlight: string] {
    let grid = (parse-board-grid $fen)
    let brackets = ["()" "[]" "{}"]

    # Plain-board mode (no layers, no highlight) skips the legend entirely —
    # nothing to key, just the position itself.
    if ($layers | length) > 0 or $highlight != null {
        print "legend:"
        for i in 0..<($layers | length) {
            let layer = ($layers | get $i)
            let b = if $i < 3 { $brackets | get $i } else { "??" }
            let open = ($b | str substring 0..0)
            let close = ($b | str substring 1..1)
            print $"  ($open)X($close)  = ($layer.name)  \(($layer.squares | length) squares\)"
        }
        if ($layers | length) >= 2 {
            print "  <X>  = stack — 2+ of the above layers on this square"
        }
        if $highlight != null {
            print $"  *X*  = ($highlight), the square in question"
        }
        print ""
    }

    for rank_idx in 0..7 {
        let rank_num = 8 - $rank_idx
        mut row_str = $"($rank_num) "
        for file_idx in 0..7 {
            let sq = $"($FILES | get $file_idx)($rank_num)"
            let occupant = ($grid | get $sq)
            let active = ($layers | enumerate | where { |it| $sq in $it.item.squares } | get index)
            let cell = if $highlight != null and $sq == $highlight {
                $"*($occupant)*"
            } else if ($active | length) >= 2 {
                $"<($occupant)>"
            } else if ($active | length) == 1 {
                let i = ($active | get 0)
                let b = if $i < 3 { $brackets | get $i } else { "??" }
                let open = ($b | str substring 0..0)
                let close = ($b | str substring 1..1)
                $"($open)($occupant)($close)"
            } else if $occupant == "." {
                " . "
            } else {
                $" ($occupant) "
            }
            $row_str = $row_str + $cell + " "
        }
        print $row_str
    }
    print "    a   b   c   d   e   f   g   h"
}
