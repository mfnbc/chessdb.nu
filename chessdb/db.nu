# Schema, migrations, and database management commands.
# Internal exports (db-merge, init-db, fetch-and-seed-eco, enrich-openings) are
# used by sibling module files; they are not re-exported from mod.nu.

# Batch INSERT OR IGNORE. Chunks rows to stay under SQLite's variable limit (~900 params).
export def db-merge [
    db: string
    table: string
    records: list
    columns: list<string>
] {
    if ($records | is-empty) { return }
    let chunk_size = ([1, (900 // ($columns | length))] | math max)
    let col_sql    = ($columns | str join ", ")
    let row_ph     = "(" + (1..($columns | length) | each { "?" } | str join ", ") + ")"
    for chunk in ($records | chunks $chunk_size) {
        let all_ph = ($chunk | each { $row_ph } | str join ", ")
        let params = ($chunk | each { |r| $columns | each { |c| $r | get $c } } | flatten)
        open $db | query db ("INSERT OR IGNORE INTO " + $table + " (" + $col_sql + ") VALUES " + $all_ph) --params $params
    }
}

# Create all tables and apply pending column migrations. Safe to re-run.
export def init-db [db: string] {
    if not ($db | path exists) {
        [{_init: 1}] | into sqlite $db -t _meta
    }
    open $db | query db "PRAGMA journal_mode = WAL"   | ignore
    open $db | query db "PRAGMA synchronous = NORMAL" | ignore

    open $db | query db "
        CREATE TABLE IF NOT EXISTS games (
            game_id        INTEGER PRIMARY KEY,
            source         TEXT,
            source_game_id TEXT,
            white          TEXT,
            black          TEXT,
            white_elo      INTEGER,
            black_elo      INTEGER,
            result         TEXT,
            played_at      DATETIME,
            time_control   TEXT,
            eco            TEXT,
            opening        TEXT
        )
    " | ignore

    open $db | query db "
        CREATE TABLE IF NOT EXISTS positions (
            zobrist       TEXT PRIMARY KEY,
            fen           TEXT UNIQUE,
            hugm_score    INTEGER,
            hugm_eval_arr TEXT,
            state_id      INTEGER,
            mate_in_1     INTEGER DEFAULT 0,
            is_checkmate  INTEGER DEFAULT 0
        )
    " | ignore
    for col_sql in [
        "ALTER TABLE positions ADD COLUMN state_id     INTEGER DEFAULT 0"
        "ALTER TABLE positions ADD COLUMN mate_in_1    INTEGER DEFAULT 0"
        "ALTER TABLE positions ADD COLUMN is_checkmate INTEGER DEFAULT 0"
    ] { try { open $db | query db $col_sql } catch { } }
    # board_pieces/updated_at: computed and stored but never queried anywhere
    # in chessdb/*.nu (confirmed via audit) — dropped 2026-07-30.
    for col_sql in [
        "ALTER TABLE positions DROP COLUMN board_pieces"
        "ALTER TABLE positions DROP COLUMN updated_at"
    ] { try { open $db | query db $col_sql } catch { } }

    open $db | query db "
        CREATE TABLE IF NOT EXISTS moves (
            game_id          INTEGER,
            position_id      TEXT,
            next_position_id TEXT,
            ply              INTEGER,
            move_number      INTEGER,
            color            TEXT,
            san              TEXT,
            canonical_san    TEXT,
            uci              TEXT,
            PRIMARY KEY (game_id, ply),
            FOREIGN KEY (game_id)          REFERENCES games(game_id),
            FOREIGN KEY (position_id)      REFERENCES positions(zobrist),
            FOREIGN KEY (next_position_id) REFERENCES positions(zobrist)
        )
    " | ignore
    # san is the real, as-played move (for single-game review); canonical_san
    # is the same move translated into the canonical (White-to-move) frame
    # positions.zobrist/.fen use, for grouping by position identity across
    # games (see chess-explore) — mixing real-frame SAN from either side
    # under one canonical position would be meaningless there.
    try { open $db | query db "ALTER TABLE moves ADD COLUMN canonical_san TEXT" } catch { }
    open $db | query db "CREATE INDEX IF NOT EXISTS idx_moves_pos ON moves(position_id)" | ignore

    # move_states columns are decoded once, here and in the migration/backfill
    # below plus the INSERT in sync.nu's import-records — from the bit layout
    # defined in nu_plugin_chessdb/src/eval/concepts.rs's BIT_* constants
    # (encode_state/decode_state_id). Downstream queries (chessdb/profile.nu)
    # must read these named columns, never re-shift ms.state_id directly —
    # that was a real, fixed bug (duplicated bit-layout knowledge outside its
    # one source of truth), not a style preference.
    open $db | query db "
        CREATE TABLE IF NOT EXISTS move_states (
            game_id         INTEGER NOT NULL,
            ply             INTEGER NOT NULL,
            state_id        INTEGER NOT NULL,
            phase_bucket    INTEGER NOT NULL,
            has_fork        BOOLEAN NOT NULL DEFAULT 0,
            has_pin         BOOLEAN NOT NULL DEFAULT 0,
            has_hanging     BOOLEAN NOT NULL DEFAULT 0,
            king_exposed    BOOLEAN NOT NULL DEFAULT 0,
            has_outpost     BOOLEAN NOT NULL DEFAULT 0,
            has_open_file   BOOLEAN NOT NULL DEFAULT 0,
            has_passed_pawn BOOLEAN NOT NULL DEFAULT 0,
            PRIMARY KEY (game_id, ply)
        )
    " | ignore
    for col_sql in [
        "ALTER TABLE move_states ADD COLUMN has_outpost     BOOLEAN"
        "ALTER TABLE move_states ADD COLUMN has_open_file   BOOLEAN"
        "ALTER TABLE move_states ADD COLUMN has_passed_pawn BOOLEAN"
    ] { try { open $db | query db $col_sql } catch { } }
    # One-time backfill for rows inserted before the three columns above
    # existed (new rows already get them from sync.nu's INSERT). Guarded to
    # only touch not-yet-backfilled rows, so this is cheap on repeat runs.
    try {
        open $db | query db "
            UPDATE move_states
            SET has_outpost     = (COALESCE(p.state_id, 0) >> 10) & 1,
                has_open_file   = (COALESCE(p.state_id, 0) >> 11) & 1,
                has_passed_pawn = (COALESCE(p.state_id, 0) >> 12) & 1
            FROM moves m JOIN positions p ON m.next_position_id = p.zobrist
            WHERE move_states.game_id = m.game_id AND move_states.ply = m.ply
              AND (move_states.has_outpost IS NULL OR move_states.has_open_file IS NULL OR move_states.has_passed_pawn IS NULL)
        "
    } catch { }

    # The failure-lattice rungs (threat_graph.rs module doc, FINDINGS.md): raw
    # miscount (outnumbered), a defender already committed elsewhere
    # (overloaded, false_defense), and the composite rung above both
    # (false_safety) that fires when the raw count alone said "safe" but
    # isn't once those commitments are discounted. Same bit-column pattern
    # as has_outpost/has_open_file/has_passed_pawn above — added when
    # state_id was widened from u16 to u32 (bits 15-18) to make room.
    for col_sql in [
        "ALTER TABLE move_states ADD COLUMN has_outnumbered   BOOLEAN"
        "ALTER TABLE move_states ADD COLUMN has_overloaded    BOOLEAN"
        "ALTER TABLE move_states ADD COLUMN has_false_defense BOOLEAN"
        "ALTER TABLE move_states ADD COLUMN has_false_safety  BOOLEAN"
    ] { try { open $db | query db $col_sql } catch { } }
    # Same backfill pattern as above, with one honest caveat: rows whose
    # positions.state_id was computed before this widening will backfill to
    # false here regardless of whether the position actually had the
    # pattern — those bits didn't exist yet to be set. Only newly (re-)
    # evaluated positions carry real values for bits 15-18; historic rows
    # need `chessdb derive-coach-signals` re-run over freshly re-evaluated
    # positions to pick these up, the same limitation any bit added to an
    # already-populated state_id column would have.
    try {
        open $db | query db "
            UPDATE move_states
            SET has_outnumbered   = (COALESCE(p.state_id, 0) >> 15) & 1,
                has_overloaded    = (COALESCE(p.state_id, 0) >> 16) & 1,
                has_false_defense = (COALESCE(p.state_id, 0) >> 17) & 1,
                has_false_safety  = (COALESCE(p.state_id, 0) >> 18) & 1
            FROM moves m JOIN positions p ON m.next_position_id = p.zobrist
            WHERE move_states.game_id = m.game_id AND move_states.ply = m.ply
              AND (move_states.has_outnumbered IS NULL OR move_states.has_overloaded IS NULL
                   OR move_states.has_false_defense IS NULL OR move_states.has_false_safety IS NULL)
        "
    } catch { }

    open $db | query db "
        CREATE TABLE IF NOT EXISTS player_baselines (
            username     TEXT    NOT NULL,
            concept_name TEXT    NOT NULL,
            phase_bucket INTEGER NOT NULL,
            mean         REAL    NOT NULL DEFAULT 0,
            std          REAL    NOT NULL DEFAULT 0,
            count        INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (username, concept_name, phase_bucket)
        )
    " | ignore
    try { open $db | query db "ALTER TABLE player_baselines ADD COLUMN std REAL NOT NULL DEFAULT 0" } catch { }
    # last_updated: declared, never written or read anywhere — dropped
    # 2026-07-30. count *was* the same kind of dead column (declared, never
    # populated with a real value) until this same pass wired it up for
    # real: it now carries the actual Welford sample size a baseline was
    # built from, which chess-derive --min-games gates anomaly emission on
    # (see coach_derive_cmd.rs) — kept and fixed, not dropped.
    try { open $db | query db "ALTER TABLE player_baselines DROP COLUMN last_updated" } catch { }

    open $db | query db "
        CREATE TABLE IF NOT EXISTS transition_events (
            username      TEXT    NOT NULL,
            state_from    INTEGER NOT NULL,
            state_to      INTEGER NOT NULL,
            total_count   INTEGER NOT NULL DEFAULT 0,
            blunder_count INTEGER NOT NULL DEFAULT 0,
            blunder_risk  REAL    NOT NULL DEFAULT 0,
            PRIMARY KEY (username, state_from, state_to)
        )
    " | ignore
    try { open $db | query db "ALTER TABLE transition_events DROP COLUMN last_updated" } catch { }

    open $db | query db "
        CREATE TABLE IF NOT EXISTS openings (
            fen   TEXT PRIMARY KEY,
            eco   TEXT NOT NULL,
            name  TEXT NOT NULL
        )
    " | ignore
    open $db | query db "CREATE INDEX IF NOT EXISTS idx_openings_eco ON openings(eco)" | ignore
    # moves: the ECO source's move-list text, stored but never queried
    # anywhere in chessdb/*.nu — dropped 2026-07-30.
    try { open $db | query db "ALTER TABLE openings DROP COLUMN moves" } catch { }

    open $db | query db "
        CREATE TABLE IF NOT EXISTS move_anomalies (
            alert_id     INTEGER PRIMARY KEY AUTOINCREMENT,
            username     TEXT    NOT NULL,
            game_id      INTEGER NOT NULL,
            ply          INTEGER NOT NULL,
            state_id     INTEGER NOT NULL,
            anomaly_type TEXT    NOT NULL,
            concept_name TEXT,
            z_score      REAL,
            severity     REAL    NOT NULL DEFAULT 0,
            signed_delta INTEGER,
            hurt_player  BOOLEAN NOT NULL DEFAULT 0,
            created_at   TEXT    DEFAULT (datetime('now')),
            consumed     BOOLEAN NOT NULL DEFAULT 0
        )
    " | ignore
    # Unique constraint makes re-derive idempotent: INSERT OR IGNORE preserves consumed flags.
    try {
        open $db | query db "
            CREATE UNIQUE INDEX IF NOT EXISTS idx_anomaly_unique
            ON move_anomalies(username, game_id, ply, concept_name)
        "
    } catch { }

    # tactical_events: one row per individual failure-lattice finding
    # (hanging/outnumbered/overloaded/false_defense/false_safety instance),
    # not an aggregate. The flat columns (square, concept_name, side,
    # severity, stage) are what's quantifiable and worth indexing/graphing
    # across a game; `detail` is the fully-identifiable structured payload
    # (named pieces, king-zone deltas, checkers — the actual
    # ThreatGraph::collapse_criticality/HangingPiece-shaped JSON) for a
    # human or LLM to read at review time. Deliberately no narrative/
    # description column: synthesizing "why this matters" from these facts
    # is interpretation, not quantification, and belongs at read time, not
    # frozen into a row that could only ever serve one framing — see
    # FINDINGS.md's "what can be described vs. what can be detected" entry.
    open $db | query db "
        CREATE TABLE IF NOT EXISTS tactical_events (
            event_id     INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id      INTEGER NOT NULL,
            ply          INTEGER NOT NULL,
            square       TEXT    NOT NULL,
            concept_name TEXT    NOT NULL,
            side         TEXT    NOT NULL,
            severity     INTEGER NOT NULL DEFAULT 0,
            stage        INTEGER NOT NULL DEFAULT 0,
            detail       TEXT,
            created_at   TEXT    DEFAULT (datetime('now'))
        )
    " | ignore
    # Unique constraint makes re-derive idempotent, same pattern as move_anomalies.
    try {
        open $db | query db "
            CREATE UNIQUE INDEX IF NOT EXISTS idx_tactical_events_unique
            ON tactical_events(game_id, ply, square, concept_name)
        "
    } catch { }
    open $db | query db "CREATE INDEX IF NOT EXISTS idx_tactical_events_game ON tactical_events(game_id, ply)" | ignore
}

# Download ecoA–E.json from JeffML/eco.json and populate the openings table. No-op if already seeded.
export def fetch-and-seed-eco [db: string] {
    let existing = (open $db | query db "SELECT COUNT(*) as cnt FROM openings").0.cnt | into int
    if $existing > 0 { return }
    print "Downloading ECO opening data from JeffML/eco.json..."
    let base = "https://raw.githubusercontent.com/JeffML/eco.json/master"
    let rows = ["ecoA" "ecoB" "ecoC" "ecoD" "ecoE"] | par-each { |f|
        try {
            http get $"($base)/($f).json"
            | items { |fen, data| {
                fen:  $fen
                eco:  ($data.eco?  | default "")
                name: ($data.name? | default "")
            }}
        } catch { [] }
    } | flatten
    if ($rows | is-empty) {
        print "Warning: ECO download failed — opening enrichment disabled."
        return
    }
    # ECO data is keyed by real FENs at whatever ply/side eco.json recorded
    # them at, but enrich-openings joins against positions.fen, which is
    # canonical (White-always-to-move) — convert once here, in one batched
    # plugin call, or matching silently fails for every opening recorded at
    # a Black-to-move ply.
    let canonical_fens = ($rows | get fen | chessdb canonicalize-fen)
    let rows = ($rows | enumerate | each { |item| $item.item | upsert fen ($canonical_fens | get $item.index) })
    db-merge $db "openings" $rows ["fen" "eco" "name"]
    print $"Seeded ($rows | length) ECO opening positions."
}

# Update games.eco and games.opening to the deepest opening FEN match per game.
export def enrich-openings [db: string] {
    let has_data = (open $db | query db "SELECT COUNT(*) as cnt FROM openings").0.cnt | into int
    if $has_data == 0 { return }
    open $db | query db "
        UPDATE games
        SET eco     = best.eco,
            opening = best.name
        FROM (
            SELECT m.game_id, o.eco, o.name
            FROM moves m
            JOIN positions p ON m.next_position_id = p.zobrist
            JOIN openings  o ON p.fen = o.fen
            WHERE m.ply = (
                SELECT MAX(m2.ply)
                FROM moves m2
                JOIN positions p2 ON m2.next_position_id = p2.zobrist
                JOIN openings  o2 ON p2.fen = o2.fen
                WHERE m2.game_id = m.game_id
            )
            GROUP BY m.game_id
        ) best
        WHERE games.game_id = best.game_id
    " | ignore
}

# Initialise the database schema and seed ECO opening data (safe to re-run).
export def "chess-init" [--db: string = "./chess.db"] {
    init-db $db
    fetch-and-seed-eco $db
    enrich-openings $db
    print $"Database ready: ($db)"
}

# Database record counts and per-player game totals.
export def "chess-status" [--db: string = "./chess.db"] {
    if not ($db | path exists) { error make {msg: $"No database at ($db)"} }
    {
        counts:  (open $db | query db "
            SELECT (SELECT COUNT(*) FROM games)     as games,
                   (SELECT COUNT(*) FROM positions) as positions,
                   (SELECT COUNT(*) FROM moves)     as moves
        ").0
        players: (open $db | query db "
            SELECT player, COUNT(*) as games FROM (
                SELECT white as player FROM games
                UNION ALL
                SELECT black as player FROM games
            ) GROUP BY player ORDER BY games DESC
        ")
    }
}

# Re-download ECO opening data and re-enrich all games. Use after eco.json updates upstream.
export def "chess-seed-openings" [--db: string = "./chess.db"] {
    if not ($db | path exists) { error make {msg: $"Database not found: ($db)"} }
    open $db | query db "DELETE FROM openings" | ignore
    fetch-and-seed-eco $db
    enrich-openings $db
    print "Opening enrichment complete."
}
