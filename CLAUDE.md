# chessdb.nu — coding guidance for Claude

## Nushell idioms (chessdb/)

- **`repeat` over `each { "?" }`** — when you need N copies of a fixed string,
  use `"?" | repeat $n | str join ", "` rather than iterating a list and
  ignoring the element.

- **Optional cell paths over `if is-not-empty`** — use `.0?` on a filtered
  table and `$m.field? | default fallback` instead of an is-not-empty branch.
  The `?` suffix safely returns null on empty/missing; `default` handles it.

- **`enumerate` over `reduce` for row-differential work** — when you need the
  previous row's value, use `enumerate` + `$raw | get ($item.index - 1)` rather
  than a `{state, rows}` accumulator. Avoids O(n) `append` clones.

- **`into record` over `reduce -f {} { merge }`** — to fold a list of
  `[$key $value]` pairs into a record, use `| into record` directly.

- **`upsert` over `update` for external/plugin data** — plugin-returned records
  and LEFT-JOIN nullable columns use `upsert`; SQL-guaranteed columns use `update`.

- **`compact` over null-check conditionals** — use `try { fetch } catch { null }
  | compact` to strip failed fetches rather than `if $result != null { ... }`.

- **`all { is-empty }` guard before destructive writes** — before deleting a
  player's coaching data, check that the plugin returned at least one non-empty
  signal list to avoid wiping and not replacing.

## Structured data output — module usage, no conditional print formatting

`chessdb/` is a **Nu module**. All public commands are `export def` in their
respective sub-files (`db.nu`, `sync.nu`, `derive.nu`, `profile.nu`) and
re-exported from `chessdb/mod.nu`.

Commands return structured data (records, tables). Let Nu render it natively.
Do **not** add `--json` flags or conditional print-formatting branches. Just return
`$result`. The caller decides how to render:

```
use chessdb *
chess-profile username              # renders as Nu table
chess-profile username | to json -r # clean JSON for LLM
chess-profile username | get profile-concepts
```

**Subprocess usage** spawns a full Nu process and must `use` the module explicitly:

```
nu -c "use chessdb *; chess-profile username | to json -r"
```

Structured data does **not** cross subprocess boundaries without `| to json -r`.
Use module form (`use chessdb *`) for any interactive pipeline work.

## SQL vs Nushell aggregation

Keep aggregation in SQL. The `CASE WHEN m.ply <= 12 THEN 'opening'` phase
classification and `AVG(json_extract(...))` patterns return ~8 rows from 334K.
Moving them to Nushell (`upsert phase { ... } | group-by`) fetches all 334K rows
first — a ~40,000× data transfer increase for no benefit.

**Rule:** if a query groups + aggregates, the CASE WHEN lives in SQL. Only bring
rows into Nushell when you need per-row transformation that SQL cannot express.

## Nu 0.111 specifics

**`job spawn`** is the correct command (experimental since 0.104).
`job send` / `job recv` pass structured records between threads without blocking.
There is no `job run` — that does not exist.

**`try/catch/finally`**: `finally` runs unconditionally regardless of success or
failure in the `try` body.

**Pass-through `let` (0.111+):** `let` without `=` is a pipeline pass-through —
it binds `$in` to the variable name and forwards the value unchanged:
```
"hello" | let msg | str length   # → 5; $msg is now "hello"
```
This is distinct from statement-assignment (`let x = ...`). Do **not** use `=`
in a pipeline context; `$data | let x = $in` fails at parse time.

**`repeat` does not exist in Nu 0.111.** To generate N copies of a string use
`1..N | each { "str" } | str join sep`. The `str expand` command exists but is
for brace expansion (`{A,B}`) — not useful for SQL placeholder generation.

**`match` over binary `if/else` on string values:**
```
let sign = match $row.color { "black" => -1, _ => 1 }
```
Use `match` when branching on a string/int/enum. Reserve `if/else` for boolean
conditions or range checks.

## Model data as typed structs, not string-keyed bags

When a piece of evaluation/domain data needs a name, give it a real field on a struct
named for the service or function that produced it — not a string key read out of a
generic `Map<String, Value>` / `HashMap<String, Value>`. A `.get("some_tag")` call on a
loosely-typed map is a signal the value should be a named field instead: the compiler
can no longer tell you the key is missing, misspelled, or has drifted from what actually
produces it, and nothing stops a second representation of the same data growing up next
to the first.

Concretely: `nu_plugin_chessdb/src/eval/position.rs`'s `EvalGroups` holds 9 named groups,
and each group's raw scoring terms live in a `terms: serde_json::Map<String, Value>`
grab-bag — deliberately private to `position.rs`'s own scoring bookkeeping. Nothing outside
that file's one conversion boundary, `build_sensor_report` (`position.rs:2812`), reads it:
`concepts::extract_concepts`, `render_explanations`, and `render_structured_explanations`
all read only `nu_plugin_chessdb/src/eval/sensor.rs`'s typed `SensorReport` now. This is the
finished state of a migration, not a hypothetical — it's recorded here as the pattern to
keep applying, not a pattern currently being violated. See
`nu_plugin_chessdb/FINDINGS.md`'s "Terms-bag → typed SensorReport migration" section for the
full history (`nu_plugin_chessdb/PLAN.md`'s "How primitives become features" has the current
architecture summary).

