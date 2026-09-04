#!/usr/bin/env nu
# Usage: nu board_probe.nu '[e2e4 e7e5 ...]'   (nuon list literal of uci
# moves, parsed with `from nuon`; omit for the start position)
#
# Every geometric/positional fact shakmaty can answer about one position,
# compiled into a single nuon record: all 64 squares' occupant/is_light/
# controls/attacked_by_white/attacked_by_black, plus position-level state
# (side to move, castling, en passant, check/mate/stalemate, checkers,
# legal moves in both san and uci, and raw material counts, never a summed
# value). One comprehensive probe instead of many narrow round trips
# through the other scripts in this directory.
#
# Composed in nu (`shakmaty_compose.nu`'s `board-probe`) from the
# geom-attacks/board-pieces leaf commands, O(pieces) plugin round trips
# rather than O(64 x pieces): computes what each of the board's own pieces
# attacks once, then inverts that into "who attacks this square" per
# square in nu. Deliberately excludes swap-list x-ray plies
# (`square_swap_list.nu`'s job) -- that recursion is expensive and only
# meaningful for a square that's actually contested, not all 64 by default.
#
# 2026-09-03, built per explicit user direction: "use shakmaty to probe the
# board of all information and then compile that into a single nuon." No
# highlighting, no ascii, no filtering -- this is the full, honest source
# of truth; a separate downstream client/renderer applies whatever filter
# it needs on top of this. 2026-09-03 (later same day): rewired onto
# `shakmaty_compose.nu` after `chessdb board-probe` (rust) was removed in
# favor of the shakmaty-1:1 architecture -- A/B-verified byte-identical
# against the rust version (after canonical-sorting both sides' square
# lists) before removal, see FINDINGS.md.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string = "[]"] {
    let fen = (history-to-fen ($moves | from nuon))
    board-probe $fen | to nuon --indent 2
}
