#!/usr/bin/env nu
# Usage: nu attackers_map.nu "<FEN>" "<square>"
#
# Renders the board as an 8x8 grid with every piece that attacks <square>
# marked — the reverse question from control_map.nu's "what does this piece
# see": this one answers "is it safe to put a piece here," directly, for a
# square whether or not it's currently occupied. Built on
# `chessdb square-attackers` (shakmaty's Board::attacks_to, occupancy-aware)
# — this script does no geometric computation of its own, same discipline as
# control_map.nu and for the same reason (FINDINGS.md, 2026-09-02).
#
# Renders through board_overlay.nu's shared convention (2026-09-02): white
# attackers are layer 1 `()`, black attackers layer 2 `[]`, and — unlike
# control_map.nu's three mutually-exclusive layers — a square genuinely CAN
# be attacked by both colors at once, so the `<>` overlap marker is real
# and meaningful here: a contested square, attacked from both sides.
use ./board_overlay.nu *

def main [fen: string, square: string] {
    let info = ($fen | chessdb square-attackers --square $square)
    print $"($square) is attacked by ($info.attacked_by_white | length) white piece\(s\) and ($info.attacked_by_black | length) black piece\(s\):"
    print ""
    render-board-grid $fen [
        {name: "attacked by white", squares: $info.attacked_by_white}
        {name: "attacked by black", squares: $info.attacked_by_black}
    ] --highlight $square
}
