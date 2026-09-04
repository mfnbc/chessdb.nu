#!/usr/bin/env nu
# Shared FEN-to-board-record helper — the single place every play-tool
# script gets board occupancy from, instead of each hand-parsing a FEN
# board string itself (the exact class of arithmetic that hung a bishop in
# live play, FINDINGS.md 2026-09-02).
#
# 2026-09-03: converted from an ascii-grid renderer (print statements, a
# bracket-legend convention) to a pure nuon record producer, per the
# nuon-everything decision the same date — every play-tool script now
# takes nuon in and returns nuon out, no print, no visual board art. The
# original grid existed to solve the same "never hand-parse a FEN" problem
# a structured record already solves without needing to be visual; see
# `chessdb_board_overlay_convention` (superseded by this file's own
# history) and `PLAN.md`/`FINDINGS.md` for the full migration record.
const FILES = [a b c d e f g h]
const STARTPOS = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

# Every play-tool script's position input, unified: a real nuon list<string>
# of uci moves (never a hand-joined space string, never a hand-typed FEN —
# both are the same "hand-parse a compact board encoding" risk that has
# caused real transcription bugs live, FINDINGS.md 2026-09-02/09-03). Empty
# list is legal and returns the start position.
export def history-to-fen [moves: list<string>] {
    mut fen = $STARTPOS
    for m in $moves {
        $fen = ($fen | chessdb apply-uci --uci $m)
    }
    $fen
}

def role-name [ch: string] {
    match ($ch | str lowercase) {
        "p" => "Pawn", "n" => "Knight", "b" => "Bishop",
        "r" => "Rook", "q" => "Queen", "k" => "King",
        _ => (error make {msg: $"unrecognized piece letter: ($ch)"}),
    }
}

# Every occupied square on a FEN, as square -> {color, role} — sparse (empty
# squares are simply absent, not null-valued), real board color (not
# mover-relative — see `chessdb_mover_not_color`).
export def fen-to-board [fen: string] {
    let board_part = ($fen | split row " " | get 0)
    let ranks = ($board_part | split row "/")
    if ($ranks | length) != 8 {
        error make {msg: $"malformed FEN board section: ($board_part)"}
    }
    mut board = {}
    for rank_idx in 0..7 {
        let rank_num = 8 - $rank_idx
        let row = ($ranks | get $rank_idx)
        mut file_idx = 0
        for ch in ($row | split chars) {
            if ($ch =~ '^[0-9]$') {
                $file_idx = $file_idx + ($ch | into int)
            } else {
                let sq = $"($FILES | get $file_idx)($rank_num)"
                let color = if ($ch =~ '^[A-Z]$') { "white" } else { "black" }
                $board = ($board | insert $sq {color: $color, role: (role-name $ch)})
                $file_idx = $file_idx + 1
            }
        }
    }
    $board
}
