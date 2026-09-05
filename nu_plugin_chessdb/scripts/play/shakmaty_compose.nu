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
    mut ply_index = 0
    loop {
        let white = (attacks-to $fen $square "white" $current_occupied)
        let black = (attacks-to $fen $square "black" $current_occupied)
        let attackers = ($white | append $black | uniq)
        let new_attackers = ($attackers | where { |sq| $sq not-in $seen })
        if ($new_attackers | is-empty) {
            break
        }
        # 2026-09-04: pin-legality fix, ply 1 AND mover's-own-color only.
        # `attacks-to` is pure geometry -- it doesn't know a piece is
        # pinned and therefore can't actually make this capture right now
        # (a real, demonstrated bug: a knight absolutely pinned to its own
        # king, with zero legal moves at all, was still listed here as a
        # real defender).
        #
        # Two narrowings, both load-bearing, both found live:
        # 1. Only ply 1 is checked against the real, unmodified `$fen` --
        #    deeper plies are already hypothetical (their attackers only
        #    "exist" once earlier squares are assumed vacated), so checking
        #    them against today's real position would wrongly reject
        #    genuine x-ray attackers still physically blocked right now.
        # 2. Only attackers whose color matches `$mover` (the position's
        #    actual side-to-move) are filtered. `chessdb is-legal` checks
        #    "is this legal for whoever's turn it actually is" -- for the
        #    opponent's ply-1 attackers, that's always a legal-for-a-side-
        #    not-currently-to-move question, and `is-legal` correctly (but
        #    unhelpfully) says no regardless of whether they're pinned.
        #    Confirmed live: without this narrowing, a real historical
        #    position (game 16, e5) lost 3 of its 4 real attackers --
        #    Black's queen/knight/rook all vanished from the notation
        #    purely because it was White's move in that FEN, nothing to do
        #    with any of them actually being pinned. Correctly resolving
        #    the opponent's hypothetical-turn legality would need a
        #    turn-flipped position (real risk of "impossible check"
        #    rejection at the boundary) -- out of scope for this pass;
        #    `swap-list`'s primary use (verifying a piece of *my own* is
        #    free to move before committing to a trade) is exactly the
        #    mover's-own-color case this narrowing still covers.
        # `seen`/`current_occupied` below still use the full geometric
        # `new_attackers` regardless of this filter, so excluding a pinned
        # piece from what's reported never changes which squares later
        # plies reveal -- it only removes the pinned piece from the
        # notation, exactly as it should.
        let visible_attackers = if $ply_index == 0 {
            ($new_attackers | where { |sq|
                let piece = ($fen | chessdb board-piece-at --square $sq)
                if $piece.color != $mover {
                    true
                } else {
                    ($fen | chessdb is-legal --move $"($sq)($square)")
                }
            })
        } else {
            $new_attackers
        }
        let entries = ($visible_attackers | each { |sq|
            let piece = ($fen | chessdb board-piece-at --square $sq)
            let value = (match $piece.role {
                "Pawn" => 100, "Knight" => 320, "Bishop" => 330,
                "Rook" => 500, "Queen" => 900, "King" => 20000,
            })
            {square: $sq, role: $piece.role, color: $piece.color, owner: (if $piece.color == $mover {"mover"} else {"opponent"}), value: $value, notation: (do $notate $piece.color $piece.role)}
        } | sort-by value)
        $seen = ($seen | append $new_attackers)
        $current_occupied = ($current_occupied | where { |sq| $sq not-in $new_attackers })
        # Always append, even when `entries` is empty (every ply-1 attacker
        # was pinned away) -- the notation loop below numbers plies by
        # array position, so dropping an empty entry here would silently
        # renumber every ply after it.
        $plies = ($plies | append [$entries])
        $ply_index = $ply_index + 1
    }

    let occupant_notation = if $occupant == null { null } else { do $notate $occupant.color $occupant.role }
    mut notation_parts = []
    if $occupant_notation != null {
        $notation_parts = ($notation_parts | append $"0ply ($occupant_notation)")
    }
    for i in 0..<($plies | length) {
        let letters = ($plies | get $i | each { |e| $e.notation } | str join "")
        # Skip an empty ply's *text* (e.g. every ply-1 attacker was pinned
        # away) but keep using `i + 1` for whatever ply comes next -- the
        # label must stay tied to array position, not to how many
        # non-empty segments have been printed so far.
        if ($letters | is-not-empty) {
            $notation_parts = ($notation_parts | append $"($i + 1)ply ($letters)")
        }
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
        detectors: (extra-detectors $fen),
    }
}

