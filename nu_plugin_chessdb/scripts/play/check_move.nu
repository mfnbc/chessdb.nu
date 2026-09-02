#!/usr/bin/env nu
# Usage: nu check_move.nu "<space-separated uci move list>" "<candidate uci move>"
# Applies the candidate move and reports full tactical safety + score.
#
# MY PIECES AT RISK is printed first and separately from everything else,
# on purpose: the Fruit-game postmortem found a real case where the tool
# already had the right signal (an Outnumbered/MoverFavored entry on my own
# rook) sitting a few lines below an exciting "material win for you" fork
# line in this exact kind of dump, and it got missed because attention went
# to the good news first. This script now filters and surfaces the bad news
# before anything else, so it can't be skipped by reading order.
def main [moves: string, candidate: string] {
    let move_list = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $move_list {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    let ok = (try { $fen | chessdb apply-uci --uci $candidate } catch { null })
    if $ok == null {
        print $"ILLEGAL: ($candidate) from ($fen)"
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
    # outnumbered.consequence/see_cp: find_outnumbered was fixed 2026-09-01
    # (FINDINGS.md) to price this directly instead of via the buggy
    # see()/see_chain — the live bug that motivated not filtering by
    # consequence here (a real hanging knight scored "Losing"/"safe") is
    # fixed at the source now. Still deliberately not filtering by
    # consequence: attacker_count > defender_count itself is the ground-truth
    # signal, and showing every such entry costs nothing now that the number
    # next to it is trustworthy too.
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
            print $"  OUTNUMBERED: ($o.piece.role)@($o.piece.square) ($o.attacker_count)v($o.defender_count) see_cp=($o.see_cp) consequence=($o.consequence)"
        }
        for m in $my_mover_favored {
            print $"  MOVER_FAVORED \(opponent, despite count looking safe\): ($m.piece.role)@($m.piece.square) ($m.attacker_count)v($m.defender_count) see_cp=($m.see_cp) consequence=($m.consequence)"
        }
    }
    print ""

    # Deliberately not printing final_score/final_score_white_relative at
    # all (2026-09-02, user feedback, FINDINGS.md): that number is a hand-
    # tuned, never-battle-tested linear formula, and having it sitting in
    # this output made it too easy to default to "highest number wins"
    # instead of actually reasoning about the position — even after
    # explicitly deciding to stop trusting it, the habit of scanning for the
    # best score crept straight back in over the very next game. The
    # per-fact numbers below (see_cp on a specific hanging/outnumbered
    # piece, consequence, attacker/defender counts) are a different thing —
    # each is tied to one concrete, individually-tested exchange, not a
    # summed formula — and stay. See the `position-eval` skill
    # (.claude/skills/position-eval/) for the reasoning method to use
    # instead of a score to actually choose between safe candidates.
    print $"fen: ($ok)"
    print $"hanging=($t.hanging | length) forks=($t.forks | length) pins=($t.pins | length) skewers=($t.skewers | length) discovered=($t.discovered | length) outnumbered=($t.outnumbered | length) mover_favored=($t.mover_favored | length) overloaded=($t.overloaded | length) false_defense=($t.false_defense | length) false_safety=($t.false_safety | length)"
    if ($t.forks | length) > 0 { print "  FORKS:"; for f in $t.forks { print $"    attacker=($f.attacker.color) ($f.attacker.role)@($f.attacker.square) consequence=($f.consequence) see_cp=($f.see_cp)" } }
    if ($t.discovered | length) > 0 { print "  DISCOVERED:"; print $t.discovered }
    print ($ev.explanations | str join "\n")
}
