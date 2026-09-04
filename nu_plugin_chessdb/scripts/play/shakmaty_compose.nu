#!/usr/bin/env nu
# Nushell-side composition over the leaf-layer plugin commands (geom-attacks,
# geom-ray, geom-between, geom-aligned, board-pieces, board-piece-at,
# square-is-light) — built 2026-09-03 per explicit user direction: chessdb
# stays a close-to-1:1 translation of shakmaty's own functions; everything
# above that (attacks_to-equivalent, whole-board probes, x-ray swap lists)
# is nushell composing those leaves, not rust composing shakmaty internally.
#
# Each function here is verified byte-for-byte against the OLDER,
# already-tested rust-composed command it replaces (square_attackers,
# square_control, square_swap_list, board_probe) before those get removed —
# matching this crate's own detect_skewers A/B-diff precedent.
use ./board_overlay.nu *

# Every piece of `color` that attacks `square`, given an explicit `occupied`
# square list — the nu-composed equivalent of `Board::attacks_to`. For each
# of `color`'s pieces, ask the leaf `geom-attacks` whether it reaches
# `square` under this occupancy, rather than a rust-side loop.
export def attacks-to [fen: string, square: string, color: string, occupied: list<string>] {
    let pieces = ($fen | chessdb board-pieces --color $color).squares
    $pieces | where { |origin|
        let piece = ($fen | chessdb board-piece-at --square $origin)
        let atk = (chessdb geom-attacks --square $origin --color $color --role $piece.role --occupied $occupied)
        $square in $atk.squares
    } | sort
}

# Every square the piece on `square` attacks/defends — the nu-composed
# equivalent of `Board::attacks_from`. Null if the square is empty.
export def attacks-from [fen: string, square: string, occupied: list<string>] {
    let piece = ($fen | chessdb board-piece-at --square $square)
    if $piece == null {
        return null
    }
    (chessdb geom-attacks --square $square --color $piece.color --role $piece.role --occupied $occupied).squares | sort
}

# The recursive x-ray swap list for one square — the nu-composed equivalent
# of `chessdb square-swap-list`'s rust-side recursion. Same ply semantics:
# ply 1 = attackers with the real board's full occupancy, ply N+1 = pieces
# newly revealed once every square collected so far is removed from
# occupancy. Bitboard removal is just a nu `where` filter once occupancy is
# a plain square list — no rust loop needed.
export def swap-list [fen: string, square: string] {
    let occupied = ($fen | chessdb board-pieces).squares
    let mover = ($fen | chessdb fen-info | get turn)
    let occupant = ($fen | chessdb board-piece-at --square $square)

    let notate = { |color, role|
        let letter = ($role | str substring 0..0 | str uppercase)
        let letter = (if $role == "Knight" { "N" } else { $letter })
        if $color == $mover { $letter } else { ($letter | str lowercase) }
    }

    mut seen = []
    mut plies = []
    mut current_occupied = $occupied
    loop {
        let white = (attacks-to $fen $square "white" $current_occupied)
        let black = (attacks-to $fen $square "black" $current_occupied)
        let attackers = ($white | append $black | uniq)
        let new_attackers = ($attackers | where { |sq| $sq not-in $seen })
        if ($new_attackers | is-empty) {
            break
        }
        let entries = ($new_attackers | each { |sq|
            let piece = ($fen | chessdb board-piece-at --square $sq)
            let value = (match $piece.role {
                "Pawn" => 100, "Knight" => 320, "Bishop" => 330,
                "Rook" => 500, "Queen" => 900, "King" => 20000,
            })
            {square: $sq, role: $piece.role, color: $piece.color, owner: (if $piece.color == $mover {"mover"} else {"opponent"}), value: $value, notation: (do $notate $piece.color $piece.role)}
        } | sort-by value)
        $seen = ($seen | append $new_attackers)
        $current_occupied = ($current_occupied | where { |sq| $sq not-in $new_attackers })
        $plies = ($plies | append [$entries])
    }

    let occupant_notation = if $occupant == null { null } else { do $notate $occupant.color $occupant.role }
    mut notation_parts = []
    if $occupant_notation != null {
        $notation_parts = ($notation_parts | append $"0ply ($occupant_notation)")
    }
    for i in 0..<($plies | length) {
        let letters = ($plies | get $i | each { |e| $e.notation } | str join "")
        $notation_parts = ($notation_parts | append $"($i + 1)ply ($letters)")
    }

    {
        square: $square,
        mover: $mover,
        occupant: $occupant,
        occupant_notation: $occupant_notation,
        plies: $plies,
        notation: ($notation_parts | str join " "),
    }
}

# Recursively strips any record key whose name matches a computed-valuation
# pattern (score/_cp/consequence, case-insensitive) from a value -- records
# and lists are walked, everything else passes through unchanged. Simple,
# name-based, not a hand-audited field-by-field allowlist: per explicit
# user direction, filtering precision is the lowest of three priorities
# here (shakmaty 1:1 first, compositing the report second, filtering
# third) -- this is deliberately a blunt instrument, not exhaustive. See
# `feedback_dont_surface_untested_scores` for why these fields are excluded
# at all (final_score/aggregated.*_cp/engine_score/consequence/see_cp are
# untested computed valuations, never raw facts).
export def strip-scores [value: any] {
    let t = ($value | describe)
    if ($t | str starts-with "record") {
        let keep = ($value | columns | where { |c| not ($c =~ '(?i)score|_cp$|centipawn|consequence') })
        $keep | reduce -f {} { |c, acc| $acc | insert $c (strip-scores ($value | get $c)) }
    } else if ($t | str starts-with "list") or ($t | str starts-with "table") {
        $value | each { |item| strip-scores $item }
    } else {
        $value
    }
}

