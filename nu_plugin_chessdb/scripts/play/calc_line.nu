#!/usr/bin/env nu
# Usage: nu calc_line.nu "<space-separated uci move history>" "<space-separated uci candidate line>"
#
# Walks a full calculated variation move by move (not just one candidate move
# like check_move.nu) and prints structural facts — hanging pieces, forks,
# king exposure, raw material by piece count, check/mate — at every node. No
# score anywhere. This exists to make real multi-ply calculation reliable:
# write out the line you're actually calculating (my move, their forced or
# expected reply, my follow-up, ...) and verify every position in it is what
# you think it is, in one call, instead of losing track between separate
# single-move checks. Stops and flags the exact ply where an illegal move or
# a miscalculation (something you didn't expect to hang) is found.
def material_line [bal: record] {
    let w = $bal.white
    let b = $bal.black
    $"White: P=($w.pawns) N=($w.knights) B=($w.bishops) R=($w.rooks) Q=($w.queens)   Black: P=($b.pawns) N=($b.knights) B=($b.bishops) R=($b.rooks) Q=($b.queens)"
}

def main [moves: string, line: string] {
    let history = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    let candidate_line = if ($line | str trim | is-empty) { [] } else { $line | split row " " }

    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $history {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    print $"=== starting position \(after history\) ==="
    print $"fen: ($fen)"
    print ""

    mut ply = 0
    for m in $candidate_line {
        $ply = $ply + 1
        let applied = (try { $fen | chessdb apply-uci --uci $m } catch { null })
        if $applied == null {
            print $"=== ply ($ply): ($m) — ILLEGAL from ($fen) — calculation stops here ==="
            return
        }
        $fen = $applied
        let ev = ($fen | chessdb hugm-eval --verbose true)
        let s = $ev.sensor_report
        let mover_just_played = if $ev.side_to_move == "white" { "black" } else { "white" }

        print $"=== ply ($ply): ($m)  \(played by ($mover_just_played)\) ==="
        print $"fen: ($fen)"
        if $s.mate_in_1_exists {
            print $"  !!! MATE IN 1 EXISTS for ($ev.side_to_move) !!!"
        }
        if $s.in_check {
            print $"  ($ev.side_to_move) is in check"
        }
        print $"  material: (material_line $s.material.balance)"

        let hanging = $s.tactical.hanging
        if ($hanging | is-not-empty) {
            print "  HANGING:"
            for h in $hanging { print $"    ($h.piece.color) ($h.piece.role)@($h.piece.square) value=($h.value) safe_to_capture=($h.safe_to_capture)" }
        }
        let forks = $s.tactical.forks
        if ($forks | is-not-empty) {
            print "  FORKS:"
            for f in $forks { print $"    attacker=($f.attacker.color) ($f.attacker.role)@($f.attacker.square) consequence=($f.consequence) see_cp=($f.see_cp)" }
        }
        let ke = $s.positional.king_exposure
        if ($ke | is-not-empty) {
            print "  KING EXPOSURE:"
            for k in $ke { print $"    ($k.color): attacker_count=($k.attacker_count) shelter_files=($k.shelter_files) king_file_open=($k.king_file_open)" }
        }
        print ""
    }
    print $"=== end of calculated line \(($ply) ply\) ==="
}
