#!/usr/bin/env nu
# Usage: nu control_overlap.nu '[e2e4 e7e5 ...]'   (nuon list literal of uci
# moves, parsed with `from nuon`; omit for the start position)
#
# Whole-board control map: which squares White controls, which Black
# controls, which are contested (both), and which are controlled by
# neither. Answers "who controls the center," "is this square actually
# contested," and similar whole-position questions that control_map.nu/
# attackers_map.nu can't -- those two are both scoped to one piece or one
# square; this one has no single square of interest, it's the full picture
# at once.
#
# Built on `chessdb attack-summary`, which has returned whole-board
# `attacked_by_white`/`attacked_by_black` since before this convention
# existed.
#
# 2026-09-03: converted to nuon in/out (a move-history nuon string in,
# never a hand-typed FEN; one nuon record out, no print, no ascii grid) per
# the same-date nuon-everything decision -- see `board_overlay.nu`.
use ./board_overlay.nu *

def main [moves: string = "[]"] {
    let fen = (history-to-fen ($moves | from nuon))
    let info = ($fen | chessdb attack-summary)
    let contested = ($info.attacked_by_white | where { |sq| $sq in $info.attacked_by_black })
    {
        white_attack_count: $info.white_attack_count,
        black_attack_count: $info.black_attack_count,
        attacked_by_white: $info.attacked_by_white,
        attacked_by_black: $info.attacked_by_black,
        contested: $contested,
    } | to nuon --indent 2
}
