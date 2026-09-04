#!/usr/bin/env nu
# Usage: nu check_move.nu '[e2e4 e7e5 ...]' <candidate uci move>   (nuon list
# literal of uci moves, parsed with `from nuon`)
# Applies the candidate move and reports tactical safety via raw structural
# facts only -- never a score, never a per-fact SEE valuation.
#
# `my_pieces_at_risk` is its own top-level field, always read first, on
# purpose: the Fruit-game postmortem found a real case where the tool
# already had the right signal (an Outnumbered/MoverFavored entry on my own
# rook) sitting a few lines below an exciting "material win for you" fork
# line in this exact kind of dump, and it got missed because attention went
# to the good news first.
#
# No `see_cp`/`consequence` anywhere in this output (2026-09-02, user
# feedback): those numbers looked more trustworthy than `final_score`
# because each is tied to one concrete exchange rather than a summed
# formula, but `find_forks` is still backed by the known-buggy `see_chain`,
# and even the direct-subtraction pricing `find_outnumbered`/
# `find_mover_favored` use is still a computed valuation, not a raw fact --
# and a real game (2026-09-02, FINDINGS.md) shows relying on it can lead to
# a move Fruit's own search still rated below its actual best. What stays:
# attacker_count/defender_count (a plain count, not a valuation), piece
# identity and standard value (100/320/330/500/900 -- a fixed constant, the
# same numbers the position-eval skill already has you count material with
# by hand, not a search result), and fork/skewer *target* lists (who's
# involved, not whether the exchange is worth it). A `defender_count`
# majority is NOT by itself safe if any single attacker is cheaper than the
# piece being defended -- see `chessdb_defender_count_vs_attacker_value`
# (2026-09-03, game 16) -- check real piece values, not just the count.
# When a flag fires here, that's the signal to actually calculate the
# resulting exchange yourself -- `calc_line.nu` for the move sequence,
# `attackers_map.nu`/`square_swap_list.nu` for "is this square/piece really
# defended, and by what" -- never to read a verdict off this output
# directly.
#
# 2026-09-03: converted to nuon in/out (move-history nuon string in, never
# a hand-typed FEN; one nuon record out, no print, no ascii grid) per the
# same-date nuon-everything decision -- see `board_overlay.nu`.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, candidate: string] {
    let fen = (history-to-fen ($moves | from nuon))
    let ok = (try { $fen | chessdb apply-uci --uci $candidate } catch { null })
    if $ok == null {
        return ({legal: false, candidate: $candidate, position_before_attempt: (fen-to-board $fen)} | to nuon --indent 2)
    }

    let ev = ($ok | chessdb hugm-eval --verbose true)
    # strip-scores: no consequence/see_cp anywhere below, matching the
    # header comment's own promise -- a real gap the 2026-09-03 nuon
    # rewrite introduced (returning whole filtered records instead of
    # selectively printing fields let these back in) and this closes.
    let t = (strip-scores $ev.sensor_report.tactical)
    # After the candidate move, side_to_move is the OPPONENT -- my_color is
    # the other one.
    let my_color = if $ev.side_to_move == "white" { "black" } else { "white" }

    # sensor_report.mate_in_1_exists is opponent-relative here (whoever is
    # to move after my candidate, i.e. NOT necessarily me) -- it's real
    # regardless of whose mate it is, so always surfaced, never dropped
    # silently the way render_explanations once did (FINDINGS.md,
    # 2026-09-01, sixth Fruit game).
    let mate_in_1 = {exists: $ev.sensor_report.mate_in_1_exists, side_to_move: $ev.side_to_move}

    let my_hanging = ($t.hanging | where { |h| $h.piece.color == $my_color })
    let my_outnumbered = ($t.outnumbered | where { |o| $o.piece.color == $my_color })
    let my_mover_favored = ($t.mover_favored | where { |m| $m.piece.color == $my_color })

    let dest_square = ($candidate | str substring 2..3)

    {
        legal: true,
        candidate: $candidate,
        mate_in_1: $mate_in_1,
        my_pieces_at_risk: {hanging: $my_hanging, outnumbered: $my_outnumbered, mover_favored: $my_mover_favored},
        destination_square: $dest_square,
        board: (fen-to-board $ok),
        counts: {
            hanging: ($t.hanging | length), forks: ($t.forks | length), pins: ($t.pins | length),
            skewers: ($t.skewers | length), discovered: ($t.discovered | length),
            outnumbered: ($t.outnumbered | length), mover_favored: ($t.mover_favored | length),
            overloaded: ($t.overloaded | length), false_defense: ($t.false_defense | length),
            false_safety: ($t.false_safety | length),
        },
        forks: ($t.forks | each { |f| {attacker: $f.attacker, targets: $f.targets} }),
        discovered: $t.discovered,
    } | to nuon --indent 2
}
