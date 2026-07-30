PLAN: HUGM (Human GM) evaluation — roadmap, status, and schema

Purpose
- Document goals and engineering plan for HUGM (formerly "critter") static evaluation.
- Provide a compact status update, a structured explanation JSON schema for consumers (chessdb/ai.nu, LLMs), and a short actionable plan.

Repo note (2026-07-28): the project was renamed and modularised (commit `4121e53`,
"refactor: rename to chessdb.nu, modularise, add chess- prefix"). References below
to `nuchessdb.nu`, `nu-agent/`, `derive-coach`, `coach-review`, `validate-gate.nu`,
`dictionary-update.nu` from earlier checkpoints have been removed — those were
superseded by the `chessdb/` Nu module (`chess-derive`, `chess-review`,
`chess-validate` in `chessdb/derive.nu` and `chessdb/sync.nu`) and the
`ai.nu`-powered analyst/coach in `ai/mod.nu`. See `../README.md` and
`../CLAUDE.md` for the current architecture.

High-level goals
- Static, explainable bitboard heuristics (no search inside HUGM).
- Human-readable annotations (phrases + structured JSON) useful for coaching and LLM consumption.
- Default analytics-friendly numeric output; verbose mode (--verbose / -v) emits explanations + structured annotations.
- Centralize guessed weights and provide a runtime override (--weights) for experimentation without recompilation.

Status summary (by feature)
- A: Tactical motifs (pins, forks, skewers, discovered)
  - Status: DONE. Implemented detect_pins, detect_forks, detect_skewers, detect_discovered. Examples stored in tactical.terms (fork_example_us, skewer_example_us, etc.). Unit tests present.

- B: King tropism
  - Status: DONE. king_tropism_score implemented and integrated into king_safety_group.terms.

- C: Rook activity (open files, 7th, doubled)
  - Status: DONE. Open-file control, rook-on-7th, doubled-rooks detected and included in piece_activity terms.

- D: Mobility & PST (per-piece mobility counters + PST hook)
  - Status: PARTIAL. Per-piece mobility counters (mobility_knight/bishop/rook/queen/pawn,
    mobility_total) are implemented and weighted (`piece_mobility_weight`,
    `src/eval/position.rs:1253-1313`). Piece-square tables are still NOT implemented —
    `piece_square_name()` is a square-naming helper for explanation text, not an
    evaluation table.

- E: Outpost / blockade
  - Status: DONE (basic). detect_outposts implemented and example context returned; blockades not fully fleshed beyond passed pawn scoring.

- F: Pawn-majority / break potential
  - Status: DONE. `pawn_majority`, `pawn_break`, `minority_attack` concepts implemented
    with dedicated weights (`src/eval/position.rs:68-70,727`) and ELO-gated at 1800/1800/2000
    respectively (`src/eval/concepts.rs:66-77`).

- G: Endgame overrides & king-activity bonuses
  - Status: PARTIAL. win_chance_scale() and draw heuristics exist; explicit small-material overrides or K+P rules not added as a dedicated feature.

Infrastructure & tooling
- Weights centralization: DONE. GUESS weights collected; Weights struct + WEIGHTS global added. Runtime override via set_weights_from_file(path) and --weights CLI flag.
- CLI: chessdb hugm-eval: default analytics-only outputs numeric groups (hugm_score, hugm_eval_arr). --verbose / -v adds "explanations" and "explanations_structured" arrays. --weights / -w loads a JSON weights file.
- Clippy: applied low-risk fixes; clippy-clean and unit tests pass (`cargo test`: 10 passed across `motif_canonical.rs` (7) and `ingest_pipeline.rs` (3), 0 failed, as of 2026-07-28).

Structured explanation JSON (schema & example)
- Purpose: give a compact, predictable shape that chessdb/ai.nu and LLM prompts can rely on.

Schema (concise)
- explanations_structured: array of Explanation objects.
- Explanation object:
  - kind: string (e.g., "fork", "pin", "skewer", "outpost", "rook_open_files", "none")
  - side: string ("white" or "black") — whose features are being reported
  - severity: integer (signed centipawn-like magnitude or simple count)
  - phrase: short human-readable string summarizing the observation
  - details: object with motif-specific keys (see examples)

Example (JSON-like)
- Single fork explanation example:
  {
    "kind": "fork",
    "side": "white",
    "severity": 80,
    "phrase": "White has 1 fork(s) detected (e.g. Nd5 forks Qf6 and Rb6).",
    "details": {
      "example": {
        "attacker": "Nd5",
        "targets": ["Qf6", "Rb6"]
      }
    }
  }

- Skewer example:
  {
    "kind": "skewer",
    "side": "white",
    "severity": 40,
    "phrase": "White has a skewer (e.g. Rg7: Rf7 -> Qf8).",
    "details": { "example": { "attacker": "Rg7", "front": "Rf7", "back": "Qf8" } }
  }

- Outpost example:
  {
    "kind": "outpost",
    "side": "black",
    "severity": 40,
    "phrase": "Black has 1 outpost(s) (e.g. Nb4 supported by c5).",
    "details": { "example": { "square": "b4", "role": "N", "support": "c5" } }
  }

Notes on schema
- details is intentionally flexible; motif detectors should place a small structured object under details.example for immediate consumption.
- severity may be a small signed integer (centipawn-ish) or a motif count depending on context. Consumers should treat it as a signed integer representing importance; phrase supplies natural-language text.

Compact actionable plan (short-term next steps)
1. Implement --examples N (default 1): return up to N motif examples per motif when verbose. Tests: ensure arrays length ≤ N. Still NOT implemented — no `--examples` flag in `src/hugm_eval_cmd.rs`.
2. Add PST hook in piece_activity_score (D). Mobility counters are already done; PST is the remaining gap. Expose PST enable via weights or a toggle flag. Add tests.
3. Expand detectors to return multiple examples (fork_examples_us -> Vec<...>) and update render_structured_explanations accordingly.
4. Add optional persistable weights profiles and an example weights JSON file in repo (eval/weights_example.json). Update README with a short usage snippet.
5. When features are stable, design and run the corpus-based ELO tuning pipeline (research project) — see Phase D below.

Longer-term ideas (deferred)
- Attribution model for corpus-driven tuning (term → move influence → game outcome).
- LLM-driven summary templates and coach-grade suggestions built on structured explanations — largely realized by `chess coach` / `chess analyst` in `ai/mod.nu`; deeper templates still open.
- PST/NNUE co-training: expose hooks to swap/tune PSTs alongside NNUE models.

Contact
- File location: nu_plugin_chessdb/src/eval/position.rs (core); hugm eval entry: nu_plugin_chessdb/src/hugm_eval_cmd.rs
- Repo branch: main

Validation & Tuning Plan (detailed — follow-through)

Goal
- Ensure HUGM detections are precise (avoid hallucination), explainable, and improve iteratively using canonical examples and real-world corpora before large-scale weight tuning.

Phase A — Canonical examples & unit tests
- Status: substantially done. `nu_plugin_chessdb/tests/motif_canonical.rs` has 7 passing
  canonical-position tests (pins, forks, skewers, discovered, outposts, rook activity,
  pawn structure). Remaining gap: negative/near-miss cases per motif (guard against
  hallucination) are not yet systematic.
- Sources used: chessprogramming.org, Wikipedia (pawn examples), representative Lichess-style positions.

Phase B — Small labeled corpus + evaluation harness (short term)
- Status: scaffolding exists. `src/bin/hugm_harness.rs` (348 lines) reads a JSONL of
  `{fen, engine_score}` records and can compute regression/metrics against HUGM output;
  `src/bin/lichess_to_jsonl.rs` and `src/bin/pgn_to_jsonl.rs` produce candidate input.
  Full TP/FP/FN precision/recall reporting per motif is not yet wired up — the harness
  currently focuses on HUGM-vs-engine-score regression (see NNUE_AUDIT.md).

Phase C — Real-world sampling and human review (medium term)
- Not started. Sample positions stratified by ELO from a large corpus (e.g., Lichess monthly dumps).
- Run HUGM verbose on sample; surface detected examples per motif to a human review step (CSV or small UI) for labeling.
- Use reviewed labels to estimate real-world precision and guide detector refinement.

Phase D — Algorithmic weight tuning (deferred research)
- Not started. After detectors are validated, implement an attribution model to map move deltas to influencing terms and then aggregate outcomes by ELO bucket.
- Use aggregated statistics to propose weight updates (regularized optimization, grid/hillclimb, or regression) with holdout validation to prevent overfitting.
- Iterate until improvements generalize across ELO buckets.

Policy: Stockfish evaluation handling
- Do NOT persist Stockfish numeric evaluations as canonical fields in the positions table by default. Stockfish is a computational oracle used for two purposes: (1) on‑demand review and (2) ephemeral labeling for training. Persisting engine scores in the primary analytics DB biases the canonical dataset toward machine judgments and reduces the pedagogical, human‑interpretable clarity of HUGM outputs.
- Operational rules:
  - On‑demand review: provide a `review` path that runs Stockfish live and returns an ephemeral evaluation to the user; do not store those numbers automatically.
  - Labeling for training: when Stockfish labels are required, generate them in a separate labeling pipeline and store them only in training shards/manifest (NPZ/JSON) with full provenance (engine version, parameters, date).
  - Auditability: record the labeling run metadata in the dataset manifest; do not bake engine outputs into the main positions table unless explicitly requested.

Operational notes
- Default pipeline: continue to emit only scalars (hugm_score/hugm_eval_arr) for corpus ingestion.
- Verbose-only: structured examples are emitted only when --verbose is passed; these are not stored by default to avoid DB bloat.
- Weights: keep WEIGHTS runtime override for fast experimentation; persist profiles separately if/when needed.

