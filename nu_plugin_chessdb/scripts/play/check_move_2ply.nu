#!/usr/bin/env nu
# Usage: nu check_move_2ply.nu '[e2e4 e7e5 ...]' <candidate uci move>   (nuon
# list literal of uci moves, parsed with `from nuon`)
#
# Breadth-first 2-ply visualizer, not a search. After playing the candidate
# move, enumerates EVERY legal reply the opponent has (no ranking, no
# opponent "best move" picked) and re-runs the same "pieces at risk" check
# check_move.nu does after just the candidate move -- one ply deeper, on
# every branch. Surfaces any reply that creates a NEW hanging/outnumbered/
# mover_favored entry on one of my own pieces that a single-position check
# can't see yet.
#
# This exists because of a real miss (session FINDINGS.md, 2026-09-01,
# fifth Fruit game): check_move.nu correctly flagged Qxc2 as walking into a
# capture, but a *second* black knight -- already on the board from an
# earlier, unrelated capture -- immediately forked that same square one ply
# later. Checking only the position right after one candidate move can't
# see that; checking every reply to that candidate move can.
#
# Deliberately NOT a search: no move gets ranked, no opponent "best" reply
# is picked or hidden from the others, nothing is minimaxed. Pure
# enumeration -- "what is reachable," not "what would a strong opponent
# actually play" -- the exact line this project's static evaluator
# deliberately does not cross (PLAN.md's "Pathfinding an exchange instead
# of calculating it").
#
# 2026-09-03: converted to nuon in/out (move-history nuon string in, never
# a hand-typed FEN; one nuon record out, no print, no ascii grid) per the
# same-date nuon-everything decision -- see `board_overlay.nu`.
use ./board_overlay.nu *
use ./shakmaty_compose.nu *

def main [moves: string, candidate: string] {
    let fen = (history-to-fen ($moves | from nuon))
    let after_candidate = (try { $fen | chessdb apply-uci --uci $candidate } catch { null })
    if $after_candidate == null {
        return ({legal: false, candidate: $candidate, position_before_attempt: (fen-to-board $fen)} | to nuon --indent 2)
    }

    # side-to-move token is the FEN's 2nd space-separated field -- cheaper
    # than an extra plugin round trip just to read it back.
    let side_after = ($after_candidate | split row " " | get 1)
    let my_color = if $side_after == "w" { "black" } else { "white" }

    let replies = ($after_candidate | chessdb legal-moves | get mobility_uci)

    let threats_by_reply = ($replies | each { |reply|
        let after_reply = ($after_candidate | chessdb apply-uci --uci $reply)
        let t = (strip-scores ($after_reply | chessdb hugm-eval | get sensor_report.tactical))
        let my_hanging = ($t.hanging | where { |h| $h.piece.color == $my_color })
        let my_outnumbered = ($t.outnumbered | where { |o| $o.piece.color == $my_color })
        let my_mover_favored = ($t.mover_favored | where { |m| $m.piece.color == $my_color })
        {reply: $reply, hanging: $my_hanging, outnumbered: $my_outnumbered, mover_favored: $my_mover_favored}
    } | where { |r| not (($r.hanging | is-empty) and ($r.outnumbered | is-empty) and ($r.mover_favored | is-empty)) })

    {
        legal: true,
        candidate: $candidate,
        reply_count: ($replies | length),
        replies_creating_new_threats: $threats_by_reply,
    } | to nuon --indent 2
}
