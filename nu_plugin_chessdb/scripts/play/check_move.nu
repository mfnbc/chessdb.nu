#!/usr/bin/env nu
# Usage: nu check_move.nu "<space-separated uci move list>" "<candidate uci move>"
# Applies the candidate move and reports tactical safety via raw structural
# facts only — never a score, never a per-fact SEE valuation.
#
# MY PIECES AT RISK is printed first and separately from everything else,
# on purpose: the Fruit-game postmortem found a real case where the tool
# already had the right signal (an Outnumbered/MoverFavored entry on my own
# rook) sitting a few lines below an exciting "material win for you" fork
# line in this exact kind of dump, and it got missed because attention went
# to the good news first. This script now filters and surfaces the bad news
# before anything else, so it can't be skipped by reading order.
#
# No `see_cp`/`consequence` anywhere in this output (2026-09-02, user
# feedback): those numbers looked more trustworthy than `final_score`
# because each is tied to one concrete exchange rather than a summed
# formula, but `find_forks` is still backed by the known-buggy `see_chain`,
# and even the direct-subtraction pricing `find_outnumbered`/
# `find_mover_favored` use is still a computed valuation, not a raw fact —
# and a real game (2026-09-02, FINDINGS.md) shows relying on it can lead to
# a move Fruit's own search still rated below its actual best. What stays:
# attacker_count/defender_count (a plain count, not a valuation), piece
# identity and standard value (100/320/330/500/900 — a fixed constant, the
# same numbers the position-eval skill already has you count material with
# by hand, not a search result), and fork/skewer *target* lists (who's
# involved, not whether the exchange is worth it). When a flag fires here,
# that's the signal to actually calculate the resulting exchange yourself —
# `calc_line.nu` for the move sequence, `attackers_map.nu`/`control_map.nu`
# for "is this square/piece really defended" — never to read a verdict off
# this output directly.
#
# No raw FEN printed either (2026-09-02, user feedback): a FEN is exactly
# the same kind of opaque, hand-parsed encoding that caused the arithmetic
# slips this whole tool set exists to avoid — it's plumbing between plugin
# calls, not something to read. The resulting position renders as an actual
# grid instead, via board_overlay.nu's shared convention.
use ./board_overlay.nu *

def main [moves: string, candidate: string] {
    let move_list = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $move_list {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    let ok = (try { $fen | chessdb apply-uci --uci $candidate } catch { null })
    if $ok == null {
        print $"ILLEGAL: ($candidate) — position before the attempt:"
        render-board-grid $fen []
        return
    }
    let ev = ($ok | chessdb hugm-eval --verbose true)
    let t = $ev.sensor_report.tactical
    # After the candidate move, side_to_move is the OPPONENT — my_color is
    # the other one.
    let my_color = if $ev.side_to_move == "white" { "black" } else { "white" }

    # mate_in_1_exists checked explicitly and first: caught the hard way
    # (FINDINGS.md, 2026-09-01, sixth Fruit game) that render_explanations
    # never surfaced this before it was fixed at the source — this session's
    # own hugm-eval --verbose true call ran right up until Qxh2# with
    # mate_in_1_exists already true and nothing about it printed anywhere.
    # sensor_report.mate_in_1_exists is opponent-relative here (whoever is
    # to move after my candidate, i.e. NOT necessarily me) -- it's real
    # regardless of whose mate it is, so always show it.
    if $ev.sensor_report.mate_in_1_exists {
        print $"!!! MATE IN 1 EXISTS for ($ev.side_to_move) in this position !!!"
        print ""
    }

    let my_hanging = ($t.hanging | where { |h| $h.piece.color == $my_color })
    let my_outnumbered = ($t.outnumbered | where { |o| $o.piece.color == $my_color })
    let my_mover_favored = ($t.mover_favored | where { |m| $m.piece.color == $my_color })

    print "=== MY PIECES AT RISK (check this first) ==="
    if ($my_hanging | is-empty) and ($my_outnumbered | is-empty) and ($my_mover_favored | is-empty) {
        print "  (none)"
    } else {
        for h in $my_hanging {
            print $"  HANGING: ($h.piece.role)@($h.piece.square) value=($h.value) safe_to_capture=($h.safe_to_capture)"
        }
        for o in $my_outnumbered {
            print $"  OUTNUMBERED: ($o.piece.role)@($o.piece.square) ($o.attacker_count)v($o.defender_count) -- verify with calc_line.nu, don't trust a count alone"
        }
        for m in $my_mover_favored {
            print $"  MOVER_FAVORED \(count alone said safe, flagged anyway\): ($m.piece.role)@($m.piece.square) ($m.attacker_count)v($m.defender_count) -- verify with calc_line.nu"
        }
    }
    print ""

    let dest_square = ($candidate | str substring 2..3)
    render-board-grid $ok [] --highlight $dest_square
    print ""
    print $"hanging=($t.hanging | length) forks=($t.forks | length) pins=($t.pins | length) skewers=($t.skewers | length) discovered=($t.discovered | length) outnumbered=($t.outnumbered | length) mover_favored=($t.mover_favored | length) overloaded=($t.overloaded | length) false_defense=($t.false_defense | length) false_safety=($t.false_safety | length)"
    if ($t.forks | length) > 0 {
        print "  FORKS (targets only -- check each target's real defense yourself, e.g. via attackers_map.nu):"
        for f in $t.forks {
            let target_list = ($f.targets | each { |x| $"($x.role)@($x.square)" } | str join ", ")
            print $"    attacker=($f.attacker.color) ($f.attacker.role)@($f.attacker.square) -> ($target_list)"
        }
    }
    if ($t.discovered | length) > 0 { print "  DISCOVERED:"; print $t.discovered }
}