Architecture notes (still current)
- The convergence gate solves the digital-switch vs analog-dial problem: survival/threat sensors
  always active, positional sensors dampened at 50% of chaos, strategic sensors fully suppressed.
  `chaos_coefficient` (`src/eval/position.rs:2314`) reads forks+pins+skewers+hanging+in_check+king_exposed
  from sensor terms. `SensorTier`/`tier_for_concept`/`attenuation` live in `src/eval/concepts.rs:302-330`.
- The ELO-gated concept table (`src/eval/concepts.rs`) currently unlocks concepts up through
  roughly 1400 (tactics, rook activity, king safety, passed pawns, development) with a further
  1800/2000 tier for pawn-majority/pawn-break/minority-attack already implemented but gated off
  for lower-rated players.
- The coach pipeline says "this was unusual *for you*" via per-player z-score baselines
  (`chess-derive` → `chessdb derive-coach-signals`, see `chessdb/derive.nu`).

Terms-bag → typed SensorReport migration (scoped and completed 2026-07-28)

Status: DONE. `extract_concepts` now takes `&SensorReport` and reads typed fields for every
concept except the handful of legitimately-scalar `GroupValue` magnitudes noted below (those
were never part of the tag-bag problem). `EvalGroups.terms` is now purely an internal
scratch representation inside `position.rs` — nothing outside that file (or outside
`build_sensor_report`'s own boundary conversion) reads a `.terms.get("...")` key anymore.

What changed:
- `concept_types.rs`: added `PawnMajority`, `RookOnSeventh`, `CenterControl`.
- `sensor.rs`: `PositionalReport` gained `pawn_majority`, `rook_on_seventh`, `center_control`;
  `SensorReport` gained a top-level `in_check: bool`.
- `position.rs`: added `extract_pawn_majority`/`extract_rook_on_seventh` (read the now-verified-correct
  `groups.pawn_structure.terms.get("majority_us"/"majority_them")` and
  `groups.piece_activity.terms.get("rook_on_seventh")`/nested `opp_terms.rook_on_seventh` —
  this is the one place `.terms` is still read, as the designed conversion boundary) and
  `extract_center_control` (self-contained, reuses the existing `center_control_score` fn
  directly against the board, no `.terms` involved). `build_sensor_report` now computes
  `in_check` once via `chess.is_check()` and shares one partial `SensorReport` between
  `encode_state` and `extract_concepts` instead of building two.
- `concepts.rs::extract_concepts` rewritten to filter typed `Vec`s by `PieceRef.color` /
  struct `color` fields instead of doing paired `_us`/`_them` string lookups. Also added a
  `hanging_piece` concept block — the tier/confidence system (`tier_for_concept`,
  `rank_issues_for_position`'s confidence match) already had `"hanging_piece"` wired in, but
  no block ever emitted one; `sensor.tactical.hanging` was sitting right there unused.
- `hugm_eval_cmd.rs`: both call sites updated to pass `&record.sensor_report` alongside
  `&record.groups`.

Bugs found and fixed along the way (verified with `analyze_fen_with_engine_score` directly,
not just `cargo test` — see `tests/motif_canonical.rs::discovered_negative_starting_position`):
- Several `extract_concepts` blocks were querying **keys that never existed** in `groups.*.terms`
  (`rook_open_file_us`, `rook_seventh_us`, `passed_us`, `passed_them` — the real keys were
  `open_files_controlled`/`rook_on_seventh`/`passed_count`, mostly unsuffixed, `_us`-only).
  Those concepts silently never fired in `gated_issues`, ever — no compiler error, because a
  string map doesn't have one. This is the exact failure mode the CLAUDE.md "typed structs, not
  string-keyed bags" convention describes, now with a concrete before/after: `rook_open_file`
  and `rook_seventh` fire correctly now via the typed `open_files`/`rook_on_seventh` fields.
- `detect_discovered` (`position.rs`) flagged a "discovered attack" whenever removing *any* own
  piece opened *any* slider's line to *any* enemy piece, with no defended/material-significance
  check — so it fired 3-per-side on the plain starting position (e.g. Ra1 "discovering" an
  attack on a7 if the a2 pawn moved). This was pre-existing and already reachable through the
  correctly-keyed `discovered_us`/`discovered_them` terms, but nothing had ever run the full
  pipeline output through `gated_issues` and looked at it before. Fixed: a reveal only counts
  if the target is undefended or worth more than the attacking slider. Regression test added.

Not touched: `rank_issues_for_player` (delta-based gating) is unused anywhere in the codebase —
left as-is, out of scope. The starting-position `king_exposed` concept still fires at severity
217 (threshold is 40) — that's a `king_safety_score` calibration question, unrelated to this
migration; noted here in case it's confusing when next seen.

Original finding (for context): `build_sensor_report` (`src/eval/position.rs:2723-2842`) assembles two parallel
representations of the same evaluation in the same call — a typed one (`TacticalReport`,
`PositionalReport`, `MaterialConceptReport` in `src/eval/sensor.rs` / `concept_types.rs`,
built by dedicated `extract_*`/`*_to_typed` functions) and an untyped one (`EvalGroups`,
`src/eval/position.rs:225-266`, whose 9 named groups each hold a `terms: serde_json::Map<String, Value>`
grab-bag). `extract_concepts` (`concepts.rs:14-138`) — the function that produces `gated_issues`,
i.e. everything the ELO-gated coach pipeline shows a player — reads from the **untyped**
`groups.*.terms.get("some_key")` side, not the typed structs sitting right next to it.
Both get serialized onto every `PositionRecord`, so they're duplicate sources of truth that
only agree because they're computed from the same board in the same call.

Per-concept audit (12 blocks in `extract_concepts`):
- **Direct typed equivalent already exists — straightforward to migrate:**
  bishop_pair (`sensor.material.balance.bishop_pair_white/black` — already `bool`, no
  re-derivation needed), forks/pins/skewers/discovered (`sensor.tactical.{forks,pins,skewers,discovered}`,
  filter by `PieceRef.color` instead of separate `_us`/`_them` keys), isolated/doubled pawns
  (`sensor.positional.{isolated_pawns,doubled_pawns}`, each carries a real `color` field),
  outposts (`sensor.positional.outposts`, filter by piece color), passed pawns
  (`sensor.positional.passed_pawns`, filter by color), king_in_check (`record.legal.is_check`
  on `PositionRecord.legal`, not even part of `SensorReport` — already typed and unused here).
- **No typed field exists yet — needs a new struct/field before its `terms.get()` can retire:**
  pawn_majority (no `PawnMajority` concept type), rook_seventh (`OpenFile` only models open
  files, not 7th-rank occupation), king_exposed-by-magnitude (`KingExposure` is attacker/shelter-count
  based; the current concept uses `groups.king_safety.blended`, a different centipawn-magnitude
  metric — semantics need reconciling, not just a field rename), development-by-magnitude
  (`DevelopmentInfo.space_advantage` vs `groups.development.blended` — same kind of metric
  mismatch as king_exposed), center_control (no typed field anywhere).
- **Latent bug found during this audit**: the isolated/doubled-pawn loop (`concepts.rs:55-62`)
  hardcodes labels `"white"`/`"black"` to the `_us`/`_them` keys rather than using `us_color`/`them_color`
  like every other block in the function does. This is only correct when `side_to_move == white`;
  for black-to-move positions it likely mislabels which side the concept applies to. Worth fixing
  as part of the migration (typed `IsolatedPawn`/`DoubledPawn.color` sidesteps the bug entirely
  since it's a real color, not a relative `_us`/`_them` key).

All of the above is now done — see "Status: DONE" at the top of this section for what actually
landed (it diverged a bit from this original scope: `king_exposed`/`development` turned out to
already be scalar `GroupValue.blended` reads, not tag lookups, so they needed no migration;
`king_in_check` became a new typed `SensorReport.in_check` field rather than reusing
`PositionRecord.legal.is_check`, since `extract_concepts` only had a `SensorReport` in scope at
its call site inside `build_sensor_report`).

External test-position corpora (added 2026-07-28)

We were validating detectors against hand-picked canonical positions only
(`tests/motif_canonical.rs`) — no real, independently-authored positions to
check against. Researched chessprogramming.org's Test-Positions page and
picked the **Strategic Test Suite (STS)** by Dann Corbit & Swaminathan
Natarajan: 15 themed sub-suites, 100 positions each, including STS12 "Center
Control" and STS13 "Pawn Play in the Center" — directly on-theme for the
pawn_majority/center_control accuracy scoping above. Vendored via
`github.com/fsmosca/STS-Rating` (MIT-licensed repackaging; original authors'
site is dead, so redistribution rests on that MIT grant plus 15+ years of
open community reuse — reasonable confidence, not independently confirmed
with the original authors).

Not committed to the repo (`nu_plugin_chessdb/testdata/` is gitignored) —
`scripts/prep-test-data.nu` fetches it on demand, with full source/license
detail recorded in the script itself since the data directory it writes to
isn't tracked. `tests/sts_positional.rs` holds `#[ignore]`'d dev-only tests
that consume it (skip gracefully if the prep script hasn't been run; normal
`cargo test` never depends on this or hits the network). Run via
`cargo test --test sts_positional -- --ignored`.

Important caveat carried into the tests themselves: STS grades *move choice*
(bm + weighted alternatives), not concept presence/absence. It can't produce
per-position pass/fail assertions the way the hand-labeled canonical suite
does. What it can do is compare hit-rate on-theme vs. on an unrelated theme
(STS1 "Undermine" used as baseline) — if a concept doesn't fire more often on
positions themed around it than on unrelated ones, it has no discriminative
power regardless of how correct its board logic looks in isolation.

**STS calibration — first attempt was methodologically invalid (2026-07-28), retracted.**
Raw numbers observed: `center_control` fired on 67/100 STS12 (center-control-
themed) *pre-move* positions vs. 70/100 on an unrelated STS1 baseline;
`pawn_majority` fired on 68/100 STS13 pre-move positions vs. 79/100 baseline.
These were initially written up as "the detector has no discriminative
power" — that conclusion doesn't follow from this data and has been
retracted. STS positions are puzzles: the FEN given is deliberately the
*undecided* moment before the thematic move, not a position that already
embodies the theme (a "Center Control" puzzle exists because center control
is contested and up for grabs — that's what makes it a puzzle rather than a
foregone conclusion). Testing whether the raw pre-move FEN already trips the
detector tests the wrong thing; the near-baseline hit rate is unsurprising
and doesn't confirm or rule out a precision problem either way.

Also considered and rejected: comparing our concept's score after STS's
top-ranked move vs. its lower-ranked alternatives (using `c9`/`c8`). That
still isn't valid ground truth for *our* purposes — STS's move ranking is
itself an engine/human judgment call entangled with search and opponent
response, a fundamentally different and much harder problem ("what's the
best move here, accounting for everything") than what we're actually trying
to validate ("does this position have property X, definitively"). Building
a detector-accuracy test on top of someone else's best-move judgment just
tests whether we agree with their full-strength evaluation, not whether our
bitmask is right.

**Definitive ground truth — the actual methodology going forward.** Only
trust positions where the concept's presence/absence is unambiguous by
construction:
1. Hand-labeled canonical positions (chessprogramming.org-style, as already
   done in `tests/motif_canonical.rs`) — a human states "this position has
   a queenside pawn majority" or "does not," no engine judgment involved.
2. Positions *derived* from a known-clear starting position by pushing our
   own side's move(s) forward, with **no opponent-response search** — e.g.
   start from a labeled-clear position, play 1 (or at most 2) of our own
   candidate moves, and check the concept updates as expected. A 2-ply
   forward push is allowed only as a hint that a tactic might be present
   there — it is not itself an assertion, and anything it surfaces needs
   independent hand-verification before being trusted as a labeled fixture.

STS (and any future puzzle/engine-graded suite) stays limited to what it's
actually good for: a crash-safety smoke test (`sts_full_suite_evaluates_without_error`)
confirming `hugm-eval` doesn't panic across ~1500 real, diverse positions —
not a source of accuracy ground truth for `pawn_majority`/`center_control`.
Expanding the hand-labeled/derived corpus for those two concepts specifically
is the next real step, not yet done here.

Consistency pass: naming, duplication, documentation (2026-07-29)

Executed the plan scoped the same day (via Ultraplan remote refinement,
approved directly in-session since a browser wasn't available to click
approve on the web — approval happened as plain-text plan review instead).

Done:
- `eval/mod.rs` now carries a module-level doc laying out the full pipeline
  (`board` → `compute_groups`/`EvalGroups` → `build_sensor_report`/`SensorReport`
  → `extract_concepts`/`Concept` → `rank_issues_for_position`/`GatedIssue`) and
  explains why `detect_X`+`X_to_typed` (cached, tactical) and `extract_X`
  (single-step, positional/material) coexist as two naming families —
  deliberate, not sloppy.
- `render_explanations`/`render_structured_explanations` (`position.rs`)
  rewritten to read `record.sensor_report` instead of independently
  re-deriving from `groups.*.terms` — the third/fourth large `.terms`
  consumer (after `extract_concepts`, fixed last session) is gone. Also fixed
  a real bug found while migrating: `render_structured_explanations`
  hardcoded `"side": "white"` on every emitted explanation regardless of
  which side was actually to move — now uses the real color. Four fields
  with no typed `SensorReport` home (`tropism_us`, `doubled_rooks`,
  `development_diff`, `initiative`) remain narrow, documented exceptions —
  reading `.terms` for four specific whole-position/legacy scores that
  aren't per-concept, not a regression back to the old pattern.
- `coach_derive_cmd.rs`'s state_id bit layout: `concepts.rs` now has
  `decode_state_id(sid: u16) -> StateVector` next to `encode_state`, sharing
  one set of `BIT_*` constants so pack/unpack can't drift apart. The
  previously-independent hand-decoded "fast path" in `coach_derive_cmd.rs`
  now calls `decode_state_id`. `encode_move_states` returns `Vec<StateVector>`
  (typed) instead of `Vec<Value>`; `compute_baselines`/`detect_anomalies`/
  `compute_transitions` read typed fields directly; conversion to `Value`
  happens once, in `format_results`, via one `state_vector_to_value` helper
  (external field names — `phase_bucket`, `has_fork`, etc. — kept identical,
  since `chessdb/sync.nu`/`chessdb/profile.nu` depend on them by name).
  Added `fast_path_and_slow_path_agree_on_state_id` regression test.
- `hugm_eval_cmd.rs`'s single-FEN and list-of-FEN branches deduped into one
  `build_output_value` helper. Found real duplicate *work*, not just
  duplicate code: both branches recomputed `gated_issues` via
  `extract_concepts`+`rank_issues_for_position` even though
  `record.sensor_report.gated_issues` already has it (computed inside
  `build_sensor_report` with the same `player_elo`). Now just copied from
  there — verified byte-identical to the old recompute before switching.
- `core.rs`: the two inlined `pos.zobrist_hash(EnPassantMode::Legal)` +
  manual hex-format call sites (`GameVisitor::san`, `pgn_to_batch_record`)
  now call the existing `get_canonical_hash` helper instead of re-deriving.

Concept/StateVector simplification — scoped and implemented 2026-07-29

User's framing: after the consistency pass, "a simpler expression of functions
is struggling to get out... as if the bitmaps and discovery of the points
could be consolidated and smoothed." Went concept-by-concept through all 20
named concepts in `extract_concepts` to see what would actually simplify vs.
what would just relocate complexity.

**Concept inventory (why a full generic registry doesn't pay for itself):**
- 8 are a uniform shape — count `Vec<T>` entries matching a color, severity =
  count × fixed weight: fork, pin, skewer, discovered_attack, hanging_piece,
  isolated_pawn, outpost, passed_pawn.
- 2 look like that shape but sum a `.count` field per entry instead of
  counting entries: doubled_pawn, rook_on_seventh.
- 1 has an extra predicate beyond color: rook_open_file (`color == us_color
  && rook_count > 0`).
- 2 fit a different clean shape — single `Option<T>` with `{color,
  strength}`: minority_attack, center_control.
- 5 are pure scalar arithmetic, no `Vec<T>` involved at all: material_imbalance,
  bishop_pair (two bools, not a Vec), king_in_check, king_exposed, development.
- **Decision needed**: `pawn_break` and `rook_on_seventh` only ever check
  `us_color`, never `them_color`, unlike all 8 siblings in the first bucket.
  No principled reason found for the asymmetry — looks like an oversight.
  Recommend making them symmetric (check both sides, matching every other
  concept's pattern) but flagging it here since it's a behavior change
  (previously-silent opponent-side pawn-break/rook-on-7th concepts would
  start firing), not pure refactor.

Given that spread, a single generic table covering all 20 would need
per-entry closures for the count/sum/predicate variation anyway — in Rust,
without macros, that's about as much code as today's inline blocks, just
relocated into a table's closure fields. Not pursuing a full registry;
instead, three narrow, verified-in-place simplifications:

**1. `encode_state`'s boolean-flag packing.** Re-checked and this isn't
*fully* uniform either (`has_fork` is an OR of two conditions, `king_exposed`
maps through an `Option`) — a table of `(bit, fn(&SensorReport) -> bool)`
pairs handles that fine since each closure can differ; the win is
structural, not textual: bit position and check land in the same tuple, so
they can't be paired wrong the way two separately-ordered lists could drift.
Bigger win found while designing this: `encode_state` can call its own
`decode_state_id` to build the returned `StateVector`'s named fields, instead
of hand-writing ten `let has_x = ...` bindings that duplicate what decoding
the just-packed bits would already tell you. Concretely:
```rust
let mut id: u16 = 0;
id |= (phase_bits as u16 & 0x3) << BIT_PHASE;
id |= ((material_sign + 2) as u16 & 0x7) << BIT_MATERIAL_SIGN;
for &(bit, check) in BOOL_BITS { if check(sensor) { id |= 1 << bit; } }
decode_state_id(id)
```
This makes packing and unpacking mutually verifying by construction — if
`decode_state_id` ever had a bug, `encode_state`'s own output would
immediately look wrong too, since it's built by calling the same decoder.
Roughly 35 lines → ~10. Adding a new bit becomes "one row in `BOOL_BITS`,"
not "one `let`, one bit constant, one pack line, one unpack line, one struct
field" scattered across the function.

**2. `tier_for_concept` — correction, not the plan from last message.**
Checked every caller in the repo (Rust and Nu): `tier_for_concept(name: &str)`
is **dead code**, never called anywhere. The live chaos-attenuation system
(`SensorTier`/`attenuation`) is entirely separate and operates at the
`EvalGroups` *group* level inside `position.rs`'s `compute_aggregates`
(hardcoding `SensorTier::Positional`/`Strategic` per score group — pawn
structure, king safety, etc.), not per individual `Concept`. So "move tier
onto `Concept` at construction" would be adding new, currently-unused
plumbing, not fixing a live re-discovery bug — there's no live bug here.
Corrected recommendation: delete `tier_for_concept` as dead code (same
disposition as `RankedConcept` below), and leave the working group-level
attenuation system untouched. If per-concept chaos attenuation for
`GatedIssue` scoring is ever wanted, that's new functionality to design
separately — not part of this cleanup.

**3. A small local helper for the 8 uniform concepts** (not a registry) —
collapses each concept's current 2 lines (us-side + them-side count-and-push)
into 1 call:
```rust
fn count_and_push_by_color<T>(
    concepts: &mut Vec<Concept>, items: &[T], color_of: impl Fn(&T) -> &str,
    us_color: &str, them_color: &str, name: &str, weight: i64, elo_min: i32,
    phrase: impl Fn(&str, i64) -> String,
)
```
called once per concept for `us_color` and once for `them_color` internally.
Cuts roughly 32 lines to 8 call sites across fork/pin/skewer/
discovered_attack/hanging_piece/isolated_pawn/outpost/passed_pawn.
doubled_pawn/rook_on_seventh (sum-based) and rook_open_file (extra predicate)
stay hand-written — forcing them through this helper would need extra knobs
that erase the simplification.

**Also found**: `concept_types.rs`'s `RankedConcept` struct is dead code —
never constructed anywhere, fully superseded by `Concept`/`GatedIssue`. Delete.

All four items implemented as scoped, plus a small clippy cleanup along the
way (`#[allow(clippy::too_many_arguments)]` on `count_and_push_by_color`,
9 args; a `SensorPredicate` type alias for `BOOL_BITS`'s function-pointer
tuple, which clippy flagged as too complex inline).

Verified properly, not just compiled: captured `extract_concepts` +
`encode_state` output on 4 canonical FENs (the fork/tactical position,
starting position, an Italian-opening middlegame, and the queenside-majority
position from earlier sessions) *before* touching anything, then diffed
against the same run after all four changes landed. The diff was exactly
one line — `pawn_break` now also firing for the opponent side on the fork
position, precisely the deliberate symmetry fix — with every `state_id`
byte-identical across all 4 positions, confirming the `encode_state`/
`decode_state_id` restructuring is fully behavior-preserving. Full `cargo
test` (27 passing) and `cargo clippy --tests` (no new warnings beyond one
pre-existing `sort_by_key` suggestion, unrelated to this pass) stayed clean
throughout.

Follow-up fixed (2026-07-29): `chessdb/sync.nu`/`chessdb/profile.nu`'s SQL-side
state_id bit-shift duplication

Closed the flagged follow-up from the pass above. `chessdb/db.nu`'s
`move_states` table gained `has_outpost`, `has_open_file`, `has_passed_pawn`
columns (bits 10/11/12, matching `concepts.rs`'s `BIT_*` constants) — the
real gap, since those three concepts had no column at all before, forcing
`profile.nu:342-345` to bit-shift `state_id` directly. Added the matching
`ALTER TABLE`/backfill migration in `init-db` (guarded by `IS NULL` so it's
cheap after the first run on an existing DB) and extended `sync.nu`'s
`INSERT OR IGNORE` to populate the three new columns for new rows going
forward. `profile.nu:174-176,185-187` (`tactical-win-impact`) and
`profile.nu:342-345` (`position-win-rates`) now read `ms.has_fork`/
`ms.has_pin`/`ms.has_hanging`/`ms.has_outpost`/`ms.has_open_file`/
`ms.has_passed_pawn`/`ms.king_exposed` directly instead of re-deriving any
of them from `ms.state_id` — `sync.nu`'s INSERT is now the only place on the
Nu/SQL side that decodes the bit layout, mirroring `decode_state_id` as the
one decode point on the Rust side (the two can't be unified across the
language boundary, but at least each language now has exactly one).

Verified against a real SQLite DB, not just `cargo check`: built a
pre-migration `move_states` table missing the three columns, seeded a row
with `state_id` bits 7/10/11/12 set, ran `chess-init` and confirmed the
backfill produced the correct decoded values, then ran
`chess-profile-tactical`/`chess-profile-position` end-to-end against that
DB and confirmed correct, error-free output (`fork` correctly attributed,
`outpost`/`open_file`/`passed_pawn` all `present: 1`, `king_exposed`
correctly `0`). Also confirmed a fresh `chess-init` produces the new columns
directly via `CREATE TABLE` (no migration path needed).

Verification: `cargo check`/`cargo clippy --tests` clean (only pre-existing
warnings remain, in untouched `src/bin/hugm_harness.rs`); full test suite
27 passing (was 26 — the new fast/slow-path regression test), 0 failed, 1
ignored (`sts_positional`, unaffected); manually confirmed
`render_explanations`/`render_structured_explanations` produce sane,
non-crashing output on real positions and `hugm_eval_cmd.rs`'s reused
`gated_issues` is identical to a fresh recompute before switching.

Known Bugs (last reviewed 2026-07-28)

RESOLVED:
- BUG-1: FIXED — added `board_pieces TEXT` to positions DDL + included in import-records SELECT
- BUG-2: ALREADY FIXED — query uses `m.game_id = g.game_id` correctly
- BUG-3: ALREADY FIXED — played_at extraction from end_time/lastMoveAt/createdAt exists
- BUG-4: FIXED — `process_corpus.rs` now uses `rayon::par_iter` for HUGM eval (`src/process_corpus.rs:224-226`)
- BUG-5: RESOLVED — `--with-stockfish` flag removed entirely; Stockfish labeling is a separate pipeline
- BUG-6: RESOLVED — `src/bin/sf_batch_eval.rs` is now a 3-line stub that points to the external
  labeling pipeline in NNUE_AUDIT.md; the hardcoded-path inconsistency it had with `nnue-eval`'s
  `STOCKFISH_BIN` no longer exists because there's no live path to be inconsistent with.
- BUG-7: FIXED — critter_eval_cmd.rs deleted; use `chessdb hugm-eval` instead
- BUG-8: ALREADY FIXED — help text uses "chessdb.nu"
- BUG-9: FIXED (2026-07-28) — `extract_concepts` queried nonexistent `groups.*.terms` keys for
  `rook_open_file`/`rook_seventh`/`passed_pawn` (real keys were unsuffixed/differently named);
  those concepts never fired in `gated_issues`. Fixed by the terms→typed migration above.
- BUG-10: FIXED (2026-07-28) — `detect_discovered` had no defended/material-significance check,
  so it flagged 3 false-positive "discovered attacks" per side on the plain starting position.
  Fixed in `position.rs::detect_discovered`; regression test in `motif_canonical.rs`.
- BUG-11: FIXED (2026-07-29) — `process_corpus.rs` / `nnue_eval_cmd.rs` module-boundary bleed.
  Extracted chess.com/lichess game parsing (ECO/opening scraping, 4-format timestamp
  normalization, result-relative-to-username logic) into `src/game_parse.rs::parse_game`
  — 14 new unit tests, including one pinning down a subtle pre-existing behavior (a
  present-but-invalid `end_time` commits to "unknown" rather than falling through to
  `lastMoveAt`/`createdAt`; preserved, not fixed, since that wasn't this task's job).
  Extracted the raw UCI/Stockfish handshake into `src/stockfish.rs::StockfishEngine`
  (spawn/handshake/`eval_fen`, `Drop` sends `quit`) — 4 new unit tests for eval-line
  parsing that had zero coverage before (buried inside `PluginCommand::run`). Both
  commands' `run()` now just parse input, call the extracted logic, and build `Value`s
  — actual wiring, not business logic. Verified beyond compiling: ran a realistic
  chess.com-style game object through `parse_game` directly and confirmed every field
  (game_id, source, result-relative-to-username, played_at, eco/opening) matched
  expectations; full test suite 20→34 passing.

Canonical position identity (tablebase-style dedup) — scoped and
implemented 2026-07-29

Follow-on from board normalization, at the user's request: instead of
normalizing only *ephemerally* inside one evaluation call, make
`positions.zobrist`/`positions.fen` themselves canonical (White-always-to-
move) — the same technique endgame tablebases use to collapse color-mirror
positions into one stored entry. Clarified first that this is *not* needed
for sign-correctness (already solved by board normalization — `hugm_score`/
`state_id` are already proven mover-relative and identical across mirrors,
verified directly in that work). The only remaining, distinct benefit is
deduplication: two different real games reaching exact color-mirror
positions currently get evaluated and stored twice.

**Key simplification found while scoping**: no new "was this occurrence
flipped" tracking column is needed anywhere. It's already fully derivable
from `moves.color` (already stored) plus ply alternation — for any
`moves` row, the position *before* the move (`position_id`) was flipped iff
`color == "black"`, and the position *after* it (`next_position_id`) was
flipped iff `color == "white"` (verified against `core.rs`'s actual ply/color
assignment in `GameVisitor::san`). Chess alternation makes this free.

**Resolved by asking, not deciding unilaterally**: `chess-explore` aggregates
`moves.san` (real, as-played notation) per position. If position identity
becomes canonical, a canonical position reached from real games via *either*
color's perspective would otherwise mix White's-frame SAN ("e4", "Nf3") with
Black's-frame SAN ("...e5", "...Nf6") under one grouped result — nonsense to
a human. User chose: **translate SAN to the canonical frame** for every
move, not just accept the mixed output or keep moves real-only. Confirmed
this is buildable: `shakmaty::Move`'s variants (`Normal`/`EnPassant`/
`Castle`/`Put`) reference only `Square`/`Role` fields, no color at all — so
translating a move to canonical frame is purely flipping its squares via
`Square::flip_vertical()` (files unaffected), then `SanPlus::from_move(pos,
&move)` regenerates correct notation against the canonical position.

**What changes:**
1. **New shared module** (not yet named — candidate: `src/canonical.rs`)
   housing the normalize-to-white-to-move logic, used by *both*
   `core.rs` (needs it per-ply, for both the pre-move and post-move
   position, to support SAN translation) and `eval::position.rs`'s existing
   `normalize_for_eval` (which should call into this shared version rather
   than duplicate it — `core.rs` is the more foundational layer here, so the
   dependency should run eval → core/canonical, not the other way).
2. **`core.rs::GameVisitor::san`** (`src/core.rs:150-200`): currently computes
   `fen`/`zobrist` from `new_pos` (the real post-move position) directly.
   Needs to: normalize `self.pos` (pre-move) and `new_pos` (post-move) each
   to canonical form; if `self.pos` needed flipping, flip `mv` (the move
   just played) via a new `flip_move(&Move) -> Move` helper before calling
   `SanPlus::from_move` for the *stored* SAN text; store the canonical
   `fen`/`zobrist` (not real) as this row's position identity.
3. **`process_corpus.rs`**: the hardcoded initial-position zobrist
   (`"463b96181691fc9c"`, `position.rs`/`process_corpus.rs`) is already
   White-to-move — unaffected. `board_pieces` (derived from `m_row.fen`)
   needs to derive from the canonical fen for consistency, once `fen` is
   canonical.
4. **Evaluation itself (`analyze_fen_with_engine_score`) needs no change in
   logic** — feeding it an already-canonical FEN just means
   `normalize_for_eval` is a no-op (`was_flipped: false` always), and
   `hugm_score`/`hugm_eval_arr`/`state_id` come out identical to today's
   values either way (already proven mover-relative/orientation-invariant).
   Confirmed `process_corpus.rs` never persists any square/color-bearing
   `SensorReport` detail (only the scalar `hugm_score`/`hugm_eval_arr`/
   `state_id`/`mate_in_1`/`is_checkmate` — checked `PendingPos`'s fields
   directly), so there's no square-level correctness concern for the stored
   pipeline from evaluating the canonical FEN instead of the real one.
5. **Naming cleanup opportunity, found while reading this code**:
   `core.rs::get_canonical_hash` is *already* named "canonical" but
   currently just means "the real position's hash" — after this change it
   would need to become genuinely canonical, or the name should be
   reconsidered if a real (non-canonical) hash is still needed somewhere.

**Consequence — bigger than prior migrations, flagging clearly**: this is
*not* a "safe to re-run chess-derive" refresh like BUG-13's or BUG-12's.
It changes what `positions.zobrist`/`positions.fen` *mean* — existing rows
were built under the real-position convention. Any existing database needs
`games`/`positions`/`moves`/`move_states` wiped and fully rebuilt via
`chess-sync` from scratch (re-fetching from chess.com), not just a targeted
re-derive. Worth confirming there's no other data (e.g. consumed anomaly/
baseline history) the user cares about preserving before this runs.

**Not yet resolved, needs a decision during implementation**:
`chess-explore`'s `--params [$zobrist]` input — once zobrist is canonical,
does the caller need to canonicalize their own lookup zobrist first (e.g.
if given a real FEN/zobrist from elsewhere), or should `chess-explore`
accept a real FEN and canonicalize it internally before querying? Leaning
toward the latter for usability, not decided.

**Implemented (2026-07-29), confirmed via "yes, go ahead":**
1. `src/canonical.rs` — new shared module: `normalize_to_white_to_move`
   (extracted verbatim from `eval::position::normalize_for_eval`'s old
   inline logic — same bitboard/`Setup` rebuild, now the single copy),
   `unflip_square`, and `flip_move` (pure square-flip on `shakmaty::Move`'s
   `Normal`/`EnPassant`/`Castle`/`Put` variants — no color field to touch).
   `eval::position::normalize_for_eval`/`unflip_square` are now thin
   wrappers delegating here — no behavior change, confirmed by the full
   test suite passing unchanged after the extraction.
2. `core.rs::GameVisitor::san` (the function both `pgn_to_fens`, used by
   `process_corpus.rs`'s real sync pipeline, and `pgn_to_batch_record`
   share): now stores `fen`/`zobrist` from
   `canonical::normalize_to_white_to_move(&new_pos)` instead of the real
   `new_pos`. When the pre-move position had Black to move, `san` is
   regenerated via `canonical::flip_move(&mv)` +
   `shakmaty::san::SanPlus::from_move(canonical_pre_pos, &flipped_mv)`
   rather than the real SAN text — `uci` is left untouched (real terms;
   confirmed nothing in `chessdb/*.nu` reads `moves.uci`, only inserts it).
   Note: had to qualify as `shakmaty::san::SanPlus` rather than the
   `pgn_reader`-re-exported `SanPlus` already imported at the top of the
   file — `pgn-reader = "0.24"` pins its own `shakmaty = "0.25"` internally,
   a different version than this crate's direct `shakmaty = "0.26"`
   dependency, so `pgn_reader::SanPlus::from_move` rejects our (0.26)
   `Chess`/`Move` types at the type level. `shakmaty` 0.26 has its own
   `san::SanPlus` with an identical `from_move` — used that instead.
3. `process_corpus.rs`'s `board_pieces` (derived from `m_row.fen`) needed
   no change — it automatically became canonical-consistent once `fen`
   itself is canonical, confirmed by re-reading (just a prefix scan over
   the FEN string, orientation-agnostic).
4. Verification (`tests/canonical_identity.rs`, 3 new tests, all
   hand-derived independently of `canonical.rs`'s own implementation):
   - `1. Nf3` (White's move from a White-to-move real position): stored
     `fen`/`zobrist` are the canonical mirror
     (`rnbqkb1r/pppppppp/5n2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 1 1`); `san`
     stays `"Nf3"` (pre-move position was already White-to-move, so real =
     canonical trivially); `uci` stays real (`g1f3`).
   - `1. Nf3 Nf6` (Black's reply, played from a real Black-to-move
     position): `san` is translated to `"Nf3"` (not the real `"Nf6"`) —
     Black's g8-f6 mirrors onto canonical White's g1-f3; `uci` stays real
     (`g8f6`); the real resulting position is White-to-move again, so
     `fen`/`zobrist` pass through unflipped.
   - Cross-checked `pgn_to_fens`'s stored zobrist for `1. Nf3` against an
     *independently* parsed-and-normalized mirror FEN (not derived via
     `pgn_to_fens` at all) — hashes matched, demonstrating the actual dedup
     mechanism: two different sources reaching color-mirror-image real
     positions converge on one identity.
   - Full test suite green throughout (41 tests across lib + integration
     files, up from 37 before this feature), `cargo clippy --all-targets`
     clean on every touched file, STS smoke test
     (`cargo test --test sts_positional -- --ignored`) still passes.
5. **`get_canonical_hash` naming resolved**: it now genuinely computes a
   canonical hash at its call site in `GameVisitor::san` (called on
   `canonical_pos`, not `new_pos`) — the name is accurate there. Its other
   two call sites (`ScanVisitor::san`, `pgn_to_batch_record`'s
   initial-position hash) were **not** touched — see below.

**Found but explicitly deferred (not fixed this pass):** `core.rs` has a
second, separate PGN-replay visitor — `ScanVisitor`/`scan_pgn`, backing the
`PgnScan` plugin command — with its own `san` method that calls
`get_canonical_hash` on real (non-canonicalized) positions, same latent
mislabeling `GameVisitor::san` had before this fix. Confirmed via
`grep -rl` across `chessdb/*.nu` and this crate that `PgnScan`/`scan_pgn`/
`ScanGameRow`/`ScanMoveRow` have **zero consumers** anywhere (not wired
into the Nu sync pipeline, no tests, no docs) — it's a dead/experimental
command. Left as-is to avoid unverified changes to unused code in this
pass; worth fixing for consistency if `PgnScan` is ever put to use.

**Not yet resolved — real loose end, needs a decision, not blocking this
migration:** `chess-explore`'s `zobrist: string` parameter (`chessdb/
sync.nu:139`) is looked up directly against `moves.position_id`. Now that
stored zobrists are canonical, a zobrist the user computes independently
from a Black-to-move FEN (e.g. via the existing `chessdb zobrist <fen>`
plugin command, which hashes the *real*, non-canonical position — left
alone per the original consolidation note, since it's a general-purpose
CLI command, not implicitly tied to the `positions` table's convention)
will not match. No Nu-side command currently exposes "canonicalize this
FEN first." Options: teach `chess-explore` to accept a FEN and canonicalize
internally (better usability, needs a new plugin command or Nu-side
port of the transform), or document that `chess-explore` only accepts
zobrists already sourced from this database (`positions`/`moves` query
results, which are self-consistently canonical). Deferred until it
actually blocks someone.

**Follow-on: player-perspective position familiarity (2026-07-29)** —
pushed the canonical-identity model onto `chess-explore` (`chessdb/
sync.nu`): since a canonical position collapses both sides' real
occurrences onto one row, "how many times has this position come up"
only means something once you say *whose turn* it was. Added an optional
`--username` flag: `moves.position_id` for a given zobrist is joined
against `games`/`moves.color` the same way `position-win-rates`'s
`player_color` CTE already does, split into `times_to_play` (occurrences
where `--username` was the one to move there) and `times_to_wait`
(occurrences, within that player's own games, where the opponent was to
move instead). Without `--username`, output is unchanged (the original
plain san/times_played/avg_elo table) — verified both call shapes plus
the split itself against a hand-built two-row throwaway SQLite db (one
move where the profiled player is to move, one where the opponent is)
via direct `nu -c` module invocation, since this is pure SQL with no
plugin dependency.

**Top-down audit (2026-07-30), at the user's request ("push this identity
onto everything else... needs full implementation/audit")** — went back
through every Nu-side consumer of `positions.fen`/`moves.san` to check
whether canonicalization broke an assumption of real (non-canonical)
orientation. Found two real, concrete regressions, both fixed:

1. **`chess-review`'s human-facing move display was showing the wrong
   notation.** The original canonical-identity implementation overwrote
   `moves.san` itself with the canonical-frame translation (needed for
   `chess-explore`'s cross-game grouping) — but `review-game`
   (`chessdb/sync.nu`) selects `m.san` to show a human the actual moves of
   one specific real game, and this directly contradicts the user's
   original design principle for this whole feature ("the moves and game
   retains w/b p/o relative to who is to move"). A player who played
   1...Nf6 would see "Nf3" in their own game review — indistinguishable
   from a bug. Fixed by splitting into two columns: `core.rs::MoveRow`
   gained `canonical_san` alongside the existing `san`; `GameVisitor::san`
   now always keeps `san` as the real, as-played SAN, and computes
   `canonical_san` separately (translated via the same
   `flip_move`/`SanPlus::from_move` logic, only when the pre-move position
   had Black to move). Plumbed through `process_corpus.rs`,
   `pgn_to_fens.rs`'s `move_rows_value`, the `moves` table schema
   (`canonical_san TEXT`, migration-safe `ALTER TABLE`), and `sync.nu`'s
   `db-merge` column list. `chess-explore` now groups by `m.canonical_san`
   (aliased back to `san` in its output for shape compatibility) instead of
   `m.san` — restoring the property that was the entire point of
   translating SAN in the first place, which the original implementation
   had accidentally achieved by breaking `chess-review` instead of by
   adding a dedicated column. Verified: `tests/canonical_identity.rs` now
   asserts `1. Nf3 Nf6`'s Black-move row has `san == "Nf6"` (real) and
   `canonical_san == "Nf3"` (translated); a hand-built two-game throwaway
   SQLite db confirmed `chess-explore` collapses both games' occurrences
   into one `canonical_san` row while `chess-review` on the same data shows
   the real per-game SAN (`Nf6`, not `Nf3`).
2. **ECO opening classification (`enrich-openings`, `db.nu`) silently
   breaks for roughly half of all named openings.** It joins
   `positions.fen` (now canonical) against `openings.fen` (seeded verbatim
   from JeffML/eco.json's real, non-canonical FENs — ECO entries are
   recorded at whatever ply/side they were actually reached, odd or even).
   Any opening keyed at a Black-to-move ply would never match again.
   Fixed at the seed boundary, not the join: added a new plugin command,
   `chessdb canonicalize-fen` (`src/canonicalize_fen_cmd.rs`, backed by
   `core::canonicalize_fen`, reusing `canonical::normalize_to_white_to_move`
   — mirrors `zobrist.rs`'s string-or-list shape), and `fetch-and-seed-eco`
   now runs every downloaded ECO FEN through one batched
   `chessdb canonicalize-fen` call before storing (one plugin call for the
   whole dataset, not one per row — `enumerate`+`upsert` reassembles the
   rows with their canonical `fen`). `enrich-openings`'s join itself is
   unchanged — both sides are canonical now, so `p.fen = o.fen` is
   meaningful again. Existing `openings` tables must be rebuilt via
   `chess-seed-openings` (already deletes+reseeds) once this ships.
   Verified: new test `canonicalize_fen_matches_pgn_to_fens_for_the_same_
   position` confirms the plugin command's output agrees with what
   `pgn_to_fens` independently stores for the same real position, plus a
   no-op check for already-White-to-move input; the `db.nu` batching logic
   (`enumerate`/`upsert` reassembly) was checked separately with a mocked
   `chessdb canonicalize-fen` (the real plugin can't be loaded in this
   sandbox's Nu 0.114 — built for 0.111 — so this is a logic-only check,
   same limitation noted earlier in this session).
3. **Checked and found NOT broken, for completeness**: `chessdb
   derive-coach-signals`' FEN-reparsing fallback path (`coach_derive_cmd.rs`,
   used when a row lacks a precomputed `state_id`) re-derives `state_id`
   from `p.fen` directly with no normalization step of its own — but since
   `p.fen` is now already canonical, this fallback gets the needed
   normalization for free, and its only output (`state_id`) was already
   established (board-normalization work, above) to be orientation-
   invariant. `derive.nu`'s query passing `p.fen` through was safe
   unchanged.

**Migration required for any existing database** (was already flagged
before implementation, repeating here since it's now actionable): existing
`games`/`positions`/`moves`/`move_states` rows were built under the old
real-position convention and are stale under the new canonical one. Wipe
those tables and fully re-run `chess-sync` from chess.com before relying on
`positions.zobrist` for lookups/dedup — a targeted re-derive (like BUG-12/
BUG-13's chess-derive refresh) is not sufficient here, since position
*identity* itself changed, not just a derived signal. Also run
`chess-seed-openings` (deletes+reseeds `openings`, then re-enriches
`games.eco`/`.opening`) — the old `openings` rows are real, non-canonical
FENs from before the `fetch-and-seed-eco` fix above and will not match.

Board-normalization — scoped and implemented (2026-07-29)

User's proposal, refined over the course of the color/perspective audit below:
instead of threading a `color`/`us`/`them` parameter through every scoring
function (the fix I'd proposed for BUG-13 alone), normalize the *board* once
— transform any position so White is always the side to move (swap piece
colors, mirror ranks, swap castling rights, mirror the en passant square) —
before evaluation, and let every scoring function just always compute
White-minus-Black, the way `material_score` already does today. Un-flip only
at the one boundary that produces human-readable output (squares/colors in
`SensorReport`).

**Key finding that changes the size estimate**: `compute_groups` already
does `let us = chess.turn(); let them = us.other();` and every scoring
function (`material_score`, `pawn_structure_score`, `king_safety_score`,
`piece_activity_score`, `tactical_score`, `detect_pins`/`forks`/`skewers`/
`discovered`/`outposts`, `center_control_score`, `piece_coordination_score`,
`tactical_pressure_score`, `vector_features_score`, `development_score`,
`development_space_score`, `draw_weight`, `king_tropism_score` — 22 functions
total, verified via `grep -n "fn \w*(.*color: Color" position.rs`) already
takes a `color: Color` parameter. **None of these 22 functions need to
change at all.** If the `Chess`/`Board` fed into `compute_groups` is already
normalized, `chess.turn()` naturally evaluates to White, and every existing
function call already does exactly the right thing — the ~25 scattered
`if color.is_white() { .. } else { .. }` branches throughout `position.rs`
(pawn advance direction, king-shield ranks, promotion distance, 7th-rank
rook detection, etc. — also verified by grep) all collapse to their White
branch, correctly, with zero code changes. **The entire ~3400-line scoring
body of `position.rs` is untouched.** This is a much smaller, lower-risk
change than it first appeared, concentrated entirely at the boundaries:

1. **New: a normalize step.** `fn normalize_for_eval(chess: Chess) -> (Chess, bool /* was_flipped */)`
   — if `chess.turn() == Black`: mirror the board vertically (`shakmaty::Board::flip_vertical`,
   library-provided) *and* swap each piece's color (shakmaty has no single
   call for this — `Color::flip`/`ByColor::flip` exist for swapping a pair of
   values, but not a "recolor every piece on a board" convenience; this is
   new, hand-written code, iterate pieces and rebuild), swap castling
   rights White↔Black, mirror the en passant square's rank if present, set
   side-to-move to White. Called once, at the top of
   `analyze_fen_with_engine_score`, before `compute_groups`/`build_sensor_report`.
2. **New: an unflip step**, applied to exactly the fields that carry
   real-board coordinates/colors — concentrated almost entirely in `PieceRef`
   (`square` needs `Square::flip_vertical()` — again library-provided —
   `color` needs `Color::other()`), applied once to every struct containing
   a `PieceRef` (`Fork`, `Pin`, `Skewer`, `DiscoveredAttack`, `HangingPiece`,
   `Outpost`, `DevelopmentInfo.undeveloped_pieces`) plus the few standalone
   `square`/`color` fields (`PassedPawn`, `IsolatedPawn`, `PawnBreak`).
   `OpenFile`/`DoubledPawn`/`PawnIsland` (file-letter only) need no change —
   vertical-only flip doesn't touch files. `threat_graph.rs`'s
   `EvaluatedFork`/`HangingPiece` (also `PieceRef`-bearing, also has its own
   `is_white()` labeling sites) needs the same treatment — verified it has
   the identical pattern.
3. **Unchanged, verified**: `EvalGroups`/`.terms` structure, `extract_concepts`,
   `encode_state`/`decode_state_id`, `Concept`/`GatedIssue`, all of
   `chessdb/*.nu` — everything downstream of `SensorReport` already consumes
   correctly-shaped output and needs zero changes, since the external
   contract (what "us"/"them"/`side` mean) doesn't change, only how it's
   computed internally.
4. **Side effect, not a separate task**: this fixes BUG-13 for free.
   `material_score` doesn't need the color-parameter fix I proposed earlier
   at all — on a pre-normalized board it already computes White(=real
   mover)-minus-Black(=real waiter) directly, automatically consistent with
   the other 8 `us`-relative components. No separate patch needed.
5. **Does NOT fix BUG-14** (StateVector's either-side concept bits) — that's
   a `state_id` bitfield design question, orthogonal to board orientation;
   `sensor.tactical.forks` still combines both colors' forks into one `Vec`
   regardless of whether the board was normalized first.

**Resolved while scoping** (was an open question, checked directly rather
than left assumed):
- `LegalInfo` (`is_check`/`is_checkmate`/`is_stalemate`/`legal_move_count`)
  and `mate_in_1_exists` are boolean/scalar and color-symmetric — plan to
  compute these from the *original*, unnormalized `Chess` to avoid any risk,
  rather than assume they're safe on the flipped one.
- `Checks{sum_groups, matches_final, delta}` compares `final_score` against
  a caller-supplied `engine_score`. Checked every caller: `process_corpus.rs`
  (the actual ingestion pipeline) always passes `None`; only
  `hugm_eval_cmd.rs`'s optional `--engine-score` CLI flag can supply a real
  value, and nothing in `chessdb/*.nu` ever sets it — it's a manual, ad-hoc
  comparison a human can invoke directly, not part of any automated or
  stored pipeline. Low risk, confirmed rather than assumed.

**Consequence (expected, not a regression)**: `positions.hugm_score`/`state_id`
get different (corrected) values for any previously-Black-to-move position
once BUG-13's fix lands as a side effect — same "safe to re-run" data
migration already noted for BUG-13, via `chess-sync`/`chess-derive`.

**Verification plan when implemented**: full existing test suite should
pass unchanged (any hardcoded expected `hugm_score`/material values for a
Black-to-move position that don't survive are exactly BUG-13 being fixed,
not a regression — verify each such diff by hand). New tests needed: (a) a
score-sign test mirroring `pawn_break_color_is_invariant_to_side_to_move`
but for `final_score`/material, (b) critically, a real square/color
correctness test — take a genuine Black-to-move tactical position, confirm
a detected fork/pin reports the *actual* board square and color, not a
flipped one (this is the one new failure mode this whole change introduces,
so it needs its own explicit test, not just an invariance check).

Implemented as scoped, with one real bug found and fixed during verification
that the scope didn't anticipate:

- `normalize_for_eval` built via `shakmaty::Setup`/`ByRole`/`ByColor`/
  `Board::from_bitboards`/`Chess::from_setup` (no direct "swap a whole
  position's colors" API exists in shakmaty, as expected from scoping —
  built it from `Board::by_role`/`by_color` bitboard extraction +
  `Bitboard::flip_vertical` on each, combined with a `ByColor` swap in one
  pass). `unflip_square`/`unflip_color`/`unflip_piece_ref`/
  `unflip_sensor_report` cover every color/square-bearing field across
  `SensorReport` — this ended up larger than the scope's "concentrated
  almost entirely in `PieceRef`" estimate: `MaterialBalance` (white/black
  `PieceCounts` + `bishop_pair_white`/`black`), and seven more structs with
  a bare `color` field but no square at all (`OpenFile`, `DoubledPawn`,
  `PawnIsland`, `MinorityAttack`, `PawnMajority`, `RookOnSeventh`,
  `CenterControl`, `KingExposure`, `DevelopmentInfo`) all needed the swap
  too — verified against every struct in `concept_types.rs`/`sensor.rs`
  directly rather than assuming the scope's estimate was complete.
- **Bug found during verification, not anticipated in scoping**:
  `GatedIssue.side` gets correctly un-flipped, but `GatedIssue.phrase` is
  free text built with literal "White"/"Black"/"white"/"black" words baked
  in by `format!` calls *inside* `build_sensor_report`, before the un-flip
  pass runs — so `side` and `phrase` disagreed (e.g. `side: "white"`,
  `phrase: "Black is up 207 centipawns..."`) until a dedicated
  `unflip_phrase` word-swap (sentinel-based, to avoid double-flipping a
  phrase containing both words) was added. Caught by an independent,
  hand-verified test (see below), not by the implementation itself —
  exactly the kind of thing that's easy to miss when the structured field
  looks right and only the free text is wrong.
- `threat_graph.rs` needed **zero code changes** — its `EvaluatedFork`/
  `HangingPiece` output already flows through the same `unflip_sensor_report`
  pass (`evaluated_forks`/`tactical.hanging` fields), confirmed by test.
- `LegalInfo`/`mate_in_1_exists` computed from the original unflipped
  position, as scoped.

**Verification, beyond the scope's plan**: constructed a mirror-position
test by hand (rank-flip + case-swap + side-to-move flip, computed
independently of `normalize_for_eval`'s own code) — a genuine White-to-move
queen fork and its exact Black-to-move mirror. Confirmed attacker/target
squares, colors, `evaluated_forks`, `hanging` pieces, and `gated_issues`
(`side` *and* `phrase`) all match the hand-computed expectation exactly, and
`final_score` is identical between the two mirrors — direct confirmation
BUG-13 is fixed as a side effect (previously, material's White-relative
sign would have broken this equality). Also ran the full ~1499-position STS
corpus (`sts_full_suite_evaluates_without_error`, many Black-to-move) to
confirm no panics across a large, realistic sample, and a hand-mirrored
en-passant pair (`e3` vs `e6` targets) to exercise the one code path
nothing else touched — both succeeded with identical `final_score`/
`state_id`. Promoted the fork-mirror test to a permanent regression
(`board_normalization_reports_real_squares_and_colors_for_black_to_move`,
`motif_canonical.rs`). Full suite: 51 passing (was 50), 0 failed, `cargo
clippy` clean (no new warnings).

**Consequence, as flagged in scoping**: `positions.hugm_score`/`state_id`
now compute differently (correctly) for previously-Black-to-move positions
— re-running `chess-sync`/`chess-derive` is needed to refresh any existing
database, same as BUG-13's standalone note above.

Color/perspective audit (2026-07-29) — requested re-check for white/black,
player/opponent mixups given the recurring pattern of such bugs this session

Ran a systematic sweep (Rust + Nu) for every site that compares, branches on,
or labels something by chess color or side-relative identity. Three concrete,
fixable-now bugs found and fixed; two deeper architectural findings need a
scoping decision before touching (recorded as BUG-13/14 below, not fixed).

FIXED:
- `extract_pawn_breaks` (`position.rs:1790`) hardcoded `color: "white".into()`
  for the us-side break and `"black".into()` for the them-side break,
  regardless of side to move — every sibling extractor
  (`extract_minority_attack`, `extract_pawn_majority`, `extract_rook_on_seventh`)
  correctly derives the label from an actual `Color` parameter. Now takes
  `us: Color, them: Color` and derives labels from `is_white()`, matching
  its siblings. Regression test: `pawn_break_color_is_invariant_to_side_to_move`
  (same physical break, FEN with `w` vs `b` to move, must report the same
  absolute color both times — it didn't before this fix).
- `concepts.rs`'s `king_exposed` block (line ~122) treated
  `groups.king_safety.blended` as White-relative ("blended < 0 → White's
  king exposed"), hardcoding `"white"`/`"black"`. But `king_safety.blended`
  is computed as `king_safety_score(us) - king_safety_score(them)` (`us =
  chess.turn()`, `position.rs:2541-2542`) — us/them-relative, exactly like
  `development.blended` two blocks below it, which already handles this
  correctly via `us_color`/`them_color`. Fixed to match. Regression test:
  `king_exposed_concept_is_invariant_to_side_to_move`.
- `chessdb/profile.nu`'s `position-win-rates`: `had_outpost`/`had_open_file`/
  `had_passed_pawn` took `MAX(ms.has_X)` over every row in the game with no
  color filter, while the sibling `had_king_exposed` correctly filtered to
  `CASE WHEN m.color = pg.player_color THEN ... ELSE 0`. Added the same
  filter to all three, so all four win-rate breakdowns are now consistently
  scoped to the tracked player's own moves.

NOT FIXED AT THE TIME — flagged for a scoping decision, found while
verifying the above. BUG-13 was subsequently fixed via the board-
normalization work (see that section, written above this one in the file
but implemented after it) — kept here as the original finding for context:

- BUG-13: FIXED (2026-07-29, via board normalization, not a standalone
  patch) — `EvalGroups.material` is White-relative (computed directly as
  White piece values − Black piece values, no color parameter — verified in
  `material_score`, `position.rs:337-476`), but `pawn_structure`,
  `piece_activity`, `king_safety`, `passed_pawns`, `development`,
  `vector_features`, `strategic`, and `tactical` are all `us − them`
  (`us = chess.turn()` at that specific FEN — verified in `compute_groups`,
  `position.rs:2528-2542` and siblings). `sum_groups` (`position.rs:2451`,
  → `final_score`/`hugm_score`) adds all nine directly with no correction.
  Result: `hugm_score` is a genuinely consistent "positive = White ahead"
  number only via its material term; the other eight components' sign
  flips depending on whose move it is in that specific stored position.
  `ai/mod.nu:209-211` documents (and `chessdb/sync.nu`'s `review-game`,
  `coach_derive_cmd.rs`'s `hurt_player` logic assume) a single uniform
  "White-relative, flip by mover color" convention for the whole score —
  true for material, not for the rest. This is the likely root cause behind
  the `king_exposed`-shaped bugs just fixed (same `us`-vs-White confusion,
  just at the aggregate-score level instead of a single concept). Not
  fixed here: correcting this touches the master evaluation number
  persisted in `positions.hugm_score` and used throughout the coaching
  pipeline (chess-review deltas, anomaly hurt_player attribution, the
  AI analyst's documented convention) — needs a decision on where the fix
  belongs (flip the 8 components in `sum_groups` only, vs. redefining what
  each `GroupValue.blended` means throughout `position.rs`) before touching it.
- BUG-14: `StateVector`'s concept-presence bits (`has_fork`, `has_pin`,
  `has_hanging`, `has_outpost`, `open_file`, `has_passed_pawn`, `has_skewer`,
  `has_discovered`) are true if *either* side has that concept present —
  `encode_state`'s `BOOL_BITS` checks read `sensor.tactical.forks`/etc.,
  which `build_sensor_report` populates by combining both `_us` and `_them`
  raw examples (e.g. `evaluated_forks.extend(graph.find_forks(them))`).
  Same for `king_exposure` (picks whichever king is more exposed, board-
  color-blind — `position.rs:2833-2840`). Consequence: `coach_derive_cmd.rs`'s
  per-concept baselines/anomalies (e.g. "eval swing when a fork was present")
  don't distinguish "the player's own fork" from "the player got forked" —
  both set the same bit. `chessdb/profile.nu`'s player-color filter (just
  applied above to `had_outpost`/etc.) narrows this to "on the player's own
  move" but can't fully fix it, since the bit itself doesn't say whose
  concept it is. A real fix would mean splitting these into per-side bits
  in the `state_id` bitfield (more of the 16-bit budget, another
  `move_states` migration) — a StateVector schema change, not a quick fix.
  Not fixed here.

Also noted, not fixed (pre-existing heuristic-calibration quirk, unrelated
to color bookkeeping): while verifying the `king_exposed` fix empirically,
found that `king_safety_score`'s shield/storm table (`position.rs:936-947`)
indexes a *missing* pawn on a file the same as a pawn still sitting on its
home square (both leave `shield_rank` at its loop-entry default), and the
resulting index (0 for White, 7 for Black after the `7 - rank` flip) maps to
very different bonuses — a fully pawnless king can score as if maximally
sheltered. Surfaced by a test position where a bare king scored *safer* than
a fully-castled one. This is a calibration issue in the heuristic itself
(same class as this file's other acknowledged "GUESS weight" gaps), not a
color-mixup — flagged here only because it's exactly the kind of thing that
would otherwise get silently absorbed into "well, chess heuristics are
approximate" without anyone writing down what was actually observed.

OPEN: BUG-14 only now (see above — needs a `state_id` schema change, not a
quick fix). BUG-13 resolved via board normalization.

RESOLVED (continued):
- BUG-12: FIXED (2026-07-29) — `dataset_builder_cmd.rs`'s two divergent label-computation
  paths. `run()` computed a side-relative WDL/scalar label from `result` via `encode_position`
  + `labels_buf`/`wdl_buf`, but passed them into `write_shard` as `_features`/`_labels`/`_wdl`/
  `_weights` — all underscore-prefixed, unused; `write_shard` instead built its own White-relative
  `score`/`result_float` from `result` directly and wrote *that*. Before fixing, checked whether
  this was a live correctness bug (wrong-perspective labels for black-to-move positions), not
  just dead code — read `bulletformat`'s vendored `ChessBoard::FromStr` source directly rather
  than assuming. First pass concluded there was no internal flip and nearly reported a false
  bug; re-reading the full function turned up `if stm == 1 { score = -score; result = 2 - result }`
  a few lines further down, confirming the string format *does* expect White-relative values
  with `bulletformat` doing its own side-to-move flip internally — so `write_shard`'s existing
  computation was already correct, and the unused `run()` computation was simply dead weight
  (removing it doesn't change any real output). Removed `encode_position`/`features_buf`/
  `labels_buf`/`wdl_buf`/`weight_buf` entirely; bundled the 7 remaining metadata vectors into a
  `ShardMeta` struct (`write_shard` was already flagged for `too_many_arguments`, 13/7 — now
  passes one struct instead). Also fixed the command's stale description ("Build NPZ shards")
  to match what it actually writes (`bulletformat` `.bin` + `.meta.json`).
  Verified beyond compiling: added unit tests that build the actual "fen | score | result"
  line and parse it through the real `bulletformat::ChessBoard` parser, confirming a white win
  is stored positive when White is to move and negative when Black is to move (i.e. the label
  really is side-to-move-relative in the final output, not just in the discarded computation).
  Test suite 34→37 passing.
  **Flag for whoever revives this (2026-07-30 audit)**: this correctness relies on `write_shard`'s
  input `fen` being real (bulletformat's own side-to-move flip, `if stm == 1 { ... }`, is keyed
  off the FEN's own turn token). This path isn't currently wired to read from the `positions`
  table via any `chessdb/*.nu` command — if it ever is, and the source is `positions.fen`, that
  FEN will be canonical (always "w" to move), which would make bulletformat's internal flip a
  permanent no-op. Re-verify the score/result labeling once this pipeline is unpaused and fed
  from the database rather than an ad hoc external source.

- BUG-15: FIXED (2026-07-30) — `hugm_score`/`hugm_eval_arr`'s sign convention drifted out from
  under several consumers that pre-date (or were never updated for) board normalization.
  Found during a service-definition/YAGNI audit ("is chess-review/chess-profile-position fit
  for purpose"), verified empirically (Rust-level tests constructing real positions, not just
  derived on paper) before touching anything, per this session's standing discipline.

  **The convention, precisely**: `positions.hugm_score` / each `hugm_eval_arr` component is
  `final_score` = `sum_groups(&groups)` computed on the *normalized* (White-always-to-move)
  position — i.e. relative to whoever is actually to move *at that stored position*. Since a
  `moves` row's position (`next_position_id`) is reached *after* `m.color`'s move, whoever is
  to move there is `m.color`'s opponent, not `m.color` itself. So a row's own mover's
  perspective is always `-hugm_score`, unconditionally — never `m.color`-conditional (chess's
  turn alternation makes "the position after my move has my opponent to move" true regardless
  of which color I am).

  Three sites assumed the older, no-longer-true convention (`hugm_score` = real-White-absolute,
  flip only for Black) instead:
  1. **`chessdb/profile.nu`**: `profile-phase-stats`'s `avg_score_cp` and
     `position-eval-components`'s `avg_king_safety_cp` used
     `CASE WHEN m.color='white' THEN X ELSE -X END` — the `black` branch happened to already be
     correct by coincidence (its negation is what the true convention needs *regardless* of
     color), so only White-mover rows were wrong. `avg_pawns_cp`/`avg_activity_cp` had *no*
     negation at all — wrong for every row, both colors. Fixed by replacing all of these with a
     plain, unconditional `-p.hugm_score` / `-json_extract(...)` — the color CASE was never
     actually doing anything a constant negation didn't already cover.
  2. **`chessdb/sync.nu`**'s `review-game`: same White-branch-wrong CASE for the displayed
     `score` column, plus a second, distinct bug in the Δ-component columns — `arr` (this row's
     position) and `prev_arr` (the position before this move) are relative to *opposite* sides
     (whoever's to move differs by exactly one ply), so `arr - prev_arr` was subtracting two
     numbers in different reference frames. The correct per-component swing from the mover's
     own perspective is `-(arr + prev_arr)`, not `arr - prev_arr` times a color sign. Fixed both.
  3. **`coach_derive_cmd.rs`**'s `detect_anomalies` (Rust): `hurt_player` — the flag behind
     `profile-concepts`' `hurt_rate`, the headline "what hurts this player" signal — had the
     identical White-branch-wrong pattern (`row.color=='white' && signed_delta<0`, `'black' &&
     signed_delta>0`). Here `signed_delta = curr - prev` compares a *single player's own* two
     consecutive rows (same color both times, so both relative to the same opponent) — a valid
     apples-to-apples subtraction, unlike (2) — so the fix is simpler: `hurt_player = signed_delta
     > 0` unconditionally (same collapse-the-CASE-into-a-constant pattern as (1)). This was the
     most consequential of the three: for every White-playing profiled user, every concept's
     `hurt_rate` was measuring "how often did this concept coincide with the player *improving*,"
     not being hurt — backwards, in the one number the whole coaching product is built to answer.
     `MoveRecord.color` became entirely unused after this fix (it was the only reader) — removed
     the field along with its parsing rather than leave known-dead struct data around.

  Verified: added `hurt_player_is_positive_signed_delta_regardless_of_color`
  (`coach_derive_cmd.rs`), which hand-supplies a baseline (bypassing Welford noise from tiny
  synthetic samples) so a real anomaly clears the z-score gate for both a White-mover and a
  Black-mover player, asserting both come back `hurt_player=true`. Confirmed the test actually
  catches the bug — not a tautology — by temporarily reintroducing the old color-conditional
  logic and watching it fail before restoring the fix. Full suite green throughout (37→38 tests),
  `cargo clippy --all-targets` clean. The `chessdb/*.nu` side of the fix (profile.nu/sync.nu)
  isn't independently unit-testable without the plugin registered (this sandbox's Nu 0.114 vs.
  the plugin's target 0.111, same known limitation as earlier canonical-identity work) — verified
  by direct algebraic/empirical derivation of the correct formula instead, the same rigor applied
  to (3).

  **Not yet done**: any `move_anomalies`/`player_baselines` rows already stored in an existing
  database were computed under the old, wrong `hurt_player` convention for White-playing users.
  This needs a `chess-derive` re-run per affected player once this ships (a normal, "safe to
  re-run" chess-derive refresh — not a full chess-sync rebuild, since `positions`/`moves`
  identity itself didn't change, only a downstream derived signal).

Top-down YAGNI / fit-for-purpose audit (2026-07-30)

Ran against a from-first-principles service definition (grounded in `chessdb/mod.nu`'s actual
export surface + `ai/mod.nu`'s tool registrations, not invented): chessdb.nu ingests any number
of players' chess.com games into a local SQLite file, evaluates positions with HUGM, and derives
per-player coaching signals so an LLM/human coach can have an evidence-grounded conversation —
via Nushell + `ai.nu` tool registration only, no web server/HTTP/GUI (architecturally, not just
currently). Findings, not yet acted on except BUG-15 above (fixed as its own item):

- Dead code, clear deletion candidates: `chessdb scan-pgn`/`ScanVisitor`/`core::scan_pgn` (zero
  callers anywhere, already known to hash non-canonically); `core::legal_moves` (zero callers).
- Initially misjudged as YAGNI, corrected after reading `NNUE_AUDIT.md`: `nnue-eval`,
  `hugm_harness`, `lichess_to_jsonl`/`pgn_to_jsonl` are a live, intentional dev-time HUGM
  calibration workflow (Stockfish ground truth → regress HUGM's own weights), not dead NNUE-
  training weight — legitimate to be unreachable from `chessdb/*.nu`, the same way a test
  harness doesn't need to be reachable from the product. `dataset_builder_cmd.rs` (bulletformat/
  NPZ shards for training an actual replacement net) genuinely is paused per NNUE_AUDIT.md, with
  the sign-convention risk noted in BUG-12 above if it's ever revived reading from `positions`.
  User decision needed: delete, or keep as an explicitly-quarantined placeholder.
  - Low-cost, technically-unreachable-from-the-interface utilities (`zobrist`, `pgn-to-fens`,
    `pgn-to-batch`): legitimate manual/debug tools, thin wrappers around functions already used
    internally — "reachable from the product interface" is the wrong bar for a debug utility.
  - `ai/mod.nu`'s `chess-analyst` system prompt hand-documents the schema and is stale: claims
    `moves.clock_seconds`/`positions.nnue_score`/`.eval_depth` (don't exist), missing
    `moves.canonical_san` (does exist). Duplicates `chess_db_schema` (a tool in the same file
    that could just be relied on live) — same "two sources of truth" problem CLAUDE.md already
    flags for the terms-bag pattern. Its score-convention line was also stale relative to BUG-15.
  - `hugm-eval` (evaluate an arbitrary hypothetical FEN) fits the coaching-conversation purpose
    but isn't exposed as an `ai.nu` tool — minor gap, not a defect.
