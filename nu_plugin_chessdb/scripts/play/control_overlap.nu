#!/usr/bin/env nu
# Usage: nu control_overlap.nu "<FEN>"
#
# Whole-board control map: every square marked by whether White controls
# it, Black controls it, both (contested), or neither. Answers "who
# controls the center," "is this square actually contested," and similar
# whole-position questions that control_map.nu/attackers_map.nu can't —
# those two are both scoped to one piece or one square; this one has no
# single square of interest, it's the full picture at once.
#
# Built on `chessdb attack-summary`, which has returned whole-board
# `attacked_by_white`/`attacked_by_black` since before this convention
# existed — this view was just never rendered until board_overlay.nu made
# a 2-layer overlap grid a one-call thing instead of a bespoke script.
# Renders through board_overlay.nu's shared convention: white control is
# layer 1 `()`, black control layer 2 `[]`, contested squares `<>`.
use ./board_overlay.nu *

def main [fen: string] {
    let info = ($fen | chessdb attack-summary)
    let contested = ($info.attacked_by_white | where { |sq| $sq in $info.attacked_by_black })
    print $"White controls ($info.white_attack_count) squares, Black controls ($info.black_attack_count), ($contested | length) contested \(both\)."
    print ""
    render-board-grid $fen [
        {name: "controlled by white", squares: $info.attacked_by_white}
        {name: "controlled by black", squares: $info.attacked_by_black}
    ]
}