const ALL_SQUARES = [
    a1 b1 c1 d1 e1 f1 g1 h1 a2 b2 c2 d2 e2 f2 g2 h2 a3 b3 c3 d3 e3 f3 g3 h3
    a4 b4 c4 d4 e4 f4 g4 h4 a5 b5 c5 d5 e5 f5 g5 h5 a6 b6 c6 d6 e6 f6 g6 h6
    a7 b7 c7 d7 e7 f7 g7 h7 a8 b8 c8 d8 e8 f8 g8 h8
]
const COLORS = [white black]
const ROLES = [pawn knight bishop rook queen king]

# Every geometric/positional fact for the whole position, compiled from the
# leaf commands -- the nu-composed equivalent of `chessdb board-probe`.
# O(pieces) plugin round trips, not O(64 x pieces): computes what each of
# the board's own pieces attacks once (up to 32 geom-attacks calls, grouped
# via 10 board-pieces calls), then inverts that into "who attacks this
# square" per square in nu, rather than asking attacks-to fresh at each of
# the 64 squares.
export def board-probe [fen: string] {
    let occ = ($fen | chessdb board-pieces).squares

    mut piece_attacks = []
    mut material = {white: {}, black: {}}
    for color in $COLORS {
        for role in $ROLES {
            let group = ($fen | chessdb board-pieces --color $color --role $role).squares
            $material = ($material | upsert $color { |m| $m | get $color | insert $role ($group | length) })
            for sq in $group {
                let atk = (chessdb geom-attacks --square $sq --color $color --role $role --occupied $occ)
                $piece_attacks = ($piece_attacks | append {origin: $sq, color: $color, targets: $atk.squares})
            }
        }
    }
    let material_count = { |m| {pawns: $m.pawn, knights: $m.knight, bishops: $m.bishop, rooks: $m.rook, queens: $m.queen, bishop_pair: ($m.bishop >= 2)} }
    let piece_attacks = $piece_attacks

    let squares = ($ALL_SQUARES | each { |sq|
        let occupant = ($fen | chessdb board-piece-at --square $sq)
        let controls = if $occupant == null { [] } else {
            ($piece_attacks | where origin == $sq | get 0 | get targets)
        }
        let attacked_by_white = ($piece_attacks | where { |pa| $pa.color == "white" and $sq in $pa.targets } | get origin | sort)
        let attacked_by_black = ($piece_attacks | where { |pa| $pa.color == "black" and $sq in $pa.targets } | get origin | sort)
        {square: $sq, occupant: $occupant, is_light: (chessdb square-is-light --square $sq), controls: ($controls | sort), attacked_by_white: $attacked_by_white, attacked_by_black: $attacked_by_black}
    })

    let info = ($fen | chessdb fen-info)
    let checkers = ($fen | chessdb checker-summary)
    let mobility = ($fen | chessdb legal-moves)

    {
        side_to_move: $info.turn,
        castling: $info.castling,
        en_passant_square: (if $info.ep_square == "-" { null } else { $info.ep_square }),
        halfmove_clock: $info.halfmoves,
        fullmove_number: $info.fullmoves,
        in_check: $info.is_check,
        is_checkmate: $info.is_checkmate,
        is_stalemate: $info.is_stalemate,
        is_insufficient_material: $info.is_insufficient_material,
        checkers: $checkers.checker_squares,
        legal_move_count: $mobility.legal_move_count,
        legal_moves_san: $mobility.mobility_san,
        legal_moves_uci: $mobility.mobility_uci,
        material_white: (do $material_count $material.white),
        material_black: (do $material_count $material.black),
        squares: ($squares | reduce -f {} { |it, acc| $acc | insert $it.square {occupant: $it.occupant, is_light: $it.is_light, controls: $it.controls, attacked_by_white: $it.attacked_by_white, attacked_by_black: $it.attacked_by_black} }),
    }
}

# The single comprehensive report: `board-probe`'s shakmaty-derived
# geometric/structural facts (all 64 squares, position state, raw
# material) merged with the existing tactical/positional detector layer
# (`chessdb hugm-eval`'s sensor_report -- hanging/forks/pins/skewers/
# discovered/outnumbered/mover_favored/overloaded/false_defense/
# false_safety, and outposts/open_files/passed_pawns/doubled_pawns/
# isolated_pawns/pawn_islands/pawn_breaks/pawn_majority/rook_on_seventh/
# king_exposure), with every computed-valuation field stripped
# (`strip-scores` -- no final_score, no aggregated.*_cp, no engine_score,
# no per-fact consequence/see_cp). One nuon record with everything a full
# position evaluation needs; `.claude/skills/position-eval/SKILL.md`
# teaches the priority order to actually read it in, not which tool to
# call for which fact.
export def full-report [fen: string] {
    let probe = (board-probe $fen)
    let ev = ($fen | chessdb hugm-eval --verbose true)
    let s = (strip-scores $ev.sensor_report)

    $probe | merge {
        mate_in_1_exists: $s.mate_in_1_exists,
        king_tropism_us: $s.king_tropism_us,
        initiative_us: $s.initiative_us,
        doubled_rooks_us: $s.doubled_rooks_us,
        tactical: $s.tactical,
        positional: $s.positional,
    }
}
