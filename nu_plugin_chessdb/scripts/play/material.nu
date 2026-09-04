#!/usr/bin/env nu
# Usage: nu material.nu '[e2e4 e7e5 ...]'   (a nuon list literal of uci moves,
# parsed with `from nuon` -- real nuon syntax, not a hand-joined string; an OS
# shell can only ever hand a script string argv, so the string itself must be
# valid nuon text rather than the script inventing its own space-joined
# format. Omit the argument, or pass '[]', for the start position.)
#
# Returns raw material by piece count for both sides -- nothing else, no
# print, one nuon record. Deliberately never touches
# sensor_report.material.balance.centipawns (2026-09-02, user feedback):
# even a simple, untuned sum-of-standard-values is still a number sitting
# there inviting "just check if it's decisive" instead of actually
# reasoning about compensation, activity, and whether a position is really
# lost or just materially down. The position-eval skill already says to
# count material by hand from these same raw counts using standard values
# (pawn=1, knight/bishop=3, rook=5, queen=9) -- this script exists so
# checking material never has to go through a field that also happens to
# have the aggregate sitting right next to it.
#
# 2026-09-03: converted to nuon in/out (no print, list<string> input instead
# of a hand-joined move string) per the same-date nuon-everything decision.
use ./board_overlay.nu *

def main [moves: string = "[]"] {
    let fen = (history-to-fen ($moves | from nuon))
    let bal = ($fen | chessdb hugm-eval --verbose true | get sensor_report.material.balance)
    {
        white: {p: $bal.white.pawns, n: $bal.white.knights, b: $bal.white.bishops, r: $bal.white.rooks, q: $bal.white.queens, bishop_pair: $bal.bishop_pair_white},
        black: {p: $bal.black.pawns, n: $bal.black.knights, b: $bal.black.bishops, r: $bal.black.rooks, q: $bal.black.queens, bishop_pair: $bal.bishop_pair_black},
    } | to nuon --indent 2
}
