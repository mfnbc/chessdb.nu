# Ingest layer: sync from chess.com, browse games, move-by-move review.

use db.nu [db-merge, init-db, fetch-and-seed-eco, enrich-openings]

# Process a list of game records and merge them into the database.
def import-records [games: list, username: string, db: string] {
    init-db $db
    let corpus = ($games | to json | chessdb process-corpus --username $username)

    if ($corpus.games | is-not-empty) {
        db-merge $db "games" $corpus.games [
            "game_id" "source" "source_game_id" "white" "black"
            "white_elo" "black_elo" "result" "played_at" "time_control" "eco" "opening"
        ]
    }
    if ($corpus.positions | is-not-empty) {
        db-merge $db "positions" $corpus.positions [
            "zobrist" "fen" "hugm_score" "hugm_eval_arr" "board_pieces"
            "state_id" "mate_in_1" "is_checkmate"
        ]
    }
    if ($corpus.moves | is-not-empty) {
        db-merge $db "moves" $corpus.moves [
            "game_id" "position_id" "next_position_id" "ply" "move_number" "color" "san" "canonical_san" "uci"
        ]
    }

    # Decode state_id bit-field into move_states rows for fast coaching queries.
    # Bit positions here must match nu_plugin_chessdb/src/eval/concepts.rs's
    # BIT_* constants (encode_state/decode_state_id) — this is the one place
    # on the Nu/SQL side that decodes state_id; downstream queries
    # (chessdb/profile.nu) read these named columns, never re-shift state_id.
    if ($corpus.moves | is-not-empty) {
        try {
            open $db | query db "
                INSERT OR IGNORE INTO move_states
                    (game_id, ply, state_id, phase_bucket, has_fork, has_pin, has_hanging, king_exposed,
                     has_outpost, has_open_file, has_passed_pawn)
                SELECT m.game_id, m.ply,
                    COALESCE(p.state_id, 0),
                    (COALESCE(p.state_id, 0) & 3),
                    ((COALESCE(p.state_id, 0) >> 7) & 1),
                    ((COALESCE(p.state_id, 0) >> 8) & 1),
                    ((COALESCE(p.state_id, 0) >> 9) & 1),
                    ((COALESCE(p.state_id, 0) >> 5) & 1),
                    ((COALESCE(p.state_id, 0) >> 10) & 1),
                    ((COALESCE(p.state_id, 0) >> 11) & 1),
                    ((COALESCE(p.state_id, 0) >> 12) & 1)
                FROM moves m JOIN positions p ON m.next_position_id = p.zobrist
            " | ignore
        } catch { }
    }

    let g = ($corpus.games     | length)
    let p = ($corpus.positions | length)
    let m = ($corpus.moves     | length)
    print $"Imported: ($g) games, ($p) positions, ($m) moves."
}

# Move-by-move evaluation breakdown for one game.
def review-game [game_id: int, db: string] {
    let raw = (open $db | query db "
        SELECT m.ply, m.move_number, m.color, m.san, p.hugm_score, p.hugm_eval_arr
        FROM moves m
        JOIN positions p ON m.next_position_id = p.zobrist
        WHERE m.game_id = ?
        ORDER BY m.ply ASC
    " --params [$game_id])

    $raw | enumerate | each { |item|
        let row      = $item.item
        let prev_arr = if $item.index == 0 { [0 0 0 0 0 0 0 0 0 0 0] } else {
            try { ($raw | get ($item.index - 1)).hugm_eval_arr | from json } catch { [0 0 0 0 0 0 0 0 0 0 0] }
        }
        let arr  = try { $row.hugm_eval_arr | from json } catch { $prev_arr }
        let sign = match $row.color { "black" => -1, _ => 1 }
        let d    = ($arr | zip $prev_arr | each { |p| ($p.0 - $p.1) * $sign })
        {
            "#":         $row.move_number
            color:       $row.color
            move:        $row.san
            score:       ($row.hugm_score * $sign)
            Δ_material:  ($d | get 0)
            Δ_structure: ($d | get 1)
            Δ_activity:  ($d | get 2)
            Δ_king:      ($d | get 3)
            Δ_passed:    ($d | get 4)
            Δ_dev:       ($d | get 5)
            Δ_space:     ($d | get 6)
            Δ_strategic: ($d | get 7)
        }
    }
}

# Download all chess.com games for a player and store them with HUGM evaluations.
export def "chess-sync" [
    username: string              # chess.com username
    --db: string = "./chess.db"
    --limit: int                  # fetch only the last N monthly archives
] {
    let archives = (http get $"https://api.chess.com/pub/player/($username)/games/archives").archives
    let targets  = if ($limit | is-empty) { $archives } else { $archives | last $limit }
    print $"Fetching ($targets | length) archives for ($username)..."

    let games = (
        $targets | par-each { |url|
            try { (http get $url).games } catch {
                sleep 5sec
                try { (http get $url).games } catch { null }
            }
        } | compact | flatten
    )

    print $"Processing ($games | length) games..."
    import-records $games $username $db
    enrich-openings $db
}

# The N most recent games (default 5).
export def "chess-recent" [
    n: int = 5
    --db: string = "./chess.db"
] {
    open $db | query db "
        SELECT game_id, played_at, white, black, result, opening
        FROM games ORDER BY played_at DESC LIMIT ?
    " --params [$n]
}

# Move-by-move evaluation breakdown for a game.
export def "chess-review" [
    game_id: int
    --db: string = "./chess.db"
] {
    review-game $game_id $db
}

# Move frequencies and average ELO for a position (identified by its
# canonical, White-always-to-move Zobrist hash — see nu_plugin_chessdb's
# PLAN.md "Canonical position identity" section). With --username, also
# reports how often that specific player faced this position: since the
# position collapses real color-mirror occurrences from either side of any
# game onto one row, "hit this position" only means something relative to
# whose turn it actually was — times_to_play counts occurrences where the
# player was the one to move, times_to_wait counts occurrences (in the
# player's own games) where the opponent was to move instead.
export def "chess-explore" [
    zobrist: string
    --db: string = "./chess.db"
    --username: string
] {
    let moves_breakdown = (open $db | query db "
        SELECT m.canonical_san as san,
               COUNT(*) as times_played,
               ROUND(AVG((g.white_elo + g.black_elo) / 2.0)) as avg_elo
        FROM moves m JOIN games g ON m.game_id = g.game_id
        WHERE m.position_id = ?
        GROUP BY m.canonical_san ORDER BY times_played DESC
    " --params [$zobrist])

    if ($username | is-empty) {
        return $moves_breakdown
    }

    let familiarity = (open $db | query db "
        SELECT
            SUM(CASE WHEN m.color = (CASE WHEN g.white = ? THEN 'white' ELSE 'black' END)
                THEN 1 ELSE 0 END) as times_to_play,
            SUM(CASE WHEN m.color != (CASE WHEN g.white = ? THEN 'white' ELSE 'black' END)
                THEN 1 ELSE 0 END) as times_to_wait
        FROM moves m JOIN games g ON m.game_id = g.game_id
        WHERE m.position_id = ? AND (g.white = ? OR g.black = ?)
    " --params [$username, $username, $zobrist, $username, $username]).0?
        | default {times_to_play: 0, times_to_wait: 0}

    {
        times_to_play: ($familiarity.times_to_play | default 0)
        times_to_wait: ($familiarity.times_to_wait | default 0)
        moves: $moves_breakdown
    }
}