# ===========================================================================
# Extra detectors (2026-09-03) -- a gap audit against dev-arcturus/
# positional_chess's ~70-motif catalog (a comparable browser/wasm chess
# analysis tool with a similar raw-engine-plus-structured-fact-layer
# architecture). Every detector below is a raw, verifiable structural
# fact, composed entirely from the leaf commands -- no new rust, no
# valuation, nothing requiring a judgment call (their `sacrifice`,
# `tempo`, `prophylaxis`, `bad_bishop`, etc. were deliberately left out or
# reframed as a raw count instead of a label -- see FINDINGS.md). Each was
# checked against a real, independently-reasoned position before being
# wired into `full-report`.
# ===========================================================================

const FILES = [a b c d e f g h]
def square-file [sq: string] { $sq | str substring 0..0 }
def square-rank [sq: string] { $sq | str substring 1..1 }
def file-index [f: string] { $FILES | enumerate | where item == $f | get 0.index }
const LIGHT_LONG_DIAGONAL = [a1 b2 c3 d4 e5 f6 g7 h8]
const DARK_LONG_DIAGONAL = [a8 b7 c6 d5 e4 f3 g2 h1]

# Two same-color sliding pieces (rook/queen on a shared rank or file;
# bishop/queen on a shared diagonal) with nothing at all between them --
# `geom-between` empty and both pieces' role compatible with that axis.
export def battery [fen: string] {
    let occ = ($fen | chessdb board-pieces).squares
    let sliders = ($COLORS | each { |color|
        (["bishop" "rook" "queen"] | each { |role|
            ($fen | chessdb board-pieces --color $color --role $role).squares
            | each { |sq| {color: $color, role: $role, square: $sq} }
        })
    } | flatten | flatten)

    let pairs = ($sliders | enumerate | each { |a|
        $sliders | enumerate | where { |b| $b.index > $a.index and $b.item.color == $a.item.color } | each { |b|
            {a: $a.item, b: $b.item}
        }
    } | flatten)

    $pairs | each { |p|
        let same_rank = ((square-rank $p.a.square) == (square-rank $p.b.square))
        let same_file = ((square-file $p.a.square) == (square-file $p.b.square))
        let file_diff = ((file-index (square-file $p.a.square)) - (file-index (square-file $p.b.square)) | math abs)
        let rank_diff = (((square-rank $p.a.square) | into int) - ((square-rank $p.b.square) | into int) | math abs)
        let same_diagonal = (not $same_rank) and (not $same_file) and ($file_diff == $rank_diff)

        let axis = if $same_rank { "rank" } else if $same_file { "file" } else if $same_diagonal { "diagonal" } else { null }
        let role_ok = if $axis == "rank" or $axis == "file" {
            ($p.a.role in ["rook" "queen"]) and ($p.b.role in ["rook" "queen"])
        } else if $axis == "diagonal" {
            ($p.a.role in ["bishop" "queen"]) and ($p.b.role in ["bishop" "queen"])
        } else { false }

        if $axis != null and $role_ok {
            let between = (chessdb geom-between --a $p.a.square --b $p.b.square).squares
            let clear = ($between | where { |sq| $sq in $occ } | is-empty)
            if $clear { {color: $p.a.color, axis: $axis, a: $p.a, b: $p.b} } else { null }
        } else { null }
    } | where { |x| $x != null }
}