The same principle applies on the Nu side: prefer a record with named fields over a
generic key-value table when the shape is known ahead of time.

## Chessdb defers to shakmaty for anything geometric

`shakmaty` already computes attack/reach generation, ray/between/alignment, distance,
blocker/occupancy-aware sliding attacks, square color, and file/rank masks — correctly,
and with real test coverage of its own. `chessdb` never re-derives a geometric or
topological board fact by hand (manual file/rank/diagonal offset arithmetic, walking
squares one at a time) when a shakmaty primitive already answers the same question,
directly or via a small, provably-equivalent composition of its public primitives.

Concretely: `nu_plugin_chessdb/src/eval/position.rs`'s `detect_skewers` used to hand-walk
8 hardcoded direction tuples one square at a time via `File::offset`/`Rank::offset`,
checking occupancy manually — its own sibling function two lines earlier, `detect_pins`,
solved an analogous problem correctly via shakmaty's occupancy-aware
`attacks::rook_attacks`/`attacks::bishop_attacks(sq, occupied)`. `detect_skewers` was
rewritten onto the same primitives (2026-09-02, A/B-verified byte-identical against real
positions before the old implementation was removed — see `FINDINGS.md`). `chebyshev_distance`
similarly used to hand-compute exactly what shakmaty's own `Square::distance` already
computes. `piece_activity_score`'s rook and queen rank bonuses used to hand-branch
`if color.is_white() { Rank::Seventh } else { Rank::Second }` (and the same for the
eighth/sixth/first/third ranks) to find "the enemy's back ranks relative to me" — exactly
what shakmaty's own `Color::relative_rank(rank)` already computes (`Color::White => rank,
Color::Black => rank.flip_vertical()`), swapped 2026-09-04, verified exhaustively rather
than sampled (the branch's only inputs are `Color` and a handful of literal `Rank`
constants — a closed, 2-value domain, so proving `color.relative_rank(r) == old_branch(r)`
for both colors is a complete proof, not an approximation, unlike `detect_skewers`/
`king_safety_score` below which are genuinely position-dependent and need real-position
A/B diffs). The same file also had ~8 more `if color.is_white() { A } else { B }`
branches picking a plain value (not a rank) — `Color::fold_wb(white, black)` is the
generic form of exactly that (`match self { White => white, Black => black }`), and
`Color::backrank()` is the same idea specialized to "my own back rank" (used for the
knight/bishop back-rank penalty and the king-square fallback in
`development_space_score`) — both swapped in alongside `relative_rank` on 2026-09-04,
same exhaustive-proof verification (see `FINDINGS.md`). All five are now the pattern to
follow, not a violation currently being fixed.

The most architecturally significant instance found so far isn't in the scoring code at
all: `nu_plugin_chessdb/src/canonical.rs`'s `flip_colors` — the transform backing the
entire canonical (White-always-to-move) position-identity convention above — used to
hand-rebuild a position's mirrored/recolored board from its role/color bitboards via
`ByRole`/`ByColor`/`Board::try_from_bitboards`, on the explicit (and wrong) belief that
"shakmaty has no single 'swap a position's colors' call." It does: `Board::swap_colors()`
plus `flip_vertical()` is exactly `Board::mirror()`, and `Setup::mirror()` composes the
*whole* transform this function needs (board mirror, turn swap, castling-rights flip,
en-passant-square flip) in one call. Swapped 2026-09-04, verified with the project's real-
position A/B discipline (not exhaustive proof, since board state isn't a small domain) —
a 6-position battery covering full/no/partial castling rights, an active en passant
square, and both colors to move, byte-identical before and after — because this function
backs stored DB identity, not an internal score, so a subtle bug here has a materially
different (and worse) blast radius than the scoring-table items above. See `FINDINGS.md`
for the full verification.

A fourth pass found the same `fold_wb`/`ByColor` pattern in `eval/threat_graph.rs`:
`checkers`/`delivers_check` hand-branched on `color.is_white()` to pick between a
`(white_king, black_king)` tuple field (`ThreatGraph.kings`) — restructured the field
itself onto `ByColor<Option<Square>>` (confirmed zero call sites outside the one file
first, despite it being `pub`) so `.get(color)` replaces the branch outright, not just
relabels it; and `see_chain`'s `winner` string picked via the same pattern, swapped onto
`fold_wb` directly. That pass's own full re-read of `core.rs` and the `eval/concepts*`/
`sensor.rs` layer found nothing further — this is the point of diminishing returns for
this sweep (see `FINDINGS.md`'s 2026-09-04 entries for the complete four-pass history);
further finds should come from a new consumer need, not another blind resweep.

Not every hand-rolled-looking loop is actually a violation, though — check whether the
computation is a genuinely *sequential* one (state carried and short-circuited across
steps) before assuming it decomposes into independent primitive queries.
`king_safety_score`'s pawn shield/storm loop looks like two independent per-file
bitboard lookups, but its single `break` exits the whole loop, coupling "nearest own
pawn" and "nearest enemy pawn" together — a first attempt at splitting it into two
independent `Bitboard::first()`/`.last()` queries (2026-09-02) looked clean and passed
`cargo test`, but was a real semantic break caught only by an explicit before/after
numeric diff against real positions (see `FINDINGS.md`); it was reverted, and the manual
loop — which already uses shakmaty's own `Rank`/`Square`/`Bitboard` types throughout, not
raw arithmetic — was kept as the correct implementation. Any change to code that feeds
the tuned scoring table needs this same explicit A/B diff before being considered done,
not just a passing test suite: no existing test asserts most of these raw group values
exactly, so a subtly wrong "equivalent" rewrite can pass `cargo test` and still be wrong.

`chessdb square-control`/`chessdb square-attackers` are the live-play-facing reason this
matters beyond internal code quality: hand-rolled geometry — checking "does this diagonal
reach that square" by mental arithmetic instead of asking the engine — is exactly the
failure class that hung a bishop in a real game (`FINDINGS.md`, 2026-09-02).

## Canonical (White-to-move) position identity — the tablebase simplification

Everything stored under `positions.zobrist`/`positions.fen` and `openings.fen`
is normalized so White is always the side to move — the same simplification
chess endgame tablebases use to collapse a position and its exact color-mirror
(reached by a different game, or by the other side of the same game) onto one
stored entity, evaluated and looked up once instead of twice. It's mechanical:
mirror the board vertically, swap piece colors, swap castling rights, mirror
the en passant square — exactly shakmaty's own `Setup::mirror()`, which
`nu_plugin_chessdb/src/canonical.rs`'s `flip_colors` calls directly (2026-09-04;
originally hand-rebuilt the same transform from `ByRole`/`ByColor` bitboards
before this was found and swapped — see "Chessdb defers to shakmaty" below and
`FINDINGS.md`). `canonical.rs` is the one implementation, used by both `core.rs`
(position/move identity) and `eval::position` (evaluation normalization).

**The corollary the name implies but is easy to forget: nothing in that
canonical form tells you who is actually White or Black in a real game.** A
canonical FEN's "w" side-to-move token and its square letters' case are not
real-game truth — they're an opaque identity/scoring-normalization key. Any
code that needs to know what actually
happened in a real game — who played a move, what a human should see
reviewing their own game, which color a specific player was — must read that
from `moves.color`, `moves.san` (**not** `moves.canonical_san`), `moves.uci`,
and `games.white`/`games.black`, and conform the canonical data back to that
real context using those columns. Never infer real color from the shape of a
canonical FEN/zobrist itself — it always looks like White is to move,
regardless of what really happened.

Two real bugs shipped from getting this backwards (full history in
`nu_plugin_chessdb/FINDINGS.md`'s "Canonical position identity" section):
- `moves.san` was overwritten with the canonical-frame move instead of the
  real one, so `chess-review` showed players a color-mirrored version of
  their own moves. Fixed by splitting into `san` (real) and `canonical_san`
  (canonical, used only by `chess-explore`'s cross-game grouping — the one
  place mixing real-frame SAN from either side actually would be nonsense).
- `enrich-openings` joined canonical `positions.fen` against real,
  non-canonical ECO FENs (`openings.fen`), silently failing to classify any
  opening recorded at a Black-to-move ply. Fixed by canonicalizing ECO data
  at seed time (`fetch-and-seed-eco`, via the `chessdb canonicalize-fen`
  plugin command) so both sides of that join are actually canonical.

**Quick reference — which side of the data is real vs. canonical:**

| Field | Convention |
|---|---|
| `positions.zobrist` / `.fen` | canonical (White always to move) |
| `openings.fen` | canonical (canonicalized at seed time) |
| `moves.san` | real (as actually played) |
| `moves.canonical_san` | canonical (cross-game grouping only) |
| `moves.uci`, `moves.color` | real |
| `games.white` / `.black` / `.result` | real |
| `positions.hugm_score` / `.state_id` / `.mate_in_1` / `.is_checkmate` | mover-relative scalars — orientation-invariant either way, safe to read directly without conforming |

## SQL string construction in db-merge

`db-merge` (in `chessdb/db.nu`) builds INSERT statements by concatenating
`$table` and `$columns` (both internal literal strings — not user input).
This is **not** an injection risk. All VALUES are parameterised via `--params`.
Do not add escaping or restructure this to avoid a non-existent vulnerability.
