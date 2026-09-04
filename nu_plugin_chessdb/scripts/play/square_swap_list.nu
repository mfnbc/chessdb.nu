#!/usr/bin/env nu
# Usage: nu square_swap_list.nu '[e2e4 e7e5 ...]' <square>   (nuon list
# literal of uci moves, parsed with `from nuon`; omit the move list for the
# start position)
#
# The raw occupancy-aware exchange picture for one square: every piece
# (either side) that can reach it right now, plus every piece only
# revealed once a nearer piece is removed from the board (an x-ray),
# recursively, until no further reveals occur -- ply by ply, real piece
# identities and standard values, notated mover-uppercase /
# opponent-lowercase per the position's side to move (e.g.
# "0ply q 1ply NbPQ 2ply B": the occupant is the opponent's queen, ply 1
# has the mover's knight+pawn+queen and the opponent's bishop defending,
# ply 2 reveals the mover's second bishop as an x-ray).
#
# Composed in nu (`shakmaty_compose.nu`'s `swap-list`) from the
# geom-attacks/board-pieces leaf commands: recursively removes each ply's
# attacker squares from the occupancy (an ordinary nu `where` filter, since
# occupancy is just a plain square list) and re-queries attacks-to until no
# new attacker appears. Answers the question game 16's decisive mistake
# needed and didn't have
# (FINDINGS.md/chessdb_defender_count_vs_attacker_value, 2026-09-03): a
# defender-count majority is not safety if any single attacker is cheaper
# than the piece being defended -- this command puts every attacker's real
# value directly in front of you, ply by ply, instead of a bare count.
# Deliberately never computes a "safe/hanging" verdict itself -- that
# judgment stays with whoever reads the piece list, same discipline as
# every other tool in this directory (see `board_overlay.nu`).
#
# 2026-09-03: built nuon-native as a thin wrapper over the (now-removed)
# `chessdb square-swap-list` rust command. 2026-09-03 (later same day):
# rewired onto `shakmaty_compose.nu`'s nu-composed `swap-list` after that
# rust command was removed in favor of the shakmaty-1:1 architecture --
# A/B-verified byte-identical against the rust version before removal, see
# FINDINGS.md.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, square: string] {
    let fen = (history-to-fen ($moves | from nuon))
    swap-list $fen $square | to nuon --indent 2
}
