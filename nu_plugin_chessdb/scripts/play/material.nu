#!/usr/bin/env nu
# Usage: nu material.nu "<space-separated uci move history>"
#
# Prints raw material by piece count for both sides -- nothing else.
# Deliberately never touches sensor_report.material.balance.centipawns
# (2026-09-02, user feedback): even a simple, untuned sum-of-standard-values
# is still a number sitting there inviting "just check if it's decisive"
# instead of actually reasoning about compensation, activity, and whether a
# position is really lost or just materially down. The position-eval skill
# already says to count material by hand from these same raw counts using
# standard values (pawn=1, knight/bishop=3, rook=5, queen=9) -- this script
# exists so checking material never has to go through a field that also
# happens to have the aggregate sitting right next to it.
#
# No raw FEN printed either (2026-09-02, user feedback) — this tool's output
# is already the "list of pieces that can be hand-calculated" format, no
# separate visualization needed for a pure material check; use control_map.nu
# /attackers_map.nu/control_overlap.nu when the actual board layout matters.
def main [moves: string] {
    let move_list = if ($moves | str trim | is-empty) { [] } else { $moves | split row " " }
    mut fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    for m in $move_list {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    let bal = ($fen | chessdb hugm-eval --verbose true | get sensor_report.material.balance)
    print $"White: P=($bal.white.pawns) N=($bal.white.knights) B=($bal.white.bishops) R=($bal.white.rooks) Q=($bal.white.queens)  \(bishop pair: ($bal.bishop_pair_white)\)"
    print $"Black: P=($bal.black.pawns) N=($bal.black.knights) B=($bal.black.bishops) R=($bal.black.rooks) Q=($bal.black.queens)  \(bishop pair: ($bal.bishop_pair_black)\)"
}