# A piece with at least one legal move, none of whose legal destinations
# are ever attacked by the opponent -- the raw fact behind game 15's
# Qxa7 queen trap, formalized instead of a one-off manual check. Reports
# every destination's real attacker list (never a computed "safe"
# verdict) -- read it the same way `square_swap_list.nu` is read.
export def piece-mobility-safety [fen: string, square: string] {
    let piece = ($fen | chessdb board-piece-at --square $square)
    if $piece == null {
        return {square: $square, piece: null, destinations: []}
    }
    let lm = ($fen | chessdb legal-moves)
    let my_moves = ($lm.mobility_uci | where { |m| ($m | str substring 0..1) == $square })
    let opponent = if $piece.color == "white" { "black" } else { "white" }

    let destinations = ($my_moves | each { |uci|
        let dest = ($uci | str substring 2..3)
        let after = ($fen | chessdb apply-uci --uci $uci)
        let occ = ($after | chessdb board-pieces).squares
        let attackers = (attacks-to $after $dest $opponent $occ)
        {uci: $uci, square: $dest, attacked_by_opponent: $attackers}
    })

    {square: $square, piece: $piece, destinations: $destinations}
}

# King's own legal destinations right now (side to move only -- legal-move
# generation is only well-defined for the side to move). An empty list
# with the king still on the board and not checkmated means every
# adjacent square is covered; a non-empty list names the real escape
# squares, never a "safe" judgment.
export def king-mobility [fen: string] {
    let lm = ($fen | chessdb legal-moves)
    let king_sq = ($fen | chessdb board-pieces --color $lm.side_to_move --role king).squares | get 0
    {square: $king_sq, side_to_move: $lm.side_to_move, destinations: ($lm.mobility_uci | where { |m| ($m | str substring 0..1) == $king_sq } | each { |m| $m | str substring 2..3 })}
}

# Knights on the rim (file a/h or rank 1/8) -- "a knight on the rim is
# dim," reduced mobility is a pure geometric fact of the square itself.
export def knights-on-rim [fen: string] {
    # "a knight on the rim is dim" -- the edge FILES specifically (a/h),
    # not rank 1/8: every knight starts on rank 1/8, so including rank
    # would flag ordinary starting/developing squares (b1/g1/b8/g8) as
    # rim, which they aren't. Caught by testing against the start
    # position before wiring this in -- see FINDINGS.md.
    $COLORS | each { |color|
        ($fen | chessdb board-pieces --color $color --role knight).squares
        | where { |sq| (square-file $sq) in ["a" "h"] }
        | each { |sq| {color: $color, square: $sq} }
    } | flatten
}

# Bishops on a fianchetto square (b2/g2 for white, b7/g7 for black).
export def fianchettoed-bishops [fen: string] {
    let squares_by_color = {white: [b2 g2], black: [b7 g7]}
    $COLORS | each { |color|
        ($fen | chessdb board-pieces --color $color --role bishop).squares
        | where { |sq| $sq in ($squares_by_color | get $color) }
        | each { |sq| {color: $color, square: $sq} }
    } | flatten
}

# Bishops/queens on either long diagonal (a1-h8 or a8-h1).
export def long-diagonal-pieces [fen: string] {
    $COLORS | each { |color|
        (["bishop" "queen"] | each { |role|
            ($fen | chessdb board-pieces --color $color --role $role).squares
            | where { |sq| $sq in $LIGHT_LONG_DIAGONAL or $sq in $DARK_LONG_DIAGONAL }
            | each { |sq| {color: $color, role: $role, square: $sq, diagonal: (if $sq in $LIGHT_LONG_DIAGONAL { "a1-h8" } else { "a8-h1" })} }
        })
    } | flatten | flatten
}

