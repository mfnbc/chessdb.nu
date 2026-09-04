#!/usr/bin/env nu
# Usage: nu full_report.nu '[e2e4 e7e5 ...]'   (nuon list literal of uci
# moves, parsed with `from nuon`; omit for the start position)
#
# The single comprehensive position report: `board_probe.nu`'s
# shakmaty-derived geometric/structural facts (all 64 squares, position
# state, raw material) merged with the existing tactical/positional
# detector layer (hanging/forks/pins/skewers/discovered/outnumbered/
# mover_favored/overloaded/false_defense/false_safety, and outposts/
# open_files/passed_pawns/doubled_pawns/isolated_pawns/pawn_islands/
# pawn_breaks/pawn_majority/rook_on_seventh/king_exposure), with every
# computed-valuation field stripped (no final_score, no aggregated.*_cp,
# no engine_score, no per-fact consequence/see_cp/centipawns).
#
# 2026-09-03: this is THE report -- one call gives everything a position
# evaluation needs. `.claude/skills/position-eval/SKILL.md` teaches the
# priority order to actually read it in (most important fact down to
# least), not which of several narrow tools to call for which fact.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string = "[]"] {
    let fen = (history-to-fen ($moves | from nuon))
    full-report $fen | to nuon --indent 2
}
