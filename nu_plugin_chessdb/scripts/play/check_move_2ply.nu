#!/usr/bin/env nu
# Usage: nu check_move_2ply.nu "<space-separated uci move list>" "<candidate uci move>"
#
# Breadth-first 2-ply visualizer, not a search. After playing the candidate
# move, enumerates EVERY legal reply the opponent has (no ranking, no
# opponent "best move" picked) and re-runs the same "MY PIECES AT RISK"
# check check_move.nu does after just the candidate move — one ply deeper,
# on every branch. Surfaces any reply that creates a NEW hanging/
# outnumbered/mover_favored entry on one of my own pieces that a
# single-position check can't see yet.
#
# This exists because of a real miss (session FINDINGS.md, 2026-09-01,
# fifth Fruit game): check_move.nu correctly flagged Qxc2 as walking into a
# capture, but a *second* black knight — already on the board from an
# earlier, unrelated capture — immediately forked that same square one ply
# later. Checking only the position right after one candidate move can't
# see that; checking every reply to that candidate move can.
#
# Deliberately NOT a search: no move gets ranked, no opponent "best" reply
# is picked or hidden from the others, nothing is minimaxed. This is pure
# enumeration — "what is reachable," not "what would a strong opponent
# actually play" — the exact line this project's static evaluator
# deliberately does not cross (PLAN.md's "Pathfinding an exchange instead
# of calculating it").
def main [moves: string, candidate: string] {
    let move_list = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $move_list {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }

    let after_candidate = (try { $fen | chessdb apply-uci --uci $candidate } catch { null })
    if $after_candidate == null {
        print $"ILLEGAL: ($candidate) from ($fen)"
        return
    }

    # side-to-move token is the FEN's 2nd space-separated field -- cheaper
    # than an extra plugin round trip just to read it back.
    let side_after = ($after_candidate | split row " " | get 1)
    let my_color = if $side_after == "w" { "black" } else { "white" }

    let replies = ($after_candidate | chessdb legal-moves | get mobility_uci)
    print $"($candidate) played \(($replies | length) opponent replies to check\):"
    print ""

    mut found_any = false
    for reply in $replies {
        let after_reply = ($after_candidate | chessdb apply-uci --uci $reply)
        let t = ($after_reply | chessdb hugm-eval | get sensor_report.tactical)
        let my_hanging = ($t.hanging | where { |h| $h.piece.color == $my_color })
        let my_outnumbered = ($t.outnumbered | where { |o| $o.piece.color == $my_color })
        let my_mover_favored = ($t.mover_favored | where { |m| $m.piece.color == $my_color })
        if not (($my_hanging | is-empty) and ($my_outnumbered | is-empty) and ($my_mover_favored | is-empty)) {
            $found_any = true
            print $"  ...($reply):"
            for h in $my_hanging {
                print $"    HANGING: ($h.piece.role)@($h.piece.square) value=($h.value) safe_to_capture=($h.safe_to_capture)"
            }
            for o in $my_outnumbered {
                print $"    OUTNUMBERED: ($o.piece.role)@($o.piece.square) ($o.attacker_count)v($o.defender_count) see_cp=($o.see_cp) consequence=($o.consequence)"
            }
            for m in $my_mover_favored {
                print $"    MOVER_FAVORED: ($m.piece.role)@($m.piece.square) ($m.attacker_count)v($m.defender_count) see_cp=($m.see_cp) consequence=($m.consequence)"
            }
        }
    }
    if not $found_any {
        print "  (none) -- no opponent reply creates a new threat to any of my pieces"
    }
}