# Files with pawns from exactly one color -- semi-open FOR the color with
# none there (distinct from `positional.open_files`, which is both colors
# absent).
export def semi-open-files [fen: string] {
    let files = [a b c d e f g h]
    let white_pawns = ($fen | chessdb board-pieces --color white --role pawn).squares
    let black_pawns = ($fen | chessdb board-pieces --color black --role pawn).squares
    $files | each { |f|
        let has_white = ($white_pawns | any { |sq| (square-file $sq) == $f })
        let has_black = ($black_pawns | any { |sq| (square-file $sq) == $f })
        if $has_white and not $has_black { {file: $f, semi_open_for: "black"} } else if $has_black and not $has_white { {file: $f, semi_open_for: "white"} } else { null }
    } | where { |x| $x != null }
}

# A pawn defended by another own pawn (attacked_by its own color, role
# pawn specifically -- narrower than the general `attacked_by_*` on
# `board-probe`'s squares, which doesn't filter by attacker role).
export def supported-pawns [fen: string] {
    let occ = ($fen | chessdb board-pieces).squares
    $COLORS | each { |color|
        let my_pawns = ($fen | chessdb board-pieces --color $color --role pawn).squares
        $my_pawns | where { |sq|
            $my_pawns | any { |other|
                $other != $sq and ($sq in (chessdb geom-attacks --square $other --color $color --role pawn --occupied $occ).squares)
            }
        } | each { |sq| {color: $color, square: $sq} }
    } | flatten
}

# A pawn that (a) has no own pawn on an adjacent file able to defend it
# by advancing, and (b) would be attacked by an enemy pawn if it advanced
# -- can neither be supported from behind nor safely push. Distinct from
# `positional.isolated_pawns` (no pawn on an adjacent file at all, a
# stricter condition); a backward pawn can have adjacent-file pawns, just
# none of them behind or level.
export def backward-pawns [fen: string] {
    let occ = ($fen | chessdb board-pieces).squares
    $COLORS | each { |color|
        let dir = if $color == "white" { 1 } else { -1 }
        let opponent = if $color == "white" { "black" } else { "white" }
        let my_pawns = ($fen | chessdb board-pieces --color $color --role pawn).squares

        $my_pawns | where { |sq|
            let f = (square-file $sq)
            let r = (square-rank $sq | into int)
            let f_idx = (file-index $f)
            let adjacent_files = ([($f_idx - 1) ($f_idx + 1)] | where { |i| $i >= 0 and $i < 8 } | each { |i| $FILES | get $i })

            let can_be_defended = ($my_pawns | any { |other|
                let of = (square-file $other)
                let or_ = (square-rank $other | into int)
                ($of in $adjacent_files) and (if $color == "white" { $or_ <= $r } else { $or_ >= $r })
            })

            if $can_be_defended { false } else {
                let advance_rank = $r + $dir
                if $advance_rank < 1 or $advance_rank > 8 { false } else {
                    let advance_sq = $"($f)($advance_rank)"
                    # board-piece-at's .role comes back capitalized ("Pawn",
                    # matching rust's role_name) -- distinct from the
                    # lowercase --role a caller passes IN to board-pieces.
                    # Caught here by testing against a real position instead
                    # of assuming case symmetry.
                    (attacks-to $fen $advance_sq $opponent $occ | where { |a| ($fen | chessdb board-piece-at --square $a).role == "Pawn" } | is-not-empty)
                }
            }
        } | each { |sq| {color: $color, square: $sq} }
    } | flatten
}

# Everything above, bundled -- one call for the whole gap-audit batch.
# Deliberately excludes `piece-mobility-safety`/`king-mobility` (both take
# a required `square`, scoped to one piece, not a whole-board sweep) and
# `double_check`/`fifty_move_rule` (already derivable from `full-report`'s
# existing `checkers`/`halfmove_clock` fields, not worth a second copy).
export def extra-detectors [fen: string] {
    {
        battery: (battery $fen),
        knights_on_rim: (knights-on-rim $fen),
        fianchettoed_bishops: (fianchettoed-bishops $fen),
        long_diagonal_pieces: (long-diagonal-pieces $fen),
        semi_open_files: (semi-open-files $fen),
        supported_pawns: (supported-pawns $fen),
        backward_pawns: (backward-pawns $fen),
    }
}
