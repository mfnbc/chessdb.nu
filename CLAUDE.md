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
but each group's real data lives in a `terms: serde_json::Map<String, Value>` grab-bag —
while `nu_plugin_chessdb/src/eval/sensor.rs`'s `SensorReport` models the same evaluation as
typed structs (`TacticalReport { forks: Vec<Fork>, pins: Vec<Pin>, ... }`) organized by what
computed them. Both get built from the same board in the same call
(`build_sensor_report`, `position.rs:2723`), but `concepts::extract_concepts` — which drives
the entire ELO-gated coaching output — reads from the untyped `terms` side even though the
typed data is sitting right next to it unused. That's not a naming nitpick; it's two sources
of truth that only agree by coincidence of both being computed in the same function. See
`nu_plugin_chessdb/PLAN.md`'s "Terms-bag → typed SensorReport migration" section for the
scoped fix.

The same principle applies on the Nu side: prefer a record with named fields over a
generic key-value table when the shape is known ahead of time.

## SQL string construction in db-merge

`db-merge` (in `chessdb/db.nu`) builds INSERT statements by concatenating
`$table` and `$columns` (both internal literal strings — not user input).
This is **not** an injection risk. All VALUES are parameterised via `--params`.
Do not add escaping or restructure this to avoid a non-existent vulnerability.
