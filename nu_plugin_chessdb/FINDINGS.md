FINDINGS: HUGM (Human GM) evaluation — the archive

This file is the archive: the full chronological build log, every session, every wrong turn
and correction, in the order they happened. For the current architecture, the design
philosophy, and the best-verified findings — compacted and organized by idea instead of by
date, kept sharp and focused rather than growing forever — see `PLAN.md`. Read `PLAN.md`
first for "what is this and why does it look like this"; come here for "how did we get
here, and what else was tried and rejected."

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
currently). Findings, acted on except the last two (still open):

- **FIXED (2026-07-30)**: dead code deleted — `chessdb scan-pgn`/`ScanVisitor`/`core::scan_pgn`
  (zero callers anywhere, already known to hash non-canonically) and `core::legal_moves` (zero
  callers). Both removed along with their `MoveRow`/`ScanGameRow`/`ScanMoveRow`/registration
  plumbing; full suite green throughout (38→35 lib tests — no tests existed for either, so the
  count drop is just their removal, not a regression).
- **FIXED (2026-07-30)**: `dataset_builder_cmd.rs` deleted, per explicit user decision (asked
  directly rather than assumed) — the bulletformat/NPZ shard-building path for training a
  replacement NNUE net, paused per NNUE_AUDIT.md with no active work. Its `bulletformat`
  dependency was the only user in the crate; while removing it, found `ndarray`/`ndarray-npy`
  had **zero usages anywhere already** (a leftover from an even earlier NPZ-only approach,
  predating bulletformat) — removed all three from `Cargo.toml`. `NNUE_AUDIT.md` updated to
  record the removal. BUG-12's sign-convention risk note is now moot (there's no code left to
  revive with that risk) but left in place as history.
  - Initially misjudged as part of the same YAGNI cluster, corrected after reading
    `NNUE_AUDIT.md`: `nnue-eval`, `hugm_harness`, `lichess_to_jsonl`/`pgn_to_jsonl` are a live,
    intentional dev-time HUGM calibration workflow (Stockfish ground truth → regress HUGM's own
    weights), not dead NNUE-training weight — legitimate to be unreachable from `chessdb/*.nu`,
    the same way a test harness doesn't need to be reachable from the product. Left as-is.
  - Low-cost, technically-unreachable-from-the-interface utilities (`zobrist`, `pgn-to-fens`,
    `pgn-to-batch`): legitimate manual/debug tools, thin wrappers around functions already used
    internally — "reachable from the product interface" is the wrong bar for a debug utility.
    Left as-is.
- **FIXED (2026-07-30)**: `ai/mod.nu`'s `chess-analyst` system prompt hand-documented the schema
  and was stale: claimed `moves.clock_seconds`/`positions.nnue_score`/`.eval_depth` (don't
  exist), was missing `moves.canonical_san` (does exist), and `move_states` (missing
  `has_outpost`/`has_open_file`/`has_passed_pawn`). Rewrote the column lists to match
  `chessdb/db.nu`'s actual schema exactly, added a one-line pointer to fall back on
  `chess_db_schema` (a tool in the same file) rather than trust this summary blindly — doesn't
  eliminate the duplication (CLAUDE.md's "two sources of truth" concern about the terms-bag
  pattern applies here too), but at least it isn't actively wrong anymore. Also rewrote the
  score-convention line to match BUG-15's fix (mover-relative, not White-absolute), and added a
  short explanation of `positions.fen`/`.zobrist` being canonical so the model doesn't try to
  infer real color from a canonical FEN's own "w"/"b" token. Verified the rewritten prompt
  string is syntactically valid — an unescaped `"` I initially introduced (`"the player who
  just moved."`) broke Nu's string parsing (`nu -c 'use ai/mod.nu'` failed with
  `extra_token_after_closing_delimiter`); caught and fixed by actually loading the module,
  not just eyeballing the diff. The remaining `AI_PROMPTS`-not-found error after that fix is
  pre-existing/expected (this module assumes `ai.nu`'s environment is already initialized;
  confirmed identical on the pre-edit committed version too, so not something this touched).
- **STILL OPEN**: `hugm-eval` (evaluate an arbitrary hypothetical FEN) fits the
  coaching-conversation purpose but isn't exposed as an `ai.nu` tool — minor gap, not a defect.

Second pass: schema/transient-struct duplication audit, and "is the canonical FEN
transform actually universal?" (2026-07-30)

User's exact question: the forward transform (`normalize_to_white_to_move`) was already
centralized in `canonical.rs` and shared by `core.rs`/`eval::position`. The *reverse* side
was not fully universal — two concrete gaps, both fixed:
1. `normalize_to_white_to_move` only conditionally flips (based on the input's own
   `chess.turn()`), so there was no way to de-canonicalize a position you already *know*
   is canonical (e.g. a `positions.fen` row known via `moves.color` to have been flipped)
   — calling the same function again is a no-op, since a canonical position always reads
   "White to move". Fixed by extracting the actual mirror+recolor logic into a new
   `pub fn flip_colors(chess: &Chess) -> Result<Chess>` — unconditional, and (unlike the
   old inline version, which hardcoded `turn: Color::White`) sets `turn: chess.turn().other()`
   so it's a true involution usable in *either* direction. `normalize_to_white_to_move` is
   now a thin wrapper: flip via `flip_colors` only when not already White-to-move.
2. The generic un-flip helpers (`unflip_color`, `unflip_phrase`, `unflip_square_str`) were
   private to `eval/position.rs` even though they don't depend on anything eval-specific —
   moved to `canonical.rs`, crate-wide reusable now. (`unflip_piece_ref`/`unflip_sensor_report`
   stayed in `eval/position.rs` — they operate on `PieceRef`/`SensorReport`, eval-specific
   types `canonical.rs` shouldn't depend on, per its own doc comment on dependency direction.)
   Removed `position.rs`'s redundant private `unflip_square` (a thin rewrap of
   `canonical::unflip_square` with no logic of its own).
   Verified: new test `flip_colors_is_an_involution_and_de_canonicalizes` proves
   `flip_colors` agrees with `normalize_to_white_to_move` on the forward direction *and*
   that flipping its own output recovers the original position exactly — the actual
   de-canonicalization use case this was extracted for.

Also audited the SQLite schema and Rust-side "transient" structs (`MoveRow`, `BatchSummary`,
`PendingPos`/`FenToEval`, `MoveRecord`) for duplication/YAGNI, per the user's request. Fixed,
in the order presented:
- **My own oversight from the prior pass, corrected**: `src/position_encoder.rs` (194 lines,
  4 tests) had zero callers left once `dataset_builder_cmd.rs` — its only consumer — was
  deleted, yet I'd written NNUE_AUDIT.md to claim it was a deliberate placeholder without
  checking. Deleted; NNUE_AUDIT.md corrected to say so plainly.
- **Dead SQLite columns removed** (declared/written, never queried anywhere in
  `chessdb/*.nu`): `positions.board_pieces` (was computed with real CPU work in
  `process_corpus.rs` for nothing), `positions.updated_at`, `openings.moves`,
  `transition_events.last_updated`. All dropped via `ALTER TABLE ... DROP COLUMN`
  migrations (try/catch, safe to re-run — verified against both a fresh DB and a
  simulated pre-migration DB with the old columns and an existing row, confirming the
  row survives and re-running `init-db` twice is a no-op the second time). All write-side
  code removed too (`PendingPos`/`FenToEval` no longer carry `board_pieces`;
  `fetch-and-seed-eco` no longer reads/stores ECO's `moves` field).
  `player_baselines.count`/`.last_updated` needed a different call: `.last_updated` is
  genuinely dead (dropped), but `.count` turned out to be the missing half of the very
  next fix below — kept and wired up for real instead of deleted. Flagging this deviation
  explicitly since the original finding said "dead column," and the actual fix was "make
  it not dead" for this one field.
- **`chess-derive --min-games` actually does something now.** It was accepted and silently
  discarded twice (`let _ = min_games;` in `detect_anomalies`, an unused `_min_games` param
  in `compute_transitions`), and `compute_baselines` computed each Welford baseline's real
  sample count only to throw it away before it could reach anything — which is why
  `player_baselines.count` was always the SQL default 0. Fixed: `compute_baselines` now
  returns `(mean, std, count)`; `detect_anomalies` skips z-score anomaly emission entirely
  when `count < min_games` (both the state-vector-concept path and the eval-component
  path); `compute_transitions`'s hardcoded `total >= 3` threshold for flagging a risky
  transition became `total >= min_games`, using the same configurable trust bar instead of
  a magic number (worth revisiting if 25 turns out too strict specifically for transition
  counts — state-pair transitions are rarer events than a single concept firing, and this
  hasn't been empirically checked against real data yet). `format_results` now emits
  `count` in `baselines_out`, and `chessdb/derive.nu`'s db-merge column list for
  `player_baselines` includes it — verified this actually reaches the table with a mocked
  `chessdb derive-coach-signals` call through a throwaway db. Added
  `detect_anomalies_respects_min_games_baseline_trust`, and confirmed it actually catches
  the bug (reverted the gate, watched it fail, restored it) — same discipline as BUG-15's
  `hurt_player` test.
- **`BatchSummary.collisions`** (`core::pgn_to_batch_record`) — computed (a full
  `BTreeMap<zobrist, BatchCollisionRow>` tracking occurrence counts and game indexes per
  position) but unread by its only real consumers: `lichess_to_jsonl.rs`/`pgn_to_jsonl.rs`
  only touch `.positions`/`.games`, and `.unique_positions.len()` for one log line — never
  `.collisions`. Removed the computation, the `BatchCollisionRow` struct, and the
  `collisions`/`stats.collisions` fields from the Nu-facing `pgn-to-batch` output.

Verified throughout: full test suite green at every step (ended at 32 lib tests, up from
31 after `position_encoder.rs`'s 4 tests were removed with the file, since the two new
regression tests added 1 net beyond that), `cargo clippy --all-targets` clean on every
touched file, STS smoke test passes.

Third pass: re-audit for anything remaining (2026-07-30)

Systematic re-check after the two passes above — Cargo.toml deps, discarded CLI params,
the games.eco/opening pipeline, doc staleness, `moves.uci`, and the `src/bin/*` dev tools.
Found and fixed:

- **I broke my own fix from earlier the same day.** Fixing `ai/mod.nu`'s stale schema
  (documented in the "fit-for-purpose" audit above) happened *before* the dead-column
  drop (`board_pieces`/`updated_at`), so the "corrected" prompt immediately went stale
  again the moment those columns were dropped. Same problem in `CLAUDE.md`'s canonical-
  identity section (used `positions.board_pieces` as its worked example of a canonical
  field). Fixed both. Lesson for next time: when a later step in a batch changes the
  schema, re-check earlier steps in the *same* batch that described the schema, not just
  what existed when each step was written.
- **`player_baselines`'s AI-facing doc line was also incomplete** — the min-games fix
  added a real `count` column that `ai/mod.nu`'s schema summary didn't mention at all
  (never stale, just never updated). Added, with a note on what a low count means.
- **Real duplication in `src/bin/pgn_to_jsonl.rs`**: the ~50-line "parse one PGN block,
  emit one JSONL line per position" logic was written out twice verbatim — once for
  blocks that end on a blank line, once for a trailing block with no final blank line.
  Extracted `has_valid_result`/`process_game_block`; both call sites now share one
  implementation. This is the same class of thing `pgn_to_jsonl.rs`'s sibling tools
  didn't have — worth a second look if more `src/bin/*` tools get added later.
- **Three small, genuinely harmless bits of dead plumbing**, fixed for completeness:
  `core.rs::GameVisitor::new`'s unused `_span` parameter (removed; 2 call sites updated);
  `hugm_harness.rs::RegressionRow::hugm_raw` (`#[allow(dead_code)]`'d by whoever wrote it —
  already known-unused, just never deleted); a discarded `.get(...).unwrap_or(0)`
  extraction in `position.rs`'s `king_tropism_present` test that asserted nothing (the
  `assert!` one line above already covers what the test claims to check);
  `lichess_to_jsonl.rs`'s `_created_at` variable (parsed from JSON, immediately discarded —
  the code already explains it doesn't need an accurate date, so there was nothing to parse
  in the first place).

**Checked, not a bug — worth recording so it isn't re-litigated**: `game_parse.rs`'s
`extract_eco_opening` (PGN-header/URL-derived, cheap, always available) and `db.nu`'s
`enrich-openings` (deeper local-ECO-data FEN match, always overwrites the first when it
finds any match) look redundant at a glance — the first's value is virtually always
replaced in the normal `chess-sync` flow. But `enrich-openings` no-ops entirely if the
`openings` table was never seeded (e.g. first run with no internet), in which case
`extract_eco_opening`'s value is the *only* one populated — a legitimate degraded-mode
fallback, not accidental duplication. Left as-is; flagging that the fallback relationship
isn't documented anywhere a future reader would find it, in case someone wants to add a
comment cross-referencing the two.

**`moves.uci` re-checked**: genuinely zero readers in `chessdb/*.nu` today, same as
`board_pieces` was — but unlike `board_pieces`, UCI notation is a standard, expected
column for any chess-moves table and is reachable via `query_chess_db` for ad hoc use.
Not the same class of finding; left alone.

Verified: full suite green (32 tests, unchanged — none of this pass's fixes touched
anything with its own tests), clippy clean, STS smoke test passes.

Fourth pass: make the module graph self-documenting instead of hand-correcting it (2026-07-30)

Used `cargo-modules` (installed via `cargo install cargo-modules`) to render the crate's
module dependency graph and confirm it's a clean, acyclic, layered DAG — `canonical` and
`eval::concept_types` at the bottom with zero internal dependencies, every plugin-command
module at the top depended on by nothing. In doing that, found the tool undercounts:
`core -> canonical`, `canonicalize_fen_cmd -> core`, and `zobrist -> core` were all real
dependencies invisible in the graph, because those call sites only ever reference the
target via a fully-qualified path (`crate::canonical::normalize_to_white_to_move(...)`)
with no `use` import and no type-position reference anywhere else in the file — the one
thing `cargo-modules` reliably resolves.

Rather than keep a hand-annotated correction alongside the tool's output, fixed the root
cause: converted every purely-fully-qualified `crate::module::item` call site across the
whole crate to a proper `use` import — `core.rs`, `canonicalize_fen_cmd.rs`, `zobrist.rs`,
`hugm_eval_cmd.rs`, `pgn_to_fens.rs`, `coach_derive_cmd.rs`, and within `eval/` itself
(`position.rs`, `concepts.rs`). The `ChessdbPlugin`/`PLUGIN_CATEGORY` fully-qualified vs.
`use`-imported split across command modules (some files did one, some the other, no
reason for the difference) got the same treatment while at it — same principle, same fix.
Re-ran `cargo-modules` afterward: all three previously-invisible edges now appear on their
own, no manual correction needed, and the acyclic check is still clean (the one thing it
flags — `ChessdbPlugin` <-> `ChessdbPlugin::new` — is the tool treating a struct and its
own constructor as circular, a false positive unrelated to any of this).

While doing this, the mechanical "just import it" fix surfaced one real structural issue
rather than a pure style nit: `SensorReport` (`sensor.rs`) has a `gated_issues:
Vec<GatedIssue>` field, but `GatedIssue` was defined in `concepts.rs` — a module that
itself depends on `sensor.rs` (`SensorReport`). Adding the "obvious" `use
crate::eval::concepts::GatedIssue;` to `sensor.rs` would have created a genuine new
`sensor.rs` <-> `concepts.rs` cycle, not just satisfied the linter — the fully-qualified
path had been silently hiding an inverted dependency the whole time, exactly the kind of
thing this exercise was meant to surface. Fixed by moving `GatedIssue`'s definition to
`concept_types.rs` (the shared foundational types module both `sensor.rs` and
`concepts.rs` already depend on) — a plain data struct with no dependencies of its own, so
the move was free. Updated `eval/mod.rs`'s public re-export (`pub use
concept_types::GatedIssue;`) to match.

Verified: full suite green (32 tests), clippy clean, STS smoke test passes, and
`cargo-modules`'s own graph now matches reality without hand-editing — the actual goal,
per the user's framing, being that the code expresses its own true structure rather than
needing documentation (or a diagram's footnote) to explain what it really does.

Fifth pass: expressiveness — does the code read as the one right way to do this? (2026-07-30)

Five areas, one real fix landed, one large finding deferred (needs a decision, see below),
the rest checked clean:

- **FIXED**: `coach_derive_cmd.rs` had six unexplained magic-number thresholds scattered
  through `compute_baselines`/`detect_anomalies`/`compute_transitions`/`Welford::std_dev`
  (`1.0`, `30.0`, `2.0`, `-200`, `0.25`, the `std_dev` floor) — each meaningful, none named,
  so the actual three-tier design (noise floor -> anomaly-candidate size -> z-score,
  entirely separate from the blunder/transition-risk thresholds) was invisible without
  tracing every call site. Named all six (`NOISE_FLOOR_CP`, `ANOMALY_CANDIDATE_CP`,
  `ANOMALY_Z_THRESHOLD`, `STD_DEV_FLOOR_CP`, `BLUNDER_LOSS_CP`, `RISKY_TRANSITION_RATE`)
  with a doc comment laying out the tiers, right at the top of the file.
- **DEFERRED — large, needs a decision**: color is a bare `String` ("white"/"black")
  everywhere in the eval engine's output types — 13 struct fields in `concept_types.rs`,
  ~51 literal `"white"`/`"black"` constructions across `position.rs`/`concepts.rs`/
  `threat_graph.rs`/`canonical.rs`. This is the same class of thing that caused several
  real bugs this session (hardcoded-color bugs, the `unflip_color` string-swap, BUG-13/15's
  sign confusion) — an actual `Color` enum (serializing to the same "white"/"black" JSON
  strings, so zero external/schema impact) would make a mismatched or mistyped color a
  compile error instead of a silent bug. Not attempted in this pass: meaningfully bigger
  and riskier than anything else fixed this session (100+ call sites across 5+ files, not
  a mechanical rename), so it needs an explicit go-ahead rather than being bundled in.
- **Checked, clean**: `position.rs`'s ~70 functions consistently follow the documented
  `detect_X`/`extract_X`/`X_score`/`X_to_typed` naming families; `get_term_i64` (the one
  place still reading a `terms` bag) is the sanctioned conversion boundary itself, not a
  violation. `chessdb/*.nu` follows its own documented idioms throughout; the one repeated
  SQL fragment (`CASE WHEN white = ? THEN 'white' ELSE 'black' END`, in 3-4 profile.nu
  queries) is left inline deliberately — CLAUDE.md already says not to over-engineer literal
  SQL, and each query stays independently readable/copy-pasteable for manual use, which a
  Nu-side string-composed helper would work against. Error handling: zero `.unwrap()` calls
  in any production code path crate-wide (confirmed by scanning every file up to its own
  `#[cfg(test)]` boundary); the two `.expect()` calls are both `RwLock::read().expect("weights
  lock")` — the standard, correct way to handle lock poisoning, not a shortcut.
- **FIXED in passing**: `src/bin/hugm_harness.rs`'s `gen_weights` used three bare
  `.unwrap()`s because it returned `()` instead of `anyhow::Result<()>`, the convention
  every other function in that file already follows. Propagated `Result` through it and
  its caller `run_multivariate_regression` so the one inconsistency in the file's error
  handling is gone.

Verified: full suite green (32 tests), clippy clean, STS smoke test passes.

`Side` enum for color — scoped and implemented 2026-07-30

Follow-on from the deferred finding above. Re-investigated to get exact numbers rather
than an estimate, and found the case is stronger than "type safety": the exact conversion
`if color.is_white() { "white" } else { "black" }.into()` (or the `== Color::White`
variant) is copy-pasted **22 times** across `position.rs`/`threat_graph.rs` — identical
in every case, just a different variable name. This isn't just a latent-bug risk anymore,
it's a concrete, already-present DRY violation this whole audit has been hunting for.

**The type**: `pub enum Side { White, Black }` in `concept_types.rs`, deriving
`Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize` with `#[serde(rename_all =
"lowercase")]` — this serializes byte-identically to the current `String` fields
("white"/"black"), so every JSON/Nu/SQL consumer downstream is unaffected. Methods:
`other(self) -> Side` (replaces `canonical.rs::unflip_color`'s string-swap entirely —
that whole function becomes dead and gets deleted, callers just write `x.color =
x.color.other()`), and `From<shakmaty::Color> for Side` (replaces all 22 duplicated
`if ... .is_white() { "white" } else { "black" }` conversions with `Side::from(color)`).
`shakmaty::Color` can't get `Serialize` directly (orphan rule — it's a foreign type),
which is exactly why this needs its own small local type rather than reusing shakmaty's
enum for the output layer.

**What converts** (all in `nu_plugin_chessdb/src/eval/`, zero external API change):
- `concept_types.rs`: 14 fields (`PieceRef.color` and 13 more `color`/`side: String`
  fields across `OpenFile`, `PassedPawn`, `PawnIsland`, `KingExposure`, `DoubledPawn`,
  `IsolatedPawn`, `DevelopmentInfo`, `PawnBreak`, `MinorityAttack`, `PawnMajority`,
  `RookOnSeventh`, `CenterControl`, `GatedIssue.side`).
- `concepts.rs`: `Concept.side`, `count_and_push_by_color`'s `color_of: impl Fn(&T) ->
  &str` becomes `Fn(&T) -> Side`, its `us_color`/`them_color: &str` params become `Side`.
- `position.rs`: `PositionRecord.side_to_move`; all 8 duplicated conversions there;
  `unflip_piece_ref`/`unflip_sensor_report`'s calls to `unflip_color` become `.other()`.
- `threat_graph.rs`: 6 duplicated conversions.
- `canonical.rs`: delete `unflip_color` (superseded by `Side::other()`); `unflip_phrase`
  is unaffected (free-text word-swapping on `GatedIssue.phrase`, a genuinely different
  problem — nothing there is a structured `Side` value).

**Explicitly out of scope**: `core.rs`'s `MoveRow.color`/`GameVisitor`'s `"white"`/`"black"`
literals stay `String` — that's DB/`moves`-table-facing (SQLite has no enum type, so
there's no analogous type-safety win there), not the eval engine's internal output-type
layer this finding is about. `move_states`/`positions` schema, Nu-side `chessdb/*.nu`:
untouched, since the wire format doesn't change.

**Verification plan**: full test suite plus a byte-for-byte JSON diff on a few of the
existing hand-verified mirror-position fixtures (`motif_canonical.rs`,
`canonical_identity.rs`) before/after, to prove this is a pure internal refactor with
zero observable behavior change — not just "it compiles."

**Implemented (2026-07-30), confirmed via "yes, go ahead":** matched the scope above
closely, with three things found only once actually doing the work:

1. **A 15th field the scope missed**: `threat_graph.rs`'s locally-defined `CaptureStep`
   struct also had its own `color: String` — not part of the original 14-field count
   (that only covered `concept_types.rs`), found once the compiler's cascading errors
   pointed at it. Converted along with everything else.
2. **One field deliberately left as `String`, for a real reason**: `threat_graph.rs`'s
   `ExchangeChain.winner` is three-valued (`"white"`/`"black"`/`"even"`), not two — it
   doesn't fit `Side` at all (a draw isn't a color), so it stays `String`. Worth
   recording so it isn't mistaken for a spot the conversion missed.
3. **`render_explanations`/`render_structured_explanations` (`position.rs`) needed their
   own small fix**: both built a capitalized display string (`"White"`/`"Black"`) for
   human-readable phrases from `record.side_to_move.as_str()` — since `Side`'s `Display`
   only gives lowercase (matching the JSON convention), kept a plain `if us_color ==
   Side::White { "White" } else { "Black" }` for that one display-text purpose. Not the
   same duplicated pattern as the 22 struct-field conversions — those construct a
   `Side` value; this constructs a capitalized string for a sentence, a genuinely
   different job.

`Side::other()` did replace `canonical::unflip_color` entirely, and `Side::from(shakmaty
::Color)` did replace all ~22 duplicated conversions (`position.rs` and `threat_graph.rs`
combined) with a single call each, exactly as scoped.

Verified: full test suite green (32 tests, unchanged — no test's *behavior* changed, only
some assertions' syntax, comparing against `Side::White`/`Side::Black` instead of string
literals), `cargo clippy --all-targets` clean (including a `clone_on_copy` warning clippy
caught in a test — `Side` being `Copy` made a leftover `.clone()` redundant, fixed), STS
smoke test passes. Proved the core claim — zero external wire-format change — directly:
serialized a `PositionRecord` for the same hand-verified mirror-position fixture already
used in `motif_canonical.rs`, and confirmed the JSON contains exactly `"color": "white"`/
`"black"` and `"side": "white"`/`"black"` (lowercase, matching the old `String` fields
byte-for-byte) with no capitalized or otherwise-different variant leaking through.

Re-audit 2026-07-30: role_name/square_name dedup, .nu-side sweep

**Fixed, `src/eval/threat_graph.rs` + `src/eval/position.rs`:**
- `role_name(Role) -> String` (Role → full piece name) was duplicated: a private helper
  in `threat_graph.rs`, and an identical inline `match` inside `position.rs`'s
  `board_to_piece_ref`. Made `role_name` `pub` and had `board_to_piece_ref` call it
  instead of re-deriving the same match.
- `threat_graph.rs`'s own `square_name(sq: Square) -> String` (hand-rolled `a`+file /
  `1`+rank formatting) was entirely redundant with `shakmaty::Square`'s own `Display`
  impl (confirmed by reading shakmaty 0.26.0's source directly — it already produces the
  same "e4"-style output). Replaced all 7 call sites with `.to_string()` and deleted the
  function. (`position.rs`'s separate `piece_square_name` helper is not a duplicate of
  this — it names a *piece on* a square for explanation text, a different job; left as
  is, matching the note already in this file's "D: Mobility & PST" status entry.)
- Verified: `cargo check --all-targets`, `cargo test` (32 lib + 19 integration, all
  green), `cargo clippy --all-targets` clean on both touched files (remaining warnings
  are the same pre-existing ones in `hugm_harness.rs`/`lichess_to_jsonl.rs`/
  `pgn_to_jsonl.rs` noted earlier in this file).

**Confirmed already done, not re-touched:** re-checked the "Consistency pass on
nu_plugin_chessdb" plan (module doc in `eval/mod.rs`, `render_explanations`/
`render_structured_explanations` reading `SensorReport` not `.terms`, `coach_derive_cmd.rs`'s
`decode_state_id`/`StateVector`/`state_vector_to_value` unification with a fast/slow
agreement test, `hugm_eval_cmd.rs`'s shared `build_output_value`, `core.rs`'s
`get_canonical_hash` reuse) — all five items are already implemented and committed from
an earlier session. Nothing outstanding there.

**Extended the audit to the Nu side** (`ai/mod.nu`, `chessdb/*.nu`) per CLAUDE.md's own
documented idioms, since the Rust crate is now clean. Findings, fixed vs. deferred:

*Fixed:*
- `ai/mod.nu`: the five `get_*_profile` tool handlers (`get_coach_profile`,
  `get_tactical_profile`, `get_precision_profile`, `get_positional_profile`,
  `get_opening_profile`) were byte-for-byte identical except which `chess-profile-*`
  subcommand they called — same username-guard, same `^nu -c` subprocess invocation, same
  exit-code/stderr handling. A sixth profile command would have been one copy-paste to
  forget. Extracted a shared `call-profile-tool [subcmd, args, nu_script, db]` def; each
  handler is now a one-line call. Verified behaviorally equivalent by exercising the
  extracted def directly (empty-username short-circuit, and real-username subprocess
  dispatch) outside `export-env`'s ai.nu-dependent context, plus `nu-check` on the file.
- `chessdb/sync.nu` (`import-records`) and `chessdb/derive.nu` (`chess-derive`): six
  `if (X | is-not-empty) { db-merge ... }` guards were redundant — `db-merge` itself
  (`db.nu:12`) already no-ops on empty `records`, and confirmed the intermediate
  `where`/`reject`/`rename`/`insert` pipeline steps feeding it are empty-list-safe too.
  Dropped the guards. Verified via `nu-check` on both files plus direct empty-list
  pipeline tests in isolation.

*Deferred (documented, not fixed — real but lower-value/higher-risk than the above):*
- `chessdb/profile.nu` has several small SQL fragments duplicated 2-3x each as literal
  text inside `query db "..."` strings: the phase-bucket-to-label `CASE ... WHEN 0 THEN
  'deep_endgame' ...` mapping (3x: `tactical-phase-breakdown`, `precision-baselines`,
  `precision-blunder-phases`), the ply-based phase `CASE WHEN m.ply <= 12 ...` mapping
  (2x: `profile-phase-stats`, `position-eval-components`), the tactical-concept allow-list
  `IN ('fork','pin','hanging_piece','skewer','discovered_attack')` (3x), the draw-result
  literal list (3x in `profile.nu` + inverted once), and the player-color
  `CASE WHEN color = 'white' THEN g.white ELSE g.black END` lookup (`derive.nu:16` /
  `profile.nu:107`, plus `sync.nu:175,177` twice in one query). Not fixed because SQLite
  has no server-side enum/view layer here and the natural Nu-side fix — interpolating a
  shared string constant into each SQL literal via `$"...(frag)..."` — trades SQL
  readability (each query stops being valid, self-contained SQL you can read top to
  bottom) for a small reduction in copy-paste, and touches `--params` positional binding
  in several places, which is exactly the kind of query code where a mechanical
  find/replace risks a silent off-by-one. If a genuine SQL-view-based simplification is
  wanted later, it deserves its own scoped pass with its own before/after query-output
  diffing — not folded into this dedup sweep.

Verified overall: `nu-check` clean on all five touched/reviewed `.nu` files; Rust-side
`cargo check --all-targets`/`cargo test`/`cargo clippy --all-targets` all green (unaffected
by the Nu-side changes, listed for completeness since both were done in the same pass).

Re-audit 2026-07-30 (2): remaining src/*.rs command files — reusability,
reproducibility, clarity

Continued the same pass over the 15 top-level `src/*.rs` files that hadn't had this
specific lens applied yet (everything outside `src/eval/`, which was already clean).
Explicitly evaluated against four facets per the user's own framing: reusability,
reproducibility, clarity, crystal-clear expression.

**Fixed:**
- **Reusability** — `canonicalize_fen_cmd.rs`, `zobrist.rs`, and `pgn_to_fens.rs`'s
  `PgnToBatch` each hand-rolled the identical `match input_value { Value::String{..} =>
  single call; Value::List{..} => loop+push; _ => "Expected string or list of strings" }`
  dispatch around a different core function. Extracted `utils::map_string_or_list(input,
  span, f)` (`f: Fn(&str, Span) -> Result<Value, LabeledError>`); each command's `run` is
  now the one-line call plus its own conversion closure. (`pgn_to_fens.rs`'s `PgnToFens`
  keeps its own dispatch — it's a genuine one-to-many flatMap, a single PGN string expands
  to many move rows, not the same shape.) Added 4 unit tests for the new helper
  (`utils::tests`) since it had no direct coverage otherwise.
- **Reusability / drift-risk** — `process_corpus.rs:86` hardcoded the starting position's
  canonical hash as the literal `"463b96181691fc9c"` plus its FEN, duplicated from
  `core.rs`'s `pgn_to_batch_record`, which computed the equivalent value via
  `get_canonical_hash(&Chess::default())`. If the hash computation ever changed, the
  literal would silently go stale with no compiler check. Added `pub fn
  initial_position() -> (String, String)` to `core.rs` (the one place both are now
  computed) and had both call sites use it — also moved it out of `process_corpus.rs`'s
  per-game loop, where it was being recomputed once per game for no reason.
- **Reproducibility (real bug)** — `coach_derive_cmd.rs`'s `format_results` (baselines
  output) and `compute_transitions` (transition-events output) built their emitted row
  lists straight off `HashMap` iteration, whose order Rust randomizes per hasher instance.
  Identical input to `chessdb derive-coach-signals` could legitimately come back with
  `player_baselines`/`transition_events` rows in a different order on every invocation —
  harmless for correctness (SQLite storage doesn't care about insertion order, and
  `profile.nu`'s queries all have explicit `ORDER BY`), but a real reproducibility gap:
  anyone diffing raw command output across two runs of the same input would see spurious
  differences. Sorted both by key (`(player, phase_bucket, concept)` and `(state_from,
  state_to)` respectively) before building the output `Vec`. Added two regression tests
  (`baseline_output_rows_are_sorted_deterministically`,
  `transition_output_rows_are_sorted_deterministically`) using inputs whose HashMap
  insertion order would not coincidentally already be sorted.
- **Clarity** — `compute_baselines` and `detect_anomalies` each independently wrote out
  the same `[(0usize, "material"), (1, "pawn_structure"), (2, "activity"), (3,
  "king_safety")]` index-to-name list for reading `hugm_eval_arr`'s first four components.
  Named it once as `EVAL_ARR_COMPONENTS`, with a comment pointing at `process_corpus.rs`
  (where the array is actually built) and `profile.nu`'s `json_extract(...,'$[N]')` reads
  (which must stay in the same order) — doesn't eliminate the cross-language index
  contract, just stops it from also being duplicated within the Rust side.

**Deferred (documented, not fixed — real, but a schema-level change, not a local
refactor):** `process_corpus.rs` builds `hugm_eval_arr` as an 11-element positional JSON
array (material, pawn_structure, piece_activity, king_safety, passed_pawns, development,
vector_features, strategic, scaling, drawishness, override_); `coach_derive_cmd.rs` reads
the first four back by bare index, and `chessdb/profile.nu` reads three of them directly
via `json_extract(p.hugm_eval_arr, '$[1]')` etc. This is exactly the "typed struct, not
index/string-keyed bag" antipattern CLAUDE.md already calls out elsewhere — reordering the
`arr` vec in `process_corpus.rs` would silently corrupt every downstream reader with no
compiler error on the Rust side and no error at all on the SQL side. Not fixed here
because a real fix (e.g. storing `hugm_eval_arr` as a named JSON object instead of a
positional array) changes the on-disk schema of every existing `chess.db`, and touches
three files across two languages plus their `--params`/`json_extract` call sites — a
migration, not a dedup. Worth its own scoped pass, with an explicit before/after check
that `chess-profile`'s numeric output is unchanged for a real database, if the user wants
it done.
- Also lower-priority, same root cause, not fixed: `compute_baselines` and
  `detect_anomalies` re-derive nearly identical `prev_score`/`prev_eval_arr`/delta
  scaffolding (~90 lines each) — a real "one loop computing two things" opportunity, but
  entangled with the eval-component-baseline vs. anomaly-detection control flow enough
  that merging them safely would want its own dedicated pass rather than a mechanical
  extraction.

**Confirmed already clean, no findings:** `src/chess.rs`, `src/canonical.rs`,
`src/game_parse.rs`, `src/stockfish.rs`, `src/main.rs`, `src/lib.rs`, `hugm_eval_cmd.rs`
(its dispatch differs enough from the three deduped above — Rayon-parallel with
verbose/player_elo threading and fail-fast error handling — that folding it into
`map_string_or_list` would obscure more than it'd save).

Verified: `cargo check --all-targets` clean; `cargo test` green (38 tests, up from 32 —
4 new `utils::tests` for `map_string_or_list`, 2 new `coach_derive_cmd::tests` for the
ordering fix); `cargo clippy --all-targets` — confirmed via line-range diffing that every
warning still present sits outside every line this pass touched (all pre-existing,
previously documented); STS smoke test (1499-position corpus) passes.

Re-audit 2026-07-30 (3): finishing pass — nnue_eval_cmd bug, clippy zero-out,
compute_baselines/detect_anomalies dedup, doc staleness

Continued past the point of diminishing returns at the user's explicit request ("keep
cleaning it out and finish").

**Fixed — real bug, not caught by the earlier sweep:** `src/nnue_eval_cmd.rs`'s output
shape depended on *result count*, not *input shape*: `if results.len() == 1 { bare
record } else { list }` meant a **one-element list input** (e.g. `["fen"] | chessdb
nnue-eval`) silently returned a bare record instead of a one-element list — inconsistent
with the command's own declared `List-in -> List-out` signature and with every sibling
command in this codebase (which always preserve input shape regardless of element count).
Also silently dropped non-string elements from a list input via `filter_map` instead of
erroring, unlike every sibling command's `v.as_str()?`. Fixed by tracking `single: bool`
from the input's actual shape (not the output count) and erroring on non-string list
elements. Not runtime-tested end-to-end (no `stockfish` binary available in this
environment) — verified by code reading plus `cargo check`; the fix is a pure
control-flow correction with no new external dependency.

**Fixed — clippy zero-out:** ran `cargo clippy --fix` across the `src/bin/*.rs` utility
scripts that had carried known pre-existing warnings all session (left alone earlier as
low-priority): `pgn_to_jsonl.rs` (1), `lichess_to_jsonl.rs` (5, two passes — the second
`--no-deps` pass caught 2 the first missed), `hugm_harness.rs` (1 auto-fixed
`needless_borrows_for_generic_args`; 2 `needless_range_loop` warnings fixed by hand since
clippy's own suggested rewrite was syntactically malformed — `names[2..k].iter()` /
`beta[2..k].iter()` instead of indexing). The same `--fix` run also swept up the lib's
remaining two known pre-existing warnings for free (`coach_derive_cmd.rs`'s two
redundant `as i64` casts, `position.rs`'s redundant `.into()` calls and a `pawn_safe =
pawn_safe & !...` → `&=` simplification — one auto-fix left mis-indented code in
`piece_activity_score`'s rook-on-sixth branch, reformatted by hand) plus a
`concepts.rs::rank_and_annotate`'s `sort_by` → `sort_by_key(Reverse(...))` simplification
found along the way. **`cargo clippy --all-targets` now reports zero warnings across the
entire crate** — every warning documented as "pre-existing, left alone" in every prior
entry in this file is now gone.

**Fixed — the previously-deferred `compute_baselines`/`detect_anomalies` duplication:**
both functions independently replayed the identical `(player, game_id)`-keyed
`prev_score`/`prev_eval_arr` bookkeeping to compute the same per-row hugm_score delta and
eval-component deltas. Extracted `compute_row_deltas(rows, states) -> Vec<RowDelta>` (one
pass, shared by both) — `RowDelta` carries the player/game_id/ply, the row's
`StateVector`, the overall delta/signed_delta, and a `component_deltas: Vec<(name,
abs_delta, signed_delta)>` list (empty on a game's first row for that player). Preserved
one small pre-existing inconsistency deliberately rather than picking a side silently:
`compute_baselines` used to default `phase_bucket` to `1` if `states` were shorter than
`rows`, while `detect_anomalies` indexed `&states[i]` directly (would panic on the same
mismatch) — both paths are unreachable in the one real caller (`DeriveCoachSignals::run`
always builds `states` via `encode_move_states(&rows)`, guaranteeing equal length), so
unified to a single defensive `states.get(i).copied().unwrap_or_default()` (phase
defaults to `0`, not panicking) — strictly safer than the previous direct-index panic
path, and the `1` vs. `0` default divergence is noted here since it's the one observable
(if practically unreachable) behavior difference from before. Added a new regression
test, `eval_component_deltas_feed_baselines_and_anomalies` — the component-delta path
had **zero existing test coverage** before this pass (the `move_record` test helper
always set `eval_arr: None`), so this was verified correct from first principles, not
just "tests still pass."

**Docs:**
- `README.md`'s "Plugin Commands" table was missing 3 of the crate's 8 registered
  commands (`pgn-to-fens`, `canonicalize-fen`, `nnue-eval`) — added.
- Deleted `nnue.md` (confirmed with the user first): fully superseded by this file's own
  "2026-05-13 Decision" entry above, which already states the bullet-training pipeline
  and `position_encoder.rs` were removed 2026-07-30 and that this file (`NNUE_AUDIT.md`)
  is where the history lives if that work is ever revived. `nnue.md` described a
  `nuchessdb/nuchessdb.nu` entrypoint and `nnue_dataset_builder`/`dataset_builder_cmd`
  files that no longer exist anywhere in the repo — actively misleading, not merely
  outdated.

Verified: `cargo check --all-targets` clean; `cargo test` green (39 tests, up from 38 —
1 new test for the previously-uncovered eval-component-delta path); `cargo clippy
--all-targets` reports **zero warnings** crate-wide (down from the ~15 documented as
pre-existing/left-alone throughout this file); STS smoke test (1499-position corpus)
passes.

Re-audit 2026-07-30 (4): chessdb/*.nu revisit + Cargo.toml

Continued auditing at the user's request ("okay keep auditing"). Read `chessdb/db.nu`
and `chessdb/mod.nu` fully for the first time this session (previously only partially
reviewed) — both already clean (migrations are idempotent via `IF NOT EXISTS`/try-catch
`ALTER TABLE`, `move_anomalies`'s unique index makes re-derive safe, `mod.nu`'s exports
match README's command list exactly). Two real findings elsewhere:

**Fixed — `chessdb/derive.nu`'s `chess-validate` ran an N-query loop instead of one
batched query.** It read the unconsumed anomalies for `(username, game_id)`, then looped
`for id in (anomalies | get alert_id) { UPDATE ... WHERE alert_id = ? }` — one SQL
round-trip per anomaly. Since nothing else can write to `move_anomalies` between the
SELECT and the loop (single-threaded Nu script, no concurrent writer), a single `UPDATE
move_anomalies SET consumed = 1 WHERE username = ? AND game_id = ? AND consumed = 0`
(same predicate as the SELECT) marks exactly the same rows in one query. Verified against
a real seeded SQLite table (not just `nu-check`): confirmed the first call returns
`status: shut` with the correct 2 rows for `(alice, game 1)`, only those exact rows flip
to `consumed = 1` (a sibling `(alice, game 2)` row and a `(bob, game 1)` row are
untouched), and a second call on the same args correctly returns `status: open` with an
empty list.

**Fixed — small, low-value dedup:** `chessdb/sync.nu`'s `review-game` wrote the same
11-element `[0 0 0 0 0 0 0 0 0 0 0]` fallback array twice (the first-row case and the
JSON-parse-failure catch) — pulled into one `let zero_arr = [...]`. (Noted in passing:
`review-game`'s `$d | get 0` through `get 7` is a fourth consumer of `hugm_eval_arr`'s
positional-index contract, alongside `process_corpus.rs`/`coach_derive_cmd.rs`/
`profile.nu` already documented above — reinforces that deferred finding, not a new one.)

**Fixed — dead Cargo.toml config:** the explicit `[[bin]] name = "nu_plugin_chessdb" path
= "src/main.rs"` block declared exactly what Cargo's default binary-target discovery
already produces for a package named `nu_plugin_chessdb` with a `src/main.rs` (confirmed
via `cargo metadata`: identical target — same name, same path — with the block removed,
and `cargo build --bin nu_plugin_chessdb` still produces the same binary at the same
path). Removed as dead configuration.

**Reviewed, no findings:** `main.rs`/`lib.rs` (all 8 registered commands match
`README.md`'s table exactly, `PLUGIN_CATEGORY` used consistently), `stockfish.rs`
(well-tested, already carries its own extraction rationale doc comment — noted in passing
that `nnue-eval`'s score is White-relative per Stockfish's own `eval` output, not
mover-relative like the rest of this codebase's HUGM convention, but `nnue_score` isn't
consumed anywhere downstream yet, so there's no live inconsistency to fix, just something
to keep in mind if it's ever wired into a pipeline that assumes mover-relative scores),
`sf_batch_eval.rs` (already a 3-line documented stub), the `tests/*.rs` integration test
files (no cross-file helper duplication — each file's helpers are appropriately scoped
to that file, and Rust compiles each integration test as its own crate anyway).

Verified: `cargo check --all-targets` clean; `cargo test` unaffected (39 tests, still
green — none of this round's fixes touched Rust test-covered code); `cargo clippy
--all-targets` still zero warnings; STS smoke test passes; both edited `.nu` files pass
`nu-check`; `chess-validate`'s fix additionally verified against a real seeded SQLite
database (not just static checks), described above.

Re-audit 2026-07-30 (5): mate_in_1 and pawn_islands were detected but never
surfaced as coaching concepts

User asked "what are all the [concepts] detected in a FEN" — walking `SensorReport`'s
full field list (`sensor.rs`) against `extract_concepts` (`concepts.rs`) to answer that
question directly found two gaps: `sensor.mate_in_1_exists` and
`sensor.positional.pawn_islands` are both fully computed (the former a real legal-move
scan for a mate-delivering move, the latter `extract_pawn_islands`'s file-adjacency scan)
but neither was ever turned into a `Concept`, so neither ever reached `gated_issues` for a
live position — the exact bug class `concepts.rs:77`'s own comment already documents once
for `hanging_piece` ("typed data existed but this concept was never emitted before this
session"). `mate_in_1_exists` in particular only ever reached players after the fact, via
`positions.mate_in_1` → `chess-profile-mate-analysis`'s aggregate "did you find your
mates" stat — never as an in-the-moment issue for the position actually being analyzed.

**Fixed**, both in `extract_concepts`:
- `mate_in_1` (ELO 400+, the lowest gate in the system — deliberately below
  `material_imbalance`/`hanging_piece`'s 600, since spotting a forced mate is more
  fundamental than either): severity fixed at **1000**, not scaled from anything, and
  verified via a real back-rank-mate FEN (`R3K3` vs `k` boxed in by its own pawns, which
  also happens to carry a large material lead) that it actually outranks
  `material_imbalance` in `rank_issues_for_position`'s output — the first version of this
  fix used severity 200 and *lost* that ranking to a 632-severity material imbalance in
  testing, which would have been a materially misleading regression (a coach that mentions
  material before "you can mate right now" is actively bad coaching) had it not been
  caught before committing.
- `pawn_islands` (ELO 1600+, same tier as `isolated_pawn`/`doubled_pawn`): severity
  `count * 20`.
- Both added to the `confidence` match arm in **both** `rank_issues_for_position` and
  `rank_issues_for_player` (`mate_in_1` at the top 1.0 tier alongside fork/pin/skewer/
  discovered_attack/king_in_check; `pawn_islands` at the 0.7 tier alongside
  king_exposed/isolated_pawn/doubled_pawn) — these two match arms are themselves
  identical copy-pasted lists, a minor pre-existing duplication noted but not fixed here
  since it wasn't part of what was asked.

Verified beyond `cargo test`: hand-ran both new FENs through `extract_concepts` and
`rank_issues_for_position` directly (a throwaway `src/bin/_scratch_verify*.rs`, deleted
after use — not committed) before writing permanent tests, specifically to catch the
severity-ranking problem above, which unit-testing "concept exists in the list" alone
would have missed entirely. Permanent regression tests added to `tests/motif_canonical.rs`:
`mate_in_1_is_detected_and_ranks_above_material_imbalance`,
`pawn_islands_is_detected`. Full suite green (39 lib + 13 motif, up from 11), clippy
zero warnings, STS smoke test passes.

Correction (same session, minutes later): the fix above was necessary but not
sufficient — `mate_in_1_exists` was still architecturally bolted on, and that turned out
to be a live bug, not just a smell. User pushed back: "it needs the de[dup]lification,
because if the fen is ever evaluated you should be able to retrieve the mate in 1 value
as well" — i.e. fold the detection into the one shared evaluation path instead of one
caller patching it on after the fact.

Tracing it further: `build_sensor_report` computed its own `gated_issues` internally
(`position.rs`, via `extract_concepts(&partial, ...)`, where `partial` is a `SensorReport`
built *inside* the function) — and returned `mate_in_1_exists: false` unconditionally at
its very end. `analyze_fen_with_engine_score` then computed the real
`mate_in_1_exists` value itself (from the un-normalized position, deliberately, per a
comment already there) and patched it onto the *already-returned* `SensorReport`
afterward — too late to affect the `gated_issues` that had already been computed inside
`build_sensor_report`. Confirmed with a throwaway scratch binary: `analyze_fen_with_engine_score(fen, None, Some(400)).sensor_report.gated_issues` for the
same back-rank-mate FEN used in the test above did **not** contain `mate_in_1` even
after the "fix" above — only the lower-level `extract_concepts`/`rank_issues_for_position`
calls (bypassing that ordering bug) had been exercised by the first test. Also confirmed
`coach_derive_cmd.rs`'s two direct `build_sensor_report` callers (the `encode_move_states`
slow path, and its own test) got nothing at all, ever — they don't go through
`analyze_fen_with_engine_score`.

**Fixed properly**: moved the mate-in-1 detection (`chess.legal_moves().iter().any(|m|
{...c.is_checkmate()})`) into `build_sensor_report` itself, computed early and included in
`partial` (so `extract_concepts`'s internal call sees it) and in the final returned
`SensorReport` — not patched on by any caller. `analyze_fen_with_engine_score` no longer
computes or patches it at all; every caller of `build_sensor_report` gets it for free now,
matching every other sensor in the file. Verified the "computed from whichever position
frame you're given, real or eval-normalized, doesn't matter" claim in the original
(now-deleted) comment is actually true, not just asserted: fed a hand-verified Black-to-move
mirror of the same physical mate-in-1 fact (rank-flipped/case-swapped/side-flipped, which
goes through `normalize_for_eval`'s internal flip) and confirmed identical
`mate_in_1_exists`/`gated_issues` output — both via a throwaway scratch check and as a
permanent assertion added to `mate_in_1_is_detected_and_ranks_above_material_imbalance`.

Verified: full suite still green (39 lib + 13 motif — no test count change, existing
assertions in the same test strengthened rather than new tests added), clippy zero
warnings, STS smoke test passes.

Follow-on (same session): material_score's white/black closures made explicit us/them

Walking the ELO-sorted sensors for a "how is this detected" session, the user caught
`material_score`'s internal `white`/`black` closures — literal `Color::White`/
`Color::Black`, unlike every sibling function in `compute_groups`
(`pawn_structure_score`, `king_safety_score`, `development_score`), which all take an
explicit `us: Color` parameter. Traced why this was safe: `material_score` is only ever
called with an already-canonical (White-to-move) board, so literal White ≡ `us` by
construction — but that's an invariant upheld entirely by caller discipline across two
files, nowhere stated at `material_score`'s own signature.

Checked all three call sites of `compute_groups` (the only caller of `material_score`)
to confirm the invariant currently holds everywhere: `analyze_fen_with_engine_score`
explicitly calls `normalize_for_eval` (itself just `canonical::normalize_to_white_to_move`)
first; `coach_derive_cmd.rs`'s two direct calls parse `chess` from `r.fen`, which traces to
`positions.fen` — already canonical by construction (confirmed: `MoveRecord.fen` ←
`rec.get("fen")` ← SQL's bare `p.fen` in `chess-derive`'s query, and `positions.fen` is
never written except via `pgn_to_fens`'s canonical output or `core::initial_position()`).
Also checked whether `MaterialBalance`'s white/black fields (built from `material_score`'s
own `terms` map) are mislabeled when the position was flipped — they aren't:
`unflip_sensor_report` (`position.rs:2953-2957`) already explicitly swaps
`bal.white`/`bal.black` and the bishop-pair flags in that case, so the real-terms output
was never actually wrong, just the internal scoring's own expression.

**Fixed**: `material_score` now takes `us: Color` explicitly and uses `ours`/`theirs`
closures (`piece_count(board, us, ...)`/`piece_count(board, them, ...)`) throughout the
`mg`/`eg` blend and all six adjustment terms (bishop pair, rook/pawn penalty, knight/pawn
bonus, minor-vs-major comparison, redundant rook, redundant queen+rook) — matching every
sibling function's convention. Its output (`blended`, feeding `material_total.value`,
the `material_imbalance` concept's source) is now correct-by-construction relative to
whichever `us` it's given, not correct-only-because-every-caller-happens-to-pass-White.
The `terms` map (the JSON `"white_queens"`/`"black_queens"` etc. keys feeding
`MaterialBalance`) deliberately stays literal-color, untouched — that's the real-terms
output path already correctly handled by `unflip_sensor_report`'s explicit swap, a
genuinely different job from the internal us-relative scoring. Added a doc comment to
`compute_groups` stating the canonicalization precondition explicitly, since nothing
enforces it — a future caller that evaluates a real, un-normalized position directly
would still silently get every score in this file wrong, not just material.

Verified as a pure refactor, not a behavior change: full suite green (39 lib + 13 motif,
unchanged), clippy zero warnings, STS smoke test passes, and directly re-checked
`material_total.value` for three FENs (632, 0, 0) against the exact values observed
before this change — byte-identical.

Follow-on: hanging_piece value/severity, and ThreatGraph::control (phase 1 of a
larger "shared continuity map" direction)

Continuing the ELO-ladder walkthrough to `hanging_piece` (ELO 600, tied with
`material_imbalance`) surfaced a real design gap: severity was a flat `count * 60`
(`concepts.rs`) — two hanging pawns scored identically to two hanging queens. Traced the
right fix through `ThreatGraph::see`/`see_chain` (already used by `find_forks` for exact
material-consequence-of-a-capture-sequence, i.e. Static Exchange Evaluation): for a piece
with **zero defenders** (which is `find_hanging`'s entire definition), SEE has no recapture
to walk — it reduces to exactly `piece_value(role)`. So weighting by piece value isn't an
approximation of SEE for this case, it *is* SEE, just without paying for a chain walk that
would immediately terminate. (A broader "defended but still SEE-losing" detector was scoped
as a distinct, higher-ELO sibling concept — not built yet, needs its own pass once the
ladder reaches a tier where calculating a trade, not just spotting an undefended piece, is
the actual skill being taught.)

**Real-game observation that shaped the severity formula**: hanging pieces don't
necessarily get captured immediately — multiple pieces can sit hanging simultaneously and
persist over several moves if neither side notices. So severity shouldn't be pure `max`
(loses the "this is a messier, more dangerous position" signal from multiple simultaneous
threats) or pure `sum` (overstates it — only one capture happens per move; summing treats
every hanging piece as equally certain to be lost). Settled on max-anchored with a damped
weight for the rest: `severity = max_value + 0.3 * sum(remaining values)`. The single
biggest piece dominates (the honest "what's actually at risk right now" signal), but a
second or third hanging piece still meaningfully raises severity above a single-piece case
of the same size, and the phrase text carries the count/max explicitly either way (e.g.
`"black has 2 hanging pieces (biggest worth 900 centipawns)"`) so the ranking math
collapsing to one number doesn't lose the fact from the coaching text.

**Fixed**:
- `HangingPiece` (`concept_types.rs`) gained a `value: i64` field, populated in
  `find_hanging` via the same `Self::piece_value(role)` table `see_chain` already uses —
  one source of truth, no second piece-value table introduced.
- `extract_concepts`'s `hanging_piece` handling replaced the `count_and_push_by_color`
  call with the max-anchored formula above (doesn't fit that helper's flat-weight shape,
  same reason `doubled_pawn` already has its own manual loop).
- Verified with a hand-built position (queen worth 900 + knight worth 320 hanging
  simultaneously, cross-checked to confirm neither piece defends the other and the king
  isn't adjacent to either): severity came out to exactly `900 + 0.3*320 = 996`, matching
  the formula precisely. Locked in as a permanent test,
  `hanging_piece_severity_is_anchored_on_the_biggest_at_risk`.

**Also added (phase 1 of a larger, explicitly-scoped-but-not-yet-executed direction)**:
`ThreatGraph::control(sq, color) -> i32` — net attacker-count differential at a square,
built entirely from `attackers_to` (already computed once per position). Motivated by
recognizing that `hanging_piece` and `detect_outposts` are both really asking the same
underlying question — "whose continuity does this square belong to" — but currently
answer it two different ways: `find_hanging` reads the shared `ThreatGraph.attackers_to`
`hanging_piece` already sits on; `detect_outposts` (`position.rs:2102`) computes its own
separate, narrower `pawn_attack_mask` (pawns only, not the full attack picture) from
scratch, even though `ThreatGraph` has *already been built* by the point
`build_sensor_report` calls `detect_outposts` — it's just never threaded through.
`king_ring` (king safety) has the same shape: a third, independently-computed attack
zone. Confirmed with shakmaty's own source (`position.rs:442-567` in the shakmaty crate)
that this isn't "moving outside shakmaty" — `king_attackers` (which shakmaty's own
`checkers()`/`is_check()` are built from) is *itself* just `board.attacks_to(...)`, the
same primitive `ThreatGraph` already calls; the generalization is reusing one whole-board
computation of a shakmaty primitive instead of each detector calling a narrower one
separately.

**Scoped, not yet done**: migrating `detect_outposts` onto `graph.attackers_to`/`control`
(needs `graph: &ThreatGraph` threaded into its signature, and a direct before/after
numeric check on real outpost positions, same discipline as the `material_score`
refactor); `king_ring`/king safety migration scoped as lower-priority and needing its own
look, since king safety scoring does more than pure attacker-counting. `in_check` also
still calls `chess.is_check()` independently rather than reading `graph.attackers_to` at
the king square (a live, previously-identified duplicate-computation instance of the same
pattern) — not yet fixed either.

Verified: `cargo check --all-targets` clean; `cargo test` green (39 lib, unchanged — the
`control` method has no callers yet so nothing exercises it directly; 14 motif tests, up
from 13, new test above); `cargo clippy --all-targets` zero warnings; STS smoke test
passes.

Phase 3: detect_outposts migrated onto the shared ThreatGraph substrate

Added `ThreatGraph::attackers(sq, color) -> Bitboard` (`attackers_to[sq]` masked to one
color — the piece-level counterpart to `control`'s differential). `detect_outposts`
(`position.rs:2102`) now reads `graph.attackers(...)` for both its "not attackable by
enemy pawns" check and its "supported by own pawn / fallback: any other piece" checks,
instead of its own separate `pawn_attack_mask`/`board.attacks_to` calls — same primitive
family `find_hanging` already reads, per the "continuity map" direction scoped in the
entry above.

**Real complication found while doing this, handled deliberately**: `detect_outposts` is
called from *two* places — `build_sensor_report` (which already has a `ThreatGraph`) and
`compute_groups` (the legacy scoring engine, which had never built one). Rather than give
`detect_outposts` two code paths (graph vs. no-graph) or leave it inconsistent between
call sites, `compute_groups` now builds its own `ThreatGraph` too — a real, deliberate
tradeoff (one extra O(64) graph build per evaluation) in exchange for one detector
implementation instead of two. Documented in a comment at the build site that
`compute_groups` and `build_sensor_report` don't yet share a single graph across one
evaluation — a further consolidation, scoped but explicitly not attempted here.

**Found in passing, confirmed pre-existing (not introduced by this change)**: the
"fallback: supported by any other piece" branch has never actually recorded *which*
piece defends the outpost — it pushes a literal `Square::E1` placeholder regardless of
where the real defender is (confirmed via `git diff`: this line was already there,
unchanged by this migration). A test position with a real piece incidentally placed on
e1 will show a misleading `supported_by` in the typed output. Not fixed — separate,
minor, pre-existing bug, noted for whenever positional-detector accuracy gets its own pass.

Verified as a pure refactor via the same discipline as the `material_score` change:
`git stash`'d this change, ran `analyze_fen_with_engine_score` on 5 real positions
(including one with a genuine detected outpost, `final_score=148, outposts_us=1`) against
the pre-migration code, `git stash pop`'d, reran the identical positions — byte-identical
`final_score` and `outposts_us`/`outposts_them` on every one. Also hand-verified all four
outpost branches individually (pawn-supported, non-pawn-supported/fallback,
attacked-by-enemy-pawn/correctly rejected, undefended/correctly rejected) against known
chess facts. Full suite green (39 lib + 14 motif, unchanged counts — this phase added no
new tests since it's a pure internal refactor, verified by the A/B numeric comparison
instead), clippy zero warnings, STS smoke test passes.

Phase 4: in_check migrated onto the shared substrate too

`ThreatGraph::is_in_check(color) -> bool` added — finds `color`'s king square from
`self.kings` and checks `attackers(king_sq, color.other()).any()`. This is exactly
shakmaty's own `checkers().any()` (proven from the shakmaty source in the entry above:
`king_attackers` there is literally `board().attacks_to(...)`, the same primitive
`attackers_to` is built from), just read from the graph already built for this position
instead of a second, separate shakmaty call. `compute_groups` and `build_sensor_report`
both used to call `chess.is_check()` independently — both now call `graph.is_in_check(us)`
using the graph each already builds (`compute_groups`'s graph build was reordered earlier
in the function so it's available before `in_check` is computed).

Left `analyze_fen_with_engine_score`'s `LegalInfo.is_check: chess.is_check()`
(`position.rs:3110`) alone, deliberately — it's grouped with `is_checkmate`/
`is_stalemate`/`is_insufficient_material`, all of which genuinely need shakmaty's full
legality engine, on the *real* (un-normalized) `chess`, for which no `ThreatGraph` exists
at that point in the function. Pulling just `is_check` out of that cluster for a marginal,
non-redundant gain (there's nothing already-built to reuse there) would fragment a
correctly-scoped block for no real benefit.

Verified with the same A/B discipline as the two changes above, this time specifically
targeting the highest-risk case (positions where the side to move genuinely is in check,
since a wrong `in_check` would corrupt `king_safety_score` and the `king_in_check`
concept): `git stash`'d, ran 4 positions (two in-check — one via a queen giving check
directly, one via discovered-style open-file check — two not) against the pre-migration
code, `git stash pop`'d, reran — `in_check`, `king_safety.blended`, and `final_score`
byte-identical on all 4, including both in-check cases. Full suite green (39 lib + 14
motif, unchanged), clippy zero warnings, STS smoke test passes.

Remaining known instance of this pattern, not yet touched: `king_ring`/king safety
scoring — scoped as its own look (not a drop-in migration) since king safety does more
than pure attacker-counting.

Phase 5: ThreatGraph::zone_control — the zone-level generalization king_ring actually needs

User's observation: `king_ring` isn't a single-square continuity question like the three
migrated above — it's a *zone* (the king's square plus its 8 neighbors), and the natural
generalization of `control()` is "sum it over every square in the zone," not a single
lookup. Checked how `king_ring` is actually consumed before building anything:
- `king_safety_score` (`position.rs:895`) doesn't use `king_ring` at all — it separately
  calls `board.attacks_to(king_sq, ...)` on just the king's own square (weighted by
  piece type), the same primitive/redundancy pattern as the now-fixed `in_check`, plus an
  unrelated pawn-shelter/storm computation.
- `piece_activity_score` (`position.rs:1045`) is the only actual consumer of `king_ring`,
  and only as a per-piece *existence* check (`atk & king_ring_bb != EMPTY`, "does this
  piece's attack pattern touch the ring around its own king" — a defensive-coverage
  bonus), never a control sum.

Neither is "zone control" yet — both are narrower, different questions. Added
`ThreatGraph::zone_control(zone: Bitboard, color: Color) -> i32` (`control` mapped over
every square in `zone`, summed) as a pure, additive primitive — **not** wired into
`king_safety_score` or `piece_activity_score` in this pass, since both are tuned,
weighted scoring formulas where a change needs its own dedicated validation (like
`material_score`'s A/B check, but for a formula that doesn't yet exist), not a
drop-in swap the way `hanging_piece`/`detect_outposts`/`in_check` were.

Added direct unit tests in a new `threat_graph.rs` test module (none existed before):
`control_and_attackers_agree_with_a_direct_recount` (validates `control`/`attackers`
against an independently-recomputed `board.attacks_to` count, not the graph's own logic,
across all 64 squares of a real position), `zone_control_sums_control_over_every_square_in_the_zone`
(validates the sum against an independent per-square loop, plus an exact hand-verified
value — first attempt at the hand-derivation was wrong, caught immediately by the test
failing, corrected to the right value: 3, not a guessed "should be negative"),
`is_in_check_matches_shakmatys_own_is_check` (cross-checks `ThreatGraph::is_in_check`
against `chess.is_check()` directly, for both an in-check and a not-in-check position).

Verified: `cargo check --all-targets` clean; `cargo test` green (42 lib tests, up from 39
— all three new; 14 motif, STS, integration tests unchanged since this phase touched no
scoring formula); `cargo clippy --all-targets` zero warnings; STS smoke test passes.

Confirmed: `control` is one shared per-square map, not two independent ones

User's observation, checked and proven rather than just agreed with: `control(sq,
White)` and `control(sq, Black)` aren't two separately-meaningful quantities that happen
to correlate — they're exact negatives of each other on every square, on every position,
by construction (swapping `color` in `control`'s own body swaps which count is `ours` vs
`theirs`). So the one real per-square fact is a single signed number
(`white_attackers[sq] - black_attackers[sq]`); `control(sq, color)` is that number read
with a sign flip depending on whose question is being asked — `attackers_to[sq]` was
never "White's map" and "Black's map," it's one shared bitboard both queries split
differently. This is the same convention every other score in this file already follows
(`material_total.value`, `king_safety.blended`, `development.blended` — single signed,
us-relative numbers, not two separate positive-for-each-side numbers), the same
convention the `material_score` phase enforced explicitly. `control` already followed it;
this makes explicit *why* it has to, and (since `zone_control` is just `control` summed)
the same antisymmetry propagates to zone_control for free.

Added `control_is_one_shared_map_not_two_independent_ones`: asserts
`control(sq, White) == -control(sq, Black)` for every square across 3 different real
positions (not just the one simple test position used elsewhere), including a real
middlegame position. Full suite green (43 lib tests, up from 42), clippy zero warnings,
STS smoke test passes.

Found, verified, deferred: `ThreatGraph::see_chain` gives wrong answers for 2+ step
exchanges (real bug, not yet fixed)

Chasing a clean example of "control is a cheap count, not a value, so it can disagree
with the real exchange result" (user's question, answered above) led to hand-verifying
`see()`'s actual output against ground truth, which didn't match.

**Position**: `k7/8/3p4/4n3/8/8/4Q3/4K3 w - - 0 1` — White queen e2, black knight e5
(defended once, by a black pawn on d6). `graph.see(Square::E5, Color::White)` returns
**+220**. Ground truth by hand: White captures the knight (+320), black recaptures with
the pawn, capturing White's *queen* (−900) — net **−580**, a clearly bad trade for
White. The two numbers aren't close; this isn't a rounding/edge-case disagreement.

**Root cause, located precisely**: in the recapture loop (`threat_graph.rs`, inside
`see_chain`), each step computes `let val = Self::piece_value(best_role);` where
`best_role` is the role of the piece *making* that recapture (e.g. the pawn, 100) — but
the value that should enter the running total at that step is the value of whatever's
*being captured* (which is always the previous side's piece that just moved there — here,
White's queen, 900). The code prices each step by who's swinging, not by what's on the
board being taken. Correct only for the first step (initial victim), where the two
coincide by construction; wrong from the second step on whenever the capturing piece's
own value differs from what it's capturing — which is most real positions.

**Also separately incomplete even once that's fixed**: the standard SEE "swap-off"
algorithm requires a backward minimax pass after the chain is walked (a rational side
stops recapturing once it's no longer favorable, rather than being forced to complete
every physical capture that exists) — `see_chain` has no such pass, so even with the
`val` computation corrected it would still describe "what happens if both sides greedily
capture to the end," not real best play. Confirmed this is a second, separate gap by
noting `delivers_check`'s "any check cancels the chain" logic has the same flavor of
issue in the position tested first (`.../4n3/8/8/4Q3/4K3` with the black king left on
e8): capturing the queen with the pawn *also* resolves the discovered check the queen's
own capture created, but the code can't distinguish "check I must answer some other way"
from "check I'm about to answer by continuing the very capture in progress," and bails
out of the chain either way.

**Why not fixed now**: this is `ThreatGraph::find_forks`'s `evaluated_forks` material
consequence — real, coaching-relevant production code, and a correct fix needs the actual
swap-off algorithm (backward pass included), not a one-line patch. Deferred as its own
scoped piece of work, not folded into the "continuity map" primitives thread this pass
was about — confirmed with the user that this is architecturally a separate layer (SEE
prices an exchange; `control`/`attackers`/`zone_control` never touch piece values at all,
so nothing built in phases 1-5 is affected by this bug). `find_forks` is the only place
`see`/`see_chain` currently feeds live output (`sensor.evaluated_forks`) — not yet
consumed by `hanging_piece`'s severity (uses `piece_value` directly, exact for the
zero-defender case per the earlier finding, no chain-walking involved) or anything from
this pass.

Found, verified, deferred: two more findings surfaced walking the ladder to fork/pin/skewer

**Fork threshold divergence between the legacy and typed detectors.** `detect_forks`
(`position.rs:1415`, inside `tactical_score`/`compute_groups`, feeds only the legacy
`tactical_total.blended` score and verbose-text examples) and `ThreatGraph::find_forks`
(`threat_graph.rs:202`, feeds the typed `sensor.tactical.forks`/`evaluated_forks` and the
`fork` coaching concept) are two independent scans of the same board answering the same
question, deliberately (confirmed `TacticalRaw` has no `fork_ex_us`/`fork_ex_them`
fields — unlike pins/skewers/discovered, which *are* cached and shared — so this isn't
leftover dead code, it's two intentionally separate consumers). But their thresholds
disagree: `detect_forks` accepts `sum >= val_rook OR attacked_pieces.count() >= 3`;
`find_forks` requires `targets.len() >= 2 AND total_val >= piece_value(Rook)` with no
count-based escape hatch. A piece forking three pawns (300cp total, below the 500cp rook
threshold) counts as a fork in the engine's own numeric score but not in the typed
sensor or the `fork` coaching concept — the number and the player-facing text can
silently disagree about whether a fork exists. Not fixed: touches the legacy scoring
engine's tuned weights, same caution `material_score` needed.

**`detect_skewers` hand-rolls ray-walking instead of reusing shakmaty's own primitive.**
`detect_pins` (two lines earlier in the same file) computes an enemy slider's attack
pattern via shakmaty's own occupancy-aware `attacks::rook_attacks`/`attacks::bishop_attacks`.
`detect_skewers` (`position.rs:1452`) solves an adjacent problem (two enemy pieces on one
ray) by manually stepping `cur_file.offset(df)`/`cur_rank.offset(dr)` one square at a
time in each of 8 hand-coded directions, checking occupancy itself — never calling
`attacks::rook_attacks`/`bishop_attacks` at all, the same primitive sitting right there
in the same function's sibling. Same "reimplements what shakmaty already computes"
pattern as the `pawn_attack_mask`/`in_check` findings fixed earlier in this thread, just
not yet migrated. Not fixed: a real behavior-preserving rewrite here needs the same A/B
verification discipline as the earlier migrations, not a quick pass.

Phase 6: king_safety_score's own attackers_to call migrated too

Deliberately steered away from the `detect_skewers` rewrite (a genuine algorithm change,
not a primitive swap — different risk profile from everything else in this thread) and
the `see_chain` fix (explicitly deferred, will likely be redesigned before it's picked
back up) in favor of staying in the safe, already-proven pattern: `king_safety_score`
(`position.rs:895`) still had its own `board.attacks_to(king_sq, color.other(),
board.occupied())` call — the exact same redundancy shape as `in_check`, just not yet
migrated when that phase happened. Added a `graph: &ThreatGraph` parameter and replaced
it with `graph.attackers(king_sq, color.other())`.

Verified with the same A/B discipline as every migration in this thread, this time
specifically including positions with real attackers on the king (the exact code path
changed — a position with zero king attackers wouldn't exercise this line at all):
`git stash`'d, ran 4 positions (two with real queen/rook pressure on a king, two without)
against the pre-migration code, `git stash pop`'d, reran — `king_safety.blended` and
`final_score` byte-identical on all 4. Full suite green (43 lib tests, unchanged — pure
refactor, no new test needed beyond the A/B check), clippy zero warnings, STS smoke test
passes.

Remaining known instances of the "reimplements a primitive instead of reusing
attackers_to/ThreatGraph" pattern, still not migrated: `piece_activity_score`'s
`king_ring` existence checks (a different question — zone overlap, not single-square
control — `zone_control` exists for this now but hasn't been wired in), and
`detect_skewers` (a genuine algorithm rewrite, not a drop-in swap, deliberately not
attempted in this pass).

Phase 7: systematic sweep for the same pattern found two more duplicated pawn-attack
computations (no ThreatGraph needed for these — just an existing function un-reused)

Went looking systematically (grepped every `board.attacks_to`/`board.attacks_from` call
site in `position.rs`, ~14 total) rather than continuing to stumble onto instances one at
a time, and classified each by whether it uses the *current*, unmodified board occupancy
(a safe swap) versus a modified one like `detect_pins`'s `occ_minus` (a genuinely
different computation, not swappable).

Found: `king_safety_score`'s own pawn-shelter loop and a separate `mobility_mask`
function (`position.rs:1037`, feeding `piece_activity_score`'s mobility scoring) each
hand-rolled their own "union of this color's pawn attacks" via a
`for sq in pawns { mask |= board.attacks_from(sq) }` loop — the exact same computation
`pawn_attack_mask` (`position.rs:319`, already used elsewhere in the file) already
provides as a named function. Verified precisely before touching anything, not assumed:
read shakmaty's own `Board::attacks_from` → `attacks::attacks` dispatch
(`attacks.rs:141-150`) and confirmed `Role::Pawn` routes to `pawn_attacks(color, sq)`
*unconditionally*, ignoring the `occupied` parameter entirely (pawn attacks aren't
blockable) — so `board.attacks_from(pawn_sq)` is provably, not just plausibly, identical
to `attacks::pawn_attacks(color, sq)`, which is exactly what `pawn_attack_mask` computes.
Both loops replaced with direct calls to the existing function — not a new primitive,
just stopping two more places from re-deriving what already has a name.

Verified with the same A/B discipline: `git stash`'d, ran 5 positions (covering real king
pressure, real piece mobility, the starting position, a real middlegame) against the
pre-migration code, `git stash pop`'d, reran — `king_safety.blended`,
`piece_activity.blended`, and `final_score` byte-identical on all 5. Full suite green (43
lib tests, unchanged), clippy zero warnings, STS smoke test passes.

Remaining classified-but-not-yet-migrated candidates from the same sweep, for whenever
this thread resumes: `development_space_score` (`position.rs:977`, two
`board.attacks_to(king_of(...), ...)` calls with unmodified occupancy — same shape as the
`king_safety_score`/`in_check` fixes, needs `graph` threaded through it and its one
caller `extract_development_info`), `piece_activity_score`'s per-piece mobility
`board.attacks_from(sq)` calls (lines ~1274-1283, occupied squares, unmodified occupancy
— same shape, needs `graph` threaded through `piece_activity_score` itself), and
`detect_forks` (`position.rs:1424`, the already-flagged legacy fork detector — its own
`board.attacks_from(sq)` could read `graph.attacks_from` too, but this function is
already queued behind the fork-threshold-divergence finding, not worth touching twice
separately). Several other call sites in the original 14 (`position.rs:1593`, `2170`,
etc.) use modified occupancy or serve genuinely different purposes — not swap candidates,
excluded from this queue.

Phase 8: development_space_score migrated

`development_space_score` (`position.rs:976`) had two `board.attacks_to(king_of(...),
..., board.occupied())` calls — same shape as `king_safety_score`/`in_check`, unmodified
occupancy, safe swap. Took a `graph: &ThreatGraph` parameter and read
`graph.attackers(...)` instead. This one had two callers needing the graph threaded
through, not one: `compute_groups` (already had its own graph in scope) and
`extract_development_info` (`position.rs:1887`, called only from `build_sensor_report`,
which already had its own graph too) — `extract_development_info` itself gained a
`graph` parameter to pass through.

Verified with the same A/B discipline: `git stash`'d, ran 4 positions (starting position,
a real middlegame with real development imbalance — `development=202`, not a trivial
zero case — an early-development position, an endgame with none of this feature active)
against the pre-migration code, `git stash pop`'d, reran — `development.blended` and
`final_score` byte-identical on all 4. Full suite green (43 lib tests, unchanged), clippy
zero warnings, STS smoke test passes.

Phase 9: piece_activity_score's per-piece mobility counters migrated — queue cleared

Added `ThreatGraph::attacks_from(sq) -> Bitboard` (a named accessor for the
`attacks_from` array field, mirroring `attackers()`, so external callers don't need the
private `idx` helper). Confirmed a field and a method can share a name in Rust without
ambiguity (indexing syntax `self.attacks_from[i]` unambiguously means the field) —
verified by compiling, not assumed.

`piece_activity_score`'s four mobility-counter loops (knight/bishop/rook/queen,
`position.rs:1271-1281`) each called `board.attacks_from(sq)` once per piece — same
shape as everything else in this thread, occupied squares, unmodified occupancy, safe
swap to `graph.attacks_from(sq)`. Took a `graph: &ThreatGraph` parameter (single
function, two call sites, both already inside `compute_groups` with a graph in scope —
simpler than the two-caller cases earlier in this thread).

Verified with the same A/B discipline: `git stash`'d, ran 4 positions (starting position,
two real middlegames with genuinely active pieces — `piece_activity=-37` and `108`, not
trivial — an endgame) against the pre-migration code, `git stash pop`'d, reran —
`piece_activity.blended` and `final_score` byte-identical on all 4. Full suite green (43
lib tests, unchanged), clippy zero warnings, STS smoke test passes.

That clears every candidate queued from phase 7's systematic sweep except `detect_forks`
(deliberately left bundled with the fork-threshold-divergence finding rather than touched
twice) and `detect_skewers` (a genuine algorithm rewrite, not a primitive swap — its own,
separately-scoped piece of work). Production functions now reading from the shared
`ThreatGraph` continuity map: `hanging_piece`, `detect_outposts`, `in_check`,
`king_safety_score`, `development_space_score`, `piece_activity_score` — six, up from
zero at the start of this thread.

Design principle: pathfind the graph, don't calculate the exchange

Stated explicitly by the user and binding for everything downstream of this point:
**HUGM detects and cross-references patterns; it does not search or calculate exchanges
precisely — that's the real engine's job.** The reasoning given: a real engine
integration is coming later, and this system isn't meant to compete with it — it's
closer to a fast, approximate "does this look right" signal (the NNUE comparison) than a
calculator. This directly changes how the deferred `see_chain` bug should eventually be
handled: the fix is correcting the wrong operand (price what's captured, not who's
capturing), *not* adding the standard algorithm's backward-minimax refinement (whether a
rational side actually keeps recapturing) — that refinement is exactly the "brute-force
precision" this system is deliberately not chasing.

The concrete design direction that follows from the principle: several tactical patterns
that look like they need calculation actually reduce to *structural* queries against the
`attackers_to` graph already built — noticing when two already-known facts overlap,
rather than simulating anything. Two instances, both scoped here:

**1. Overload** — the mirror image of a fork. A fork is one piece attacking 2+ enemy
targets; overload is one piece being the *sole* defender of 2+ of its own side's
currently-attacked pieces. If that piece is captured, distracted, or pinned, everything
it was solely covering becomes as undefended as a zero-defender hanging piece today.
Pure `attackers_to` lookup, no cross-referencing another detector needed: for each of
`color`'s pieces `D`, find every other piece `T` (of the same color, currently attacked
by the enemy) where `D` is `T`'s *only* defender; if `D` is sole defender for 2+ such
`T`, `D` is overloaded.

**2. False defense (pin cross-reference)** — a piece can pass `hanging_piece`'s
zero-defender check and still not really be defended, if its only defender is pinned.
Real subtlety, checked before implementing rather than assumed: a pinned piece *can*
still legally recapture if the recapture square lies on the same line as the pin itself
(pin restricts moving *off* the attacker–king line, not moving *along* it) — so "defender
appears in the pins list" alone isn't sufficient; needs an actual collinearity check
(`attacks::ray(pin.attacker, pin.shielded)` contains the target square) or it will
produce real false positives in a common pattern (recapturing along the same file/
diagonal the pin sits on). Scoped for the immediate next pass, not yet implemented in
this one — starting with the simpler, self-contained `overload` first and verifying it
completely before taking on the collinearity subtlety.

Both are explicitly *not* search: no move is simulated, no position is played out, no
exchange is priced beyond reusing each target's already-known `piece_value`. Just
noticing what the graph already implies.

Implemented: overload — the first "pathfind the graph" concept

**New**: `ThreatGraph::find_overloaded(color) -> Vec<Overloaded>`. For each of `color`'s
pieces `D`, check every other same-color piece `T`: if `T` is currently attacked by the
enemy and `D` is `T`'s *sole* defender (`attackers_to[T] & own_color` has exactly one
member and it's `D`), `T` goes in `D`'s `critical_for` list. `D` is overloaded once
`critical_for.len() >= 2`. `critical_value` (sum of `critical_for`'s piece values) is
computed right there, where the real `Role` enum is on hand — same reasoning as
`HangingPiece.value`, not re-derived from `PieceRef.role` strings later.

New types: `Overloaded{piece, critical_for, critical_value}` (`concept_types.rs`),
`TacticalReport.overloaded: Vec<Overloaded>` (`sensor.rs`), wired into
`build_sensor_report` (both colors, same pattern as `find_hanging`) and
`unflip_sensor_report` (both `piece` and every entry in `critical_for` need the
color/square correction, not just the first — verified explicitly, see below). New
concept in `extract_concepts`: `"overloaded"`, ELO 1400 (needs the same "what happens if
I remove this piece" coordination insight as `discovered_attack`, which sits at the same
tier), severity = `critical_value`, confidence tier 0.8 (alongside
`rook_open_file`/`outpost`/`development` — a real vulnerability but one step short of a
forcing tactic, since the opponent still has to actually target both pieces to cash it
in, unlike `fork`/`pin`/`skewer` at the top tier).

Verified precisely before trusting it, same discipline as everything else in this
thread — not just "compiles and tests pass":
- Hand-built a position (`rr2k3/8/8/1R6/R7/2N5/8/4K3 w - - 0 1`) where a White knight on
  c3 is provably the sole defender of two rooks (a4 via the open a-file, b5 via the open
  b-file) — checked by hand that rooks don't defend each other diagonally, so the knight
  is genuinely the only common link, not a construction artifact. Confirmed detected,
  `critical_value = 1000` (500+500).
- Negative case: added a second defender for one of the two targets (a rook on b1 also
  covering b5) — confirmed the knight drops out of the overloaded list entirely, since
  it's now sole-responsible for only one target (the `>= 2` threshold is the actual
  definition, not "defends something").
- Flip-invariance: hand-built the Black-to-move mirror of the identical physical fact and
  confirmed the mirrored knight reports as `Side::Black` at real square `c6` (not
  White/c3), and every `critical_for` entry — not just checked in aggregate, each
  square/color individually asserted — comes back in real terms too. This is exactly the
  kind of bug `unflip_sensor_report` would produce if the `critical_for` loop were
  forgotten (easy to do, since it's a `Vec` nested inside the flipped struct, not a single
  field).

All three verifications promoted to permanent tests: `overloaded_piece_is_detected`,
`single_responsibility_is_not_overloaded`, `overloaded_piece_survives_the_flip_with_real_terms`.

Verified: `cargo check --all-targets` clean; `cargo test` green (43 lib unchanged, 17
motif tests up from 14 — all three new); `cargo clippy --all-targets` zero warnings; STS
smoke test passes.

**Not yet implemented**: the false-defense (pin cross-reference) design from the entry
above — scoped, including the collinearity subtlety, but not started. Natural next step
in this thread.

Implemented: false_defense — and caught a real geometry bug before it ever ran

**New**: `ThreatGraph::find_false_defense(color, pins: &[Pin]) -> Vec<FalseDefense>`. For
each of `color`'s attacked-but-defended pieces (nonzero raw defender count, so
`find_hanging` doesn't touch them), checks every defender: is it in the `pins` list's
`pinned` set, and if so, is the defended square on that pin's legal recapture line? Only
if *every* defender is pinned-and-off-line does the piece count as falsely defended.
Takes `pins: &[Pin]` as a parameter rather than detecting them itself — `ThreatGraph`
doesn't know about pins (see this file's own module doc); this is a genuine
cross-reference between two independently-computed facts, the first instance of that
shape in this thread rather than a self-contained graph query like `find_overloaded`.

**Caught before ever running a test, by re-deriving the chess rule from first
principles instead of trusting the first implementation**: the initial version used
`attacks::ray(attacker, king)` for "is this square on the pin line" — wrong.
`attacks::ray()` returns the *infinite* line through both points, extending past both
endpoints in both directions. A pinned piece can only legally move to squares *strictly
between* the attacker and its own king, or capture the attacker itself — moving to a
square beyond the king, on the far side from the attacker, exposes the king exactly as
much as moving off the line entirely (confirmed by tracing a concrete example by hand:
a rook pinned on a file, moved past its own king to the far side, no longer shields it
at all). Fixed to `attacks::between(attacker, king) | Bitboard::from(attacker)` — the
correct legal-squares primitive — before writing a single test against it.

**A second, more interesting thing verified while designing the test**: tried to
construct a position where a pinned defender *could* legally recapture on-line (to prove
the collinearity check's positive branch actually works), and discovered it's
architecturally impossible given this codebase's `detect_pins` implementation. `detect_pins`
requires the *entire* segment between attacker and king to be clear except for the one
candidate blocker (it checks `before`/`after` attack patterns with that whole segment's
real occupancy) — so any third piece sitting anywhere on the pin line prevents the pin
from being detected at all, before the collinearity question can even arise. Verified this
directly, not just reasoned about it: built a position with a third piece deliberately
placed on the segment and confirmed `detect_pins` reports zero pins for it. So the
`between`-based collinearity check is correct and worth keeping (defensive, and would
matter if `detect_pins` is ever generalized to x-ray/relative pins), but its positive
branch is currently unreachable through this codebase's own pin detector — not a bug,
just a fact about the current system worth recording so it isn't mistaken for untested
dead code later.

Verified with three hand-built positions (same discipline as `overloaded`): a real
false-defense (`Ba5` pins `Nc3` to `Ke1` along the `a5–e1` diagonal; `Nc3` is `Rb1`'s only
defender, but `b1` isn't on that diagonal — flagged, `value: 500`), an unpinned-defender
negative (remove the pinning bishop, keep the attacking rook — correctly not flagged), and
a mixed-defenders negative (add a second, unpinned defender alongside the pinned one —
one real defender is enough, correctly not flagged). Flip-invariance verified the same
way as `overloaded` — hand-built the Black-to-move mirror, confirmed both the
falsely-defended piece and `pinned_defenders` come back in real terms, not just the
piece's own field.

New concept `false_defense` (ELO 1600 — needs pin recognition at 1200 plus the extra
inferential step that changes the picture; confidence tier 0.8, alongside `overloaded`).
All five verifications promoted to permanent tests:
`false_defense_is_detected_when_the_only_defender_is_pinned_off_line`,
`unpinned_defender_is_not_a_false_defense`,
`one_real_defender_among_several_is_enough_to_not_be_false_defense`,
`false_defense_survives_the_flip_with_real_terms`.

Verified: `cargo check --all-targets` clean; `cargo test` green (43 lib unchanged, 21
motif tests up from 17 — four new); `cargo clippy --all-targets` zero warnings; STS smoke
test passes.

Both concepts scoped in the "pathfind the graph" design entry are now implemented. This
thread (mate_in_1 → material_score → hanging_piece → ThreatGraph continuity primitives →
six migrated production functions → overload → false_defense) has been building
continuously since the ELO-ladder walkthrough began; nothing in it is committed yet.

Completeness gap found and closed: outnumbered

User's question, "is this complete?", answered precisely rather than just agreed with:
no — `find_hanging` (zero defenders) and `find_false_defense` (defenders exist but are
all pinned-and-ineffective) leave a real gap between them: a piece with *some* real,
unpinned defenders where attackers simply outnumber them (e.g. 2 attackers, 1 defender)
isn't caught by either. That's `control(sq, color) < 0` on an occupied square — the
single most direct application of the `control` primitive built first in this whole
thread, skipped over in favor of the two more elaborate cross-references. No good excuse
beyond: built bottom-up (migrate existing code, then pivot to new patterns), and reached
for the flashier compound patterns before the plain direct one.

**New**: `ThreatGraph::find_outnumbered() -> Vec<Outnumbered>` — same calling convention
as `find_hanging` (no color parameter, single pass over `Square::ALL`, not
`find_overloaded`/`find_false_defense`'s per-color-call shape), since it's structurally
`find_hanging`'s closest sibling: same per-square attacker/defender count check, just
`>` instead of `== 0`. `Outnumbered{piece, attacker_count, defender_count, value}`. New
concept `outnumbered`, ELO 800 — between `hanging_piece` (600, no defenders at all) and
`fork` (1000, needs seeing a double-attack pattern) — confidence tier 0.7 (lower than
`hanging_piece`'s 0.9, since piece values could still make the actual trade fine; that's
exactly the pricing question this system deliberately doesn't calculate).

Verified with the same discipline as `overloaded`/`false_defense`: a hand-built position
(white pawn d4, attacked by `Rd8` open-file and `Ba7` diagonal = 2 attackers, defended
only by `Rd1` = 1 defender) confirmed detected; two negative cases (2-attackers-2-defenders,
and 1-vs-1 with the second attacker removed) confirmed *not* flagged — "outnumbered"
means strictly more attackers than defenders, not "is attacked at all"; flip-invariance
confirmed with a hand-built Black-to-move mirror. Four new permanent tests:
`outnumbered_piece_is_detected`, `equal_attacker_defender_count_is_not_outnumbered`,
`outnumbered_piece_survives_the_flip_with_real_terms`.

Also confirmed, precisely rather than by assumption: pin/skewer/fork's relationship to
`control` is not uniform. Fork (`attacks_from[sq]`, one piece's reach) and skewer
(alignment of two enemy pieces on one ray) are sibling queries over the same graph data,
not derived from `control` specifically. Pin genuinely is a `control`-derivative hiding
in plain sight — "does removing this piece flip control of my king's square from safe to
attacked" is exactly what `detect_pins`'s before/after `rook_attacks`/`bishop_attacks`
comparison already computes, just not through `ThreatGraph.control()` (pins are "not
built from this graph at all," per this file's own module doc). A
`control_if_removed(sq, color, removed_sq)` primitive would make that relationship
explicit and could genuinely replace `detect_pins`'s implementation — scoped as a future
direction, not started.

Verified: `cargo check --all-targets` clean; `cargo test` green (43 lib unchanged, 24
motif tests up from 21 — four new); `cargo clippy --all-targets` zero warnings; STS smoke
test passes. `threat_graph.rs`'s module doc updated to describe `find_outnumbered`
alongside `find_hanging` as the two direct `control` applications.

## 2026-07-30: the failure lattice — `false_safety`, and reporting every rung to the database

User's framing, verbatim, after the `outnumbered`/`overloaded`/`false_defense` work above:
"I feel like it is a 'threat' with a ladder of validation. Sure its under attack, but is
it defended, and if it is how well is it defended vs attacked, and if there is overlap are
any of the involved pieces committed to protecting the king (pin) or another piece
(overload), and later into positional guides like center or flanks... it is increasingly
nuanced and strategic all the way up." Then, asked to consolidate, document, and make sure
the database can report every layer of that ladder — explicitly so a coaching system can
distinguish "you counted attackers vs defenders correctly and still missed the tactic
(because you didn't see a commitment)" from "you didn't even count right" — a genuine
skill-level diagnostic, not just a flat list of concept names.

**The lattice, precisely, confirmed against the code that already existed:**
1. Attacked at all? Shared precondition every rung below checks first, not a concept.
2. Defended at all? `find_hanging` (0 raw defenders) and `find_outnumbered` (>0 raw
   defenders, still fewer than attackers) are two halves of one raw-count comparison,
   split only because certainty differs.
3. Is a raw defender actually free to help? `find_overloaded` (from the defender's side:
   am I already the sole defender of something else?) and `find_false_defense` (from the
   attacked piece's side: are *all* my defenders pinned off the recapture line?) are
   siblings, not one step — different anchor point, different strength of constraint
   (overload is soft/costly, pin is hard/illegal).
4. Does that change the verdict the raw count gave? This rung was missing — nothing
   cross-referenced `overloaded`/`false_defense`-style commitment facts back against a
   raw count that, read alone, said "safe." That's the specific miss the user described:
   "you have more attacking than defending... but failed to recognize the other
   commitments of those pieces" when the *raw numbers* looked fine.

**New: `ThreatGraph::find_false_safety(color, pins, overloaded) -> Vec<FalseSafety>`**
(`threat_graph.rs`). Fires exactly when `raw_defender_count >= attacker_count` (so neither
`find_hanging` nor `find_outnumbered` touch it — this rung only exists where the raw count
alone would have said "safe") but discounting defenders that are pinned off-line *or*
overloaded elsewhere drops the effective count below the attacker count. Deliberately a
*partial*-discount generalization of `find_false_defense`'s all-or-nothing check, not a
replacement for it — `false_defense` still reports its own narrower fact (every defender
compromised) standalone; `false_safety` is the new, wider net that also catches the
partial case (2 of 3 defenders fine, but 1 compromised one flips a borderline count) and
folds in overload as a second compromise source pin-only `false_defense` never considered.
`FalseSafety{piece, attacker_count, raw_defender_count, effective_defender_count,
compromised_defenders, value}` — both counts carried, not just the conclusion, exactly so
a report (or a database row) can show the gap between what the bare numbers said and
what's actually true. New concept `false_safety`, ELO 1800 (above both `overloaded`/1400
and `false_defense`/1600 — it requires composing either with a count that looks fine on
its own), confidence tier 0.8 (same as its two constituent facts).

Verified with a deliberately non-overlapping test position (`threat_graph.rs`'s own
"pathfind the graph" discipline extended to test design too): white pawn d4 attacked by
`Rd8` + `Ba7` (2 attackers), defended by `Rd1` (genuinely free) and `Nb3` (defends d4 by
knight-move, but pinned to `Kb1` by `Rb8` along the b-file — d4 isn't on that line) = 2 raw
defenders, 1 effective. Chosen specifically so only 1 of 2 defenders is compromised —
`false_defense`'s all-or-nothing check does *not* also fire on this position, confirmed in
the test itself, so the two concepts are shown to report genuinely different facts, not
duplicates. Negative test: same position plus a third free defender (`Bb2`) restores
effective count to exactly the attacker count — not flagged (boundary is strict `<`, not
`<=`). Flip-invariance test confirmed both the piece and every `compromised_defenders`
entry unflip correctly. Three new permanent tests in `motif_canonical.rs`.

**Database reporting — preserving every rung, not collapsing them into one signal.**
Traced the actual reporting path (it's `chessdb/*.nu`, not anything inside the Rust
crate — no sqlite/rusqlite code lives in `nu_plugin_chessdb` itself): `StateVector`
(`concepts.rs`) packs a compact per-position bitfield, stored as `positions.state_id` and
decoded once into named `move_states` boolean columns (`chessdb/db.nu`,
`chessdb/sync.nu`) — never re-shifted downstream, per the existing convention documented
in both files. `coach_derive_cmd.rs`'s `compute_baselines`/`detect_anomalies` read those
booleans to build per-player, per-phase Welford baselines (`player_baselines`) and flag
eval-swing anomalies (`move_anomalies.concept_name`), which `chessdb/profile.nu` then
aggregates into player-facing KPIs (`tactical-concepts`, `tactical-phase-breakdown`,
`tactical-win-impact`, `profile-concepts`, `concept-examples`).

That pipeline only reports what's in `StateVector` — and `state_id` was a `u16` with
exactly **one** free bit (bits 0-14 already used: 2 for phase, 3 for material sign, 10
single-purpose flags; bit 15 was the last one). Four new rungs need four new bits.
Widened `StateVector.state_id` (and `SensorReport.state_id`, and every `u16` call site
that touched it: `process_corpus.rs`'s `PendingPos.state_id`, `coach_derive_cmd.rs`'s
`MoveRecord.state_id`) from `u16` to `u32` — bits 15-18 now `BIT_OUTNUMBERED`,
`BIT_OVERLOADED`, `BIT_FALSE_DEFENSE`, `BIT_FALSE_SAFETY`. `encode_state`/`decode_state_id`
needed no logic change beyond the wider integer type, by design (this is exactly what
their "add one row to `BOOL_BITS`, one field to the struct" contract was built for).

Wired the four new booleans through every layer that already handled the existing ten,
same shape, no new abstraction:
- `coach_derive_cmd.rs`: added to both `check_concepts` arrays (`compute_baselines`,
  `detect_anomalies`) and `state_vector_to_value`'s field list. The fast/slow-path
  agreement test (`fast_path_and_slow_path_agree_on_state_id`) needed no changes — it
  compares whole `StateVector`s field-by-field already, so it started exercising the new
  bits automatically.
- `chessdb/db.nu`: four new `move_states` columns (`has_outnumbered`, `has_overloaded`,
  `has_false_defense`, `has_false_safety`), same `ALTER TABLE ADD COLUMN` + backfill
  pattern as the existing `has_outpost`/`has_open_file`/`has_passed_pawn` migration —
  with one honest caveat written into the comment: rows whose `positions.state_id` was
  computed before this widening backfill to `false` regardless of the position's real
  properties, because those bits simply didn't exist yet to be set. Historic data needs
  positions re-evaluated (not just `move_states` re-derived) to get real values here —
  same limitation any bit added to an already-populated cache column has.
- `chessdb/sync.nu`: the same four bit-shifts added to `import-records`'s `move_states`
  INSERT.
- `chessdb/profile.nu`: widened the three tactical `concept_name IN (...)` allow-lists
  (`tactical-concepts`, `tactical-phase-breakdown`, `tactical-worst-games`) to include the
  four new names, and extended `tactical-win-impact`'s hand-rolled pivot (both
  `player_flags`/`opp_flags` CTEs, eight new `UNION ALL` blocks) the same mechanical way
  its existing fork/pin/hanging_piece rows were built. `profile-concepts` and
  `concept-examples` needed no changes — they already select all `concept_name != 'hugm_delta'`
  rather than hardcoding a list, so they pick up the new rungs automatically.

The result: a player's specific failure depth is now a queryable fact, not just an
aggregate "missed a tactic" signal — `tactical-concepts`/`tactical-phase-breakdown` can
show whether a player's anomalies cluster at `outnumbered` (can't count) vs `false_safety`
(counts fine, misses commitments) vs `overloaded`/`false_defense` (sees the commitment
type but not the count interaction) — the actual skill-ladder diagnostic the user asked
for, not a proxy for it.

Verified: `cargo check --all-targets` clean; `cargo test` green (43 lib, 27 motif tests —
three new); `cargo clippy --all-targets` zero warnings; STS smoke test passes; `nu -c "use
chessdb *"` confirms `db.nu`/`sync.nu`/`profile.nu` still parse after the schema and query
changes. `threat_graph.rs`'s module doc gained a "failure lattice" section laying out the
four rungs explicitly, matching this entry.

## 2026-07-31: correction — "know," not "save"; and `GatedIssue.stage`

User pushback on the entry above, verbatim: "I think you took me too literally. I want to
be able to know each condition not necessarily save each condition. Its all part of the
ladder, at what point do you check more deeply." Then, once asked whether the LLM coach
could discover these patterns itself instead of being told: "we have to tell the LLM at
least through the tactical layer, and possibly much through the strategic layer." Then,
concretely: "we need to communicate where on the progression of calculation the player
failed."

**What this actually corrected**: not the Rust-side detection (`find_false_safety` etc.
stays exactly as built), but the DB persistence layer added right after it — widening
`state_id`, four new `move_states` columns, `coach_derive_cmd.rs` baseline/anomaly wiring,
`profile.nu`'s win-impact extension. That whole apparatus treats each rung as its own
independently-tracked, independently-baselined cross-game statistic (Welford means,
z-scores, win-rate pivots) — a different question ("does this player historically blunder
more when overloaded") than what's actually needed ("does the coach, looking at *this*
position, know the full ladder to explain *this* mistake"). The second is already answered
by `chessdb hugm-eval`'s `gated_issues` (`hugm_eval_cmd.rs:153`, copied straight from
`sensor_report.gated_issues`) — no schema needed. The DB layer stays in the tree
(reverting it wasn't asked for either — "not necessarily save" leaves room for it) but
isn't the mechanism doing the actual coaching-time communication.

**The three-layer framing, user's words, mapped onto real code**: control (the geometric
map, `ThreatGraph.control`) → tactical value of control net of the opponent's (where
exchanges/blunders happen — the whole failure lattice, `hanging`→`outnumbered`→
`overloaded`/`false_defense`→`false_safety`) → strategic value of control net of the
opponent's (center/flank/king-zone importance — substrate exists as `zone_control`, no
concept built on it yet). Resolved where the LLM-vs-precompute line falls, and it isn't
"tactical vs strategic": it's whether producing the fact requires **exhaustive, exact
enumeration across the board** (must be computed and told — this is every rung of the
ladder, and will be true of strategic zone-control numbers too) vs. **classification of a
few already-clean summary numbers** (fine to hand the LLM raw and let it reason — e.g.
phase-from-material-count, which turned out to not even be LLM-facing today: grepped
`sensor.rs`/`concept_types.rs`/`hugm_eval_cmd.rs`, `phase` appears in none of them — it's
internal engine plumbing for `compute_groups`'s tapered-eval weight blending and
`encode_state`'s phase bucket, never a coaching fact, so nothing was actually undermined
by concluding the LLM could do that judgment itself).

**Concrete fix for "communicate where the progression failed"**: checked the five ladder
concepts' existing phrases. `false_defense`/`false_safety` already narrate the progression
explicitly ("looks defended but..."/"looks defended by the count... but..."). `outnumbered`
didn't — rewrote its phrase to `"{color}'s {role} has a defender, so it isn't simply
hanging, but is still outnumbered (...)"`, stating the shallower check (not hanging) that
passed before the one that failed. `hanging_piece` needs no such framing (rung 1, nothing
shallower exists). `overloaded` is anchored on the *defender's* vantage point, not a
specific victim's verdict, so it can't take the same "looks X but Y" shape without
picking one arbitrary target from `critical_for` — left as its own standalone fact; the
connection to a specific victim's false verdict is already made explicitly wherever
`false_safety`/`false_defense` name that defender in `compromised_defenders`/
`pinned_defenders`.

Also added a machine-legible anchor, not just prose: `GatedIssue.stage: u8`
(`concept_types.rs`) — 1=`hanging_piece`, 2=`outnumbered`, 3=`overloaded`/`false_defense`,
4=`false_safety`, 0 for everything outside this specific ladder. Computed by a new
`ladder_stage(name) -> u8` helper (`concepts.rs`), called from both
`rank_issues_for_position` and `rank_issues_for_player` right where `confidence` is
already matched on `c.name` — same pattern, same place. Deliberately not a new field on
`Concept` (would have required touching every one of the ~15 `Concept{...}` construction
sites for a field only 5 of them need) — `GatedIssue` is built via `filter_map` in exactly
two places already, so the derivation lives there instead.

Verification note: tried testing `stage` through `rank_issues_for_position` on the existing
`overloaded`/`false_defense`/`false_safety` test positions first — all three failed, because
every hand-built tactical test FEN has *some* material imbalance (they're built around one
specific tactic, not material balance), and `rank_issues_for_position`'s existing
`has_critical` gate ("a critical low-ELO issue suppresses higher-level coaching") retains
only `elo_min <= 1200` whenever a `severity >= 80, elo_min <= 1000, score > 10` issue
coexists — which `material_imbalance` (`elo_min: 600`) always does in these positions,
regardless of player_elo. That's a real, deliberate, pre-existing behavior (worth noting: it's
itself a form of the same "communicate the right depth" principle — don't hand a beginner
false_safety-level nuance when they're one move from losing a whole piece for free) — just
not the thing these particular tests needed to exercise. Moved the `stage` check to a
direct unit test of `ladder_stage()` in a new `concepts.rs::tests` module instead
(`ladder_stage_orders_the_five_piece_safety_rungs`), and kept the `rank_issues_for_position`-
based stage checks only on `hanging_piece`/`outnumbered` (`elo_min` 600/800, both survive
the retain) in `motif_canonical.rs`. Also added a phrase-content assertion on the rewritten
`outnumbered` phrase.

Verified: `cargo check --all-targets` clean; `cargo test` green (44 lib — one new; 27 motif
tests, phrase assertion added, no new test count change); `cargo clippy --all-targets` zero
warnings; STS smoke test passes.

## 2026-07-31: `collapse_criticality` — clear the cluster, place each candidate back, no move order

Motivating problem, user's example: "a brilliant move can look like a hanging piece." A
queen sac with zero raw defenders fires `find_hanging` at rung 1 — the shallowest, most
confident-looking signal in the whole lattice — but "zero defenders" says nothing about
whether the capturing piece then walks into disaster. Closing that gap by actually
simulating the position after the capture is exactly the search this system was built to
avoid ("I don't want to compete with the real engine").

**Two design attempts, both caught and corrected before landing:**

1. First attempt: pick the *least-valuable attacker* and substitute it onto the victim's
   square (discard victim, discard capturer's origin, place capturer on the square). User
   caught this immediately: "you are still thinking too much like an engine... this looks
   almost entirely like a move by move calculation." Choosing *which* piece recaptures is a
   move decision, the exact thing every other primitive in this file avoids.
2. Second attempt: remove cluster members one at a time from the otherwise-unchanged board
   (see the superseded design notes originally here). User corrected this too: "it shouldn't
   remove just one piece, it should remove all related pieces controlling that square, and
   then put each in that square." Removing one piece while the rest of the cluster still
   sits on the board doesn't cleanly answer "if this piece is the one left standing once
   everyone else contesting the square had traded off, is that safe for it" — the other
   still-present cluster members confound the reading.

**The mechanic that stuck**: clear the *entire* local cluster first — every attacker, every
defender, the occupant, all removed in one pass, giving a clean board. Then, one candidate
at a time, place *just that piece* back onto the contested square and rebuild the graph
fresh. No capture order, no recapture choice, nobody moves anywhere except the one
placement being tested — a structural "if this is the piece left standing here, is that
actually safe for it," independent of which order any real exchange would happen in. This
identifies false defenders directly and generally: a piece whose own king ends up in check
(or checkmate) once it's the one occupying the square cannot actually go there safely,
whatever the raw attacker/defender count said — the same underlying fact `find_false_defense`
narrowly checks via pin-list membership, now discoverable for *any* piece via one clear-and-
place operation. "Those can live just as counts and which piece that delta came from" (user,
verbatim) — no verdict baked in, just readings for the caller to interpret.

**Implementation** (`threat_graph.rs`):
- `ThreatGraph::build` split into `build(chess: &Chess)` (now one line) and
  `build_from_board(board: Board, turn: Color)` — the actual construction logic, which never
  touched anything legality-dependent (castling rights, move history) to begin with, just
  `board.attacks_from`/`attacks_to`/`king_of`. Lets a graph be built on a *hypothetical*,
  not-necessarily-legally-reachable board.
- `king_ring` relocated from `position.rs` (`pub fn`, unchanged body) — a zone *definition*
  ("the box he's being put in," user's phrase), belongs next to `zone_control`, not bundled
  with the legacy scoring functions that happen to consume it as a mobility mask.
- `PieceCriticality` (`concept_types.rs`, marked EXPERIMENTAL, not wired into
  `TacticalReport`/`extract_concepts`): `piece: PieceRef` (the candidate being tested),
  `square_control_delta`, `white_king_zone_delta`, `black_king_zone_delta`,
  `own_king_in_check`, `own_king_checkmated`, `delivers_check`, `delivers_checkmate`.
- `ThreatGraph::collapse_criticality(&self, sq: Square) -> Vec<PieceCriticality>`: builds the
  cluster from `attackers_to[sq]` plus the occupant, discards the whole cluster into a
  `clean_slate` board once, then for each cluster member clones `clean_slate`, places that
  one piece back on `sq`, rebuilds via `build_from_board`, and reads `control`/`zone_control`
  deltas against the real position plus `is_in_check` for both colors on the hypothetical.
- Checkmate is a best-effort enrichment on top of the always-reliable `is_in_check`, not a
  replacement: `is_checkmate_best_effort(board, turn)` builds a `Setup` and calls
  `Chess::from_setup` — which validates full legality (exactly one king per side, etc.) —
  and treats any rejection as simply "not checkmate" rather than propagating an error. This
  matters concretely: whenever a *king* is itself part of the cluster (common near actual
  king safety questions), every other candidate's hypothetical is missing that king
  entirely, and `from_setup` correctly refuses to call that a position at all.
- Folds in the "check, capture, control" hierarchy (checks, then captures, then
  positional control — the priority order for scanning a position, user's framing) directly
  into the three kinds of readings returned, rather than a separate classification pass:
  `own_king_in_check`/`checkmated` and `delivers_check`/`checkmate` are the check tier;
  `square_control_delta` is the capture tier; the king-zone deltas are the control tier.

**Verified** with a hand-derived, then code-confirmed position: `"7k/8/8/5P2/7n/8/8/4K2R w
- - 0 1"` — White `Rh1`+`Pf5`+`Ke1`, Black `Nh4`+`Kh8`. The knight on h4 is the pawn f5's
sole attacker *and* blocks White's rook from the entire h-file. Cluster is `{pawn f5, knight
h4}`; clean slate removes both. Hand-derived every `control`/`zone_control` value
square-by-square for both candidates before writing the test, then let the actual run
confirm — passed on the first run both times this session (this design and its
predecessor), no arithmetic corrections needed:
- Pawn candidate (knight simply absent from this hypothetical): `square_control_delta: 1`
  (nothing attacks f5 any more), `black_king_zone_delta: -2`, `delivers_check: true` — the
  h-file is open regardless of who's on f5, since the knight's absence alone reopens it.
- Knight candidate (as if it had captured and were the one standing on f5):
  `square_control_delta: -1`, `black_king_zone_delta: -1`, `own_king_in_check: true` — the
  knight vacating h4 to occupy f5 leaves *its own* king in check. This is the precise shape
  of "brilliant move looks like a hanging piece": the piece that would do the capturing is
  the one that can't actually afford to.
- Neither candidate is checkmate in this position (Black's king still has g7/g8) — both
  `own_king_checkmated`/`delivers_checkmate` correctly read false, confirming the
  best-effort checkmate check agrees with "just check" rather than over- or under-calling it.
- A second position (`"7k/6n1/8/8/8/8/8/6QK w - - 0 1"`, cluster `{queen g1, king h8,
  knight g7}`) confirms the king-in-cluster case degrades gracefully: every candidate but
  the king itself leaves Black's king missing from the hypothetical, and the checkmate
  check reads false without panicking, for all three candidates.
- A negative control: an empty, unattacked square returns an empty cluster.

Verified: `cargo check --all-targets` clean; `cargo test` green (47 lib — three new; 27
motif tests unchanged); `cargo clippy --all-targets` zero warnings; STS smoke test passes.

**Explicitly not done yet** (stays an experiment per the user's framing, "right now this is
an experiment"): not wired into `TacticalReport`, `extract_concepts`, or the failure lattice.
Open questions before it could be: what swing magnitude is "significant" enough to flag as
compensating for an apparent hang; whether this should run automatically for every
`find_hanging`/`find_outnumbered`/`find_false_safety` hit (expensive — one `ThreatGraph`
rebuild per cluster member) or only on demand; and whether the several readings returned
should combine into one severity number or stay separate for the caller to weigh.

### Same day, follow-up: consolidate through shakmaty, and surface more than just the king

Two more corrections, same design, both from continuing to question "are we reinventing
something shakmaty already gives us, and are we only looking at the king?"

**Consolidated check/checkmate through one shakmaty round-trip.** Original version ran two
separate mechanisms per candidate: the graph's own `is_in_check` (cheap, geometry-only) for
"check," and a *separate* `Chess::from_setup` construction only for "checkmate" — user asked
directly whether this was reinventing something shakmaty already answers. It wasn't fully —
`is_in_check` already existed specifically *because* it avoids "a second, separate shakmaty
call" (its own doc comment, from earlier this session) — but running a second, independent
shakmaty construction alongside it for checkmate, without also trusting that same
construction's `is_check()`, was an inconsistency worth fixing. `is_checkmate_best_effort`
replaced with `check_and_mate_via_shakmaty(board, turn) -> Option<(bool, bool)>`: one
`Chess::from_setup` per color now answers *both* questions when it succeeds (`chess.is_check()`
and `chess.is_checkmate()`, both cheap reads off the same constructed position); falls back to
`graph.is_in_check(...)` only when `from_setup` rejects the hypothetical outright (still no
fallback for checkmate — that genuinely needs shakmaty's legal move generation, nothing
graph-native replicates it). Net effect: half as many shakmaty constructions per candidate,
same answers (all four existing assertions on the check-based test still passed unchanged).

**`newly_hanging`: reusing `find_hanging` on the hypothetical itself.** User: "It should tell
us if there are more pieces threatened, checked, checkmated, etc." — the king-zone-only view
missed the general case: clearing a cluster can undefend some *completely different* piece
elsewhere on the board, not just open lines toward a king. Added `PieceCriticality.newly_hanging:
Vec<PieceRef>` — for each candidate's hypothetical, calls `graph.find_hanging()` (the
already-built, already-tested primitive, not a bespoke re-check) and reports entries absent
from `self.find_hanging()` on the real position, i.e. genuinely new consequences of this
specific candidate's placement, not pieces that were already hanging regardless.

One real finding from testing this, not just a test-writing slip: `find_hanging` doesn't
special-case the king's role, so a king left in check with no "defender" (in the loose
attacker/defender-count sense) reads as "hanging" too — caught when a hand-built test
expected exactly one new entry (a rook that lost its knight-defender) and got two, the second
being the king itself, now in check because the same collapse reopened a file onto it.
Correct behavior, redundant signal: check is already reported explicitly and more clearly via
`own_king_in_check`/`delivers_check`. Filtered `role != "King"` out of `newly_hanging` so it
stays focused on genuinely new information, not a second, murkier phrasing of a fact already
covered elsewhere in the struct.

Verified with a purpose-built position (`Bc6` attacking, `Nh4` defending a black rook on
`g2` — 1-for-1, not hanging in the real position; `Nh4` is *also* part of `f5`'s collapse
cluster from the earlier test) confirming `g2`'s rook shows up in `newly_hanging` for both
candidates once `h4` is vacated either way, and confirming the king-filter removes the
redundant check-as-hanging duplicate. `cargo check --all-targets` clean; `cargo test` green
(48 lib — one more than the previous entry; 27 motif tests unchanged); `cargo clippy
--all-targets` zero warnings; STS smoke test passes.

### Same day, second follow-up: wired into `extract_concepts` — `hanging_piece` checks first

User: "wire it into extract_concepts so hanging_piece checks this before flagging." This is
the point of building `collapse_criticality` at all — the failure lattice's rung 1
(`hanging_piece`) was a raw zero-defender count with no way to distinguish a real blunder
from a brilliant sacrifice, and now it can.

**The recursion problem, caught before it shipped**: `collapse_criticality` already calls
`self.find_hanging()` for its `newly_hanging` baseline. Having `find_hanging` call
`collapse_criticality` (to compute the new field) would recurse forever —
`find_hanging → collapse_criticality → find_hanging → collapse_criticality → ...`. Fixed by
splitting: `find_hanging_raw` (the original scan, unenriched, `safe_to_capture: true`
placeholder) is what `collapse_criticality`'s baseline and `newly_hanging` computation both
call; the public `find_hanging` calls `find_hanging_raw` once, then for each entry calls
`collapse_criticality(sq)` to fill in `safe_to_capture`. No cycle: `find_hanging_raw` depends
on nothing else in this family; `collapse_criticality` depends only on `find_hanging_raw`;
`find_hanging` depends on both, in that order.

**`HangingPiece.safe_to_capture: bool`** (`concept_types.rs`): true iff at least one attacker
of this piece could capture it without its own king ending up in check
(`collapse_criticality(sq)`, filtered to candidates of the attacking color, `.any(!own_king_in_check)`).
`extract_concepts`'s `hanging_piece` loop (`concepts.rs`) now filters `sensor.tactical.hanging`
to `h.safe_to_capture` *before* collecting values/severity/pushing the concept — a piece
nobody can safely take doesn't count toward this concept at all, checked before flagging, not
as an afterthought correction.

Verified against the exact position from the `collapse_criticality` work (`Nh4` both attacks
`Pf5` with zero defenders *and* blocks `Rh1` from Black's own king): `pawn.safe_to_capture`
is `false`, and `hanging_piece` no longer fires for White in this position. Caught one thing
while writing the test: the knight on h4 is *also* independently hanging (`Rh1` attacks it
directly) and genuinely safe for White to take — confirms the suppression is scoped to the
specific piece/side affected, not a blanket "no hanging pieces here" — a real
`hanging_piece` concept for Black still fires correctly. Also added `safe_to_capture: true`
assertions to the pre-existing `hanging_piece_severity_is_anchored_on_the_biggest_at_risk`
test as a positive control (ordinary hanging pieces must still be reported).

Performance: hanging pieces are rare per position, so the added `collapse_criticality` calls
(one `ThreatGraph` rebuild per cluster member, per hanging piece found) didn't show up in
the STS timing (~0.8s before and after, ~1499 real positions) — the cost this was flagged as
an open question about in the first `collapse_criticality` entry turned out to be
negligible in practice.

Verified: `cargo check --all-targets` clean; `cargo test` green (48 lib unchanged; 28 motif
tests — one new); `cargo clippy --all-targets` zero warnings; STS smoke test passes,
timing unaffected.

## 2026-07-31, continued: checker identity, and "what can be described vs. detected"

**Checker identity.** `own_king_in_check`/`delivers_check` were plain booleans — a real gap,
since naming *which* piece delivers a check is exactly the kind of identifiability this whole
system exists for. Added `ThreatGraph::checkers(color) -> Vec<PieceRef>` — same "read it off
the graph, don't make a second shakmaty call" reasoning `is_in_check` already used, generalized
from yes/no to naming names. Redefined `is_in_check` in terms of `checkers` (one computation,
not two that happen to agree — the same discipline `control`'s doc comment already states).
This also simplified `collapse_criticality`: check no longer needs shakmaty at all, only
checkmate does (genuinely needs legal move generation) — `check_and_mate_via_shakmaty`
collapsed into `is_checkmate_via_shakmaty`, a plain bool, no `Option` ceremony. `PieceCriticality`
gained `own_king_checked_by`/`delivers_check_via: Vec<PieceRef>` alongside the existing bools.
Verified against the existing h4/f5 test: both directions correctly identify White's `Rh1` as
the checking piece (via the reopened h-file), whether it's blocked (pawn candidate, `Rh1`
checks Black directly with the knight simply absent) or the knight itself walks into it
(knight candidate, same rook, now via the square it vacated). 48 lib tests, 28 motif tests,
zero clippy, STS unaffected.

**"What can be described vs. what can be detected/quantified"** — user's framing, verbatim,
for the persistence design. Everything `collapse_criticality` produces decomposes into typed
fields without loss: which piece, which square, checkmate or not, by how much a zone swung —
that's structure, not a description of structure. But synthesizing those facts into "why this
matters" — weighing several named facts into one narrative account — doesn't have a canonical
structured form; forcing it into more columns just produces more structure *about* the facts,
never the account of them. This wasn't a new problem: `Concept`/`GatedIssue` already draws
this line (typed fields next to one `phrase: String`) — it just never had to do real
interpretive work, since existing phrases are mechanical renderings of the numbers beside them
("2 attackers vs 1 defender"), not synthesis. The design conclusion: persist the structured,
identifiable facts; don't try to also pre-bake a narrative at write time, since any fixed
account can only serve one framing and the real interpretive work (tailoring an explanation to
a specific player at a specific moment) belongs at read time — the same "let the LLM
interpret, don't make it enumerate" principle from the phase-classification discussion,
applied to the *output* side instead of the input side this time.

**Implementation — three new pieces, wired together:**
- **`tactical_events` table** (`chessdb/db.nu`): one row per *individual* finding (a specific
  hanging piece, a specific outnumbered piece — not the aggregated per-side `Concept`), since
  `square` only means something at that granularity. Flat columns
  (`game_id`, `ply`, `square`, `concept_name`, `side`, `severity`, `stage`) for graphing/SQL
  aggregation; `detail TEXT` for the fully-identifiable JSON payload. Deliberately no
  description/narrative column. `stage` mirrors `concepts.rs`'s `ladder_stage()` — a second,
  small copy of that mapping, since `ladder_stage` is private and built for aggregated
  `Concept`s, not raw per-instance structs; logged as the same *kind* of known, deferred
  duplication as the confidence-tier match arms, not fixed here. Unique index on
  `(game_id, ply, square, concept_name)`, same idempotent-re-derive pattern as
  `move_anomalies`. Verified: fresh `init-db` produces the correct schema and both indexes.
- **`chessdb collapse-criticality` plugin command** (`src/collapse_criticality_cmd.rs`):
  `collapse_criticality` was Rust-only with no Nu-facing exposure at all. Takes a FEN (pipeline)
  and `--square`, returns the `Vec<PieceCriticality>` as JSON — thin glue over two
  already-thoroughly-tested pieces (`collapse_criticality` itself, `json_to_nu_value`), no new
  logic beyond FEN/square parsing. Registered in `lib.rs`'s command list alongside the others.
- **`chess-tactical-events` Nu command** (`chessdb/sync.nu`, exported via `mod.nu`): for one
  game's moves, calls `hugm-eval` (no `--player-elo` — this is the raw lattice, not the
  elo-gated shortlist) per resulting position, and for each of the five ladder concepts builds
  one row per instance; hanging pieces additionally call `collapse-criticality` on their
  square and fold the full per-candidate breakdown into `detail`. `db-merge`'s existing
  `INSERT OR IGNORE` gives idempotent re-runs for free, same pattern as `chess-derive`.

**Verification gap, disclosed rather than papered over**: the installed `nu` CLI in this
environment is 0.114.0; `nu_plugin_chessdb`'s `Cargo.toml` pins `nu-plugin`/`nu-protocol` to
0.111 — a pre-existing mismatch, not something this work introduced (confirmed: only one `nu`
binary present, `plugin add` succeeds but `plugin use` then fails to load with an explicit
version-compatibility error). This means `chessdb hugm-eval`/`chessdb collapse-criticality`
could not be invoked live in this session to verify `chess-tactical-events`'s actual per-move
computation end-to-end. What *was* verified: the Rust command's logic is thin, well-typed glue
over already-tested primitives; `chess-tactical-events`'s no-moves-found path runs correctly
against a real (empty) database; the whole `chessdb` module, including the new function,
parses cleanly; the `tactical_events` schema (table + both indexes) is confirmed correct via
direct `PRAGMA table_info`/`index_list` queries against a freshly initialized database. The
actual live round-trip (real game, real moves, real plugin calls, rows landing in
`tactical_events`) needs to be run once the plugin is rebuilt against a matching `nu` version,
or the installed `nu` is downgraded/matched to 0.111 — flagged here rather than assumed to work.

Verified: `cargo check --all-targets` clean; `cargo test` green (48 lib, 28 motif tests,
unchanged from the prior entry — no Rust logic changed in this persistence pass beyond the
new thin command); `cargo clippy --all-targets` zero warnings; STS smoke test passes.

### Same day: version mismatch fixed, full pipeline verified live

Bumped `nu-plugin`/`nu-protocol` from `"0.111"` to `"0.114"` (`Cargo.toml`) to match the
installed `nu` CLI (0.114.0) — the crate's API surface turned out fully compatible, zero
code changes needed (`cargo check --all-targets` clean immediately after `cargo update -p
nu-plugin -p nu-protocol`). Rebuilt the release binary, `plugin add`/`plugin use` succeeded
this time.

**Live verification, not just parse-checking:**
- `chessdb collapse-criticality --square f5` on the h4/f5 test position returned exactly the
  hand-derived, Rust-test-verified values (both candidates' every field byte-for-byte
  matching), confirming the actual nu-plugin wire protocol round-trip, not just the Rust logic.
- `chessdb hugm-eval`'s `sensor_report.tactical.hanging` on the same FEN correctly showed
  `safe_to_capture: true` for the knight, `false` for the pawn.
- Built a real game through the actual import pipeline (`chessdb process-corpus` on a
  Scholar's Mate PGN, `db-merge`d into a fresh scratch database — not synthetic rows) and ran
  `chess-tactical-events` against it live. Result: 6 real, correct findings — notably, at ply
  6 (after 3...Nf6), `outnumbered` on Black's `f7` pawn (`Bc4`+`Qh5` vs. just the king — the
  actual tactical point that makes the mate work) and `hanging_piece` on White's own `Qh5`
  (`Nf6` attacks it, nothing defends it — genuinely true at that exact position, White just
  gets to move first). Confirmed idempotent: re-running produced the same 6 rows, not 12.
  Confirmed the `detail` JSON for the hanging queen correctly nests the full
  `collapse_criticality` breakdown (both candidates, all fields, `safe_to_capture: true`).

The verification gap from the previous entry is closed — this is no longer "should work,
parses correctly" but confirmed working end to end: real PGN → real import → real plugin
calls over the wire → real rows in `tactical_events`, idempotent, fully identifiable.

Verified: `cargo check`/`test`/`clippy --all-targets` all clean post-bump (48 lib, 28 motif
tests, zero warnings); STS smoke test passes; live plugin round-trip confirmed via
`collapse-criticality`, `hugm-eval`, and `chess-tactical-events` against both a hand-built
position and a real imported game.

## 2026-08-01: real bug caught by testing against recognizable games — canonical fen fed to hugm-eval

User asked to run two more real, recognizable games (Scholar's Mate re-verified, plus the
Fried Liver Attack main line through move 7) through `chess-tactical-events` as further live
verification. This is exactly why testing against *known, checkable* games is valuable — it
caught a real bug the Scholar's Mate test's first pass had already produced but nobody had
manually verified square-by-square yet.

**The bug**: `chess-tactical-events` queried `positions.fen` and fed it straight into
`hugm-eval`. But `positions.fen` is stored in *canonical* (White-always-to-move) frame — for
any ply where the true side to move is Black, it's a rank-mirrored, color-swapped view of the
real position, not the real position. CLAUDE.md's own canonical-identity section says exactly
this and exactly warns against this mistake ("nothing in canonical form tells you who is
actually White or Black in a real game... never infer real color from the shape of a
canonical FEN/zobrist itself"). Confirmed empirically before fixing anything: queried the
stored fen after `3.Nf3` (real position: only `e4 e5 Nf3` played, Black to move next) and
found a black knight already sitting on `f6` — impossible in reality, since `Nf6` isn't played
until ply 6. That's the canonical mirror of White's real `Nf3` (rank 3 → mirrored to rank 6,
color-swapped), not a real board fact.

**The fix**: `positions.fen`/`pgn_to_fens`'s own `fen` field are *both* canonical (confirmed by
reading `core.rs`'s `GameVisitor::san` — it explicitly canonicalizes before storing, "Store
this position's identity in canonical... frame"). Nothing in the existing schema stores real
per-ply FENs. `moves.uci`, however, *is* stored in real terms. Added a new plugin command,
`chessdb apply-uci` (`src/apply_uci_cmd.rs`, thin wrapper over the already-existing but
never-wired-up `core::apply_uci`) — plays one UCI move on a FEN and returns the result, same
frame in as out, no canonicalization either direction. `chess-tactical-events` now replays
`moves.uci` from the real starting position via `reduce` (genuinely sequential state, not a
previous-row read — the right tool per CLAUDE.md's own `enumerate`-vs-`reduce` guidance) to
reconstruct real per-ply FENs, and uses those instead of `positions.fen`.

**Re-verified both games after the fix, by hand, not just by re-running**: Scholar's Mate now
shows `Qh5` immediately hanging Black's `e5` pawn (real, well-known point of `2.Qh5`); the
`f7` double-attack (`Bc4`+`Qh5` vs. king only) appearing exactly when `3.Bc4` is played; and
`Qh5` itself becoming hanging once `3...Nf6` is played (the actual reason Scholar's Mate
needs immediate follow-through — hesitate and Black just takes the queen). Fried Liver showed
a genuinely subtle correct result that needed re-deriving by hand to confirm: after `4...d5`,
White's `e4` pawn is attacked by both `Nf6` and the `d5` pawn but shows `defender_count: 1`,
not 0 — re-checking the position by hand found the missed defender: `Ng5` (the same knight
that played `Nf3`-`Ng5`) defends `e4` via a *second* knight-move geometry from g5, simultaneous
with its attack on `f7`. The tool was right; my first manual check was incomplete — a good
concrete demonstration of exactly the kind of exhaustive-enumeration mistake this whole system
exists to avoid making silently.

Also noted, not fixed (pre-existing, unrelated to this bug): `find_hanging` doesn't exclude
kings, so a checked king with no piece able to interpose/capture reads as "hanging" too (seen
at ply 13 of the Fried Liver line, `f7 hanging severity 20000` — literally the king's piece
value, right after `7.Qf3+`). Already known and already handled specifically for
`newly_hanging` (kings filtered there, this file's earlier `collapse_criticality` entry) but
`sensor.tactical.hanging` itself was never filtered — out of scope for this fix, flagged for
whenever that gets revisited.

Verified: `cargo check`/`test`/`clippy --all-targets` clean (48 lib, 28 motif, zero warnings)
post-fix; STS smoke test passes; `chessdb apply-uci` verified directly
(`e2e4` from the start position returns the correct resulting FEN); both games re-run live
through the corrected `chess-tactical-events` and spot-checked by hand against real chess
theory, not just "it ran without erroring."

## 2026-08-01, follow-up: excluded kings from find_hanging/find_outnumbered

Fixed the pre-existing quirk flagged (but explicitly deferred) in the previous entry: a
checked king with no piece able to interpose or capture the attacker read as a "hanging
piece worth 20000 centipawns" — technically consistent with the raw attacker/defender
count `find_hanging_raw` computes, but wrong: check is a different, better-stated fact
(`is_in_check`/`checkers`), and kings can't actually be captured (the game ends at
checkmate first). Added `if piece.role == Role::King { continue; }` to both
`find_hanging_raw` and `find_outnumbered` (the same root cause applies to outnumbered too —
a double-checked king with a friendly piece geometrically covering its square would
otherwise read as "outnumbered", equally wrong for the same reason). This also made the
king-role filter in `collapse_criticality`'s `newly_hanging` computation redundant
(`find_hanging_raw` itself never produces king entries now) — removed it rather than leave
dead defensive code checking for something that can no longer happen.

Verified with a real position, not hand-constructed: reconstructed the exact FEN after the
Fried Liver line's `7.Qf3+` via the live `chessdb apply-uci` pipeline (chained through all
13 real moves), confirmed independently that Black's king on f7 really is in check from
`Qf3` with `checkers()` correctly naming it, and confirmed neither `find_hanging` nor
`find_outnumbered` report anything on `f7`. Re-ran the live Fried Liver
`chess-tactical-events` test end to end after rebuilding — the bogus `ply 13, f7,
hanging_piece, severity 20000` row is gone; every other real finding (the `e5`/`e4`/`c4`
hangs, the `f7` double-attack, the `e4`/`d5` outnumbered findings) is unchanged.

Verified: `cargo check`/`test`/`clippy --all-targets` clean (49 lib — one new; 28 motif
tests unchanged); STS smoke test passes; live re-verification against the real imported
Fried Liver game confirms the fix and no regression to the other findings.

## 2026-08-01, follow-up: regression set of known games — `tests/known_games.rs`

User: "lets build a regression set of games, the ones you already used plus all the
beginner traps that we can find, all well trodden and named." Sourced five more real,
well-documented traps via `WebSearch`/`WebFetch` (cross-checking at least two independent
sources per game before trusting a line, same discipline as the ICBM/Alien Gambit work —
one WebFetch extraction genuinely hallucinated an illegal move, `4.Nxe6` with nothing on
e6, caught by disagreeing with a second source rather than assumed correct): Légal's Trap,
the Blackburne Shilling Gambit, the Queen's Gambit Declined Elephant Trap, the Ruy Lopez
Noah's Ark Trap (real historical game, Steiner–Capablanca, Budapest 1929), and the Drunken
Bishops Gambit's mating line — this last one pulled as *raw PGN* directly from a real
lichess study export (`lichess.org/study/.../....pgn`, not a video summary), the most
reliable sourcing of the batch.

**A second, deeper king-inclusion bug, found only by testing against real games.** Running
Noah's Ark live surfaced `overloaded`/`false_safety` findings with king-sized severities
(20000/20500) — the same root issue as the `hanging_piece`/`outnumbered` king fix from
earlier the same day, just in the two detectors that fix didn't touch. `find_overloaded`
was treating the king as a normal capturable "target" a piece could be the sole defender
of; `find_false_defense`/`find_false_safety` were doing raw attacker/defender counting on
king-occupied squares directly. Fixed all three the same way: skip `Role::King` — as a
target in `find_overloaded`'s inner loop, as the occupant in `find_false_defense` and
`find_false_safety`. Verified with a real position (Noah's Ark after `10.Qc6+`,
reconstructed via `apply-uci`, not hand-built) confirming the king really is in check but
no longer appears in any of the three detectors — new test
`checked_king_is_excluded_from_overloaded_false_defense_and_false_safety_too`. This is
exactly the value of testing against a *diverse* set of real games rather than only
hand-built minimal positions: the original king-exclusion fix only exercised `hanging`/
`outnumbered` directly, and it took a real game reaching a checked-king-with-nominal-
defenders configuration to expose that `overloaded`/`false_safety` had the identical gap.

**`tests/known_games.rs`**: one test per game (9 total — the 4 from the earlier ad hoc
live sessions plus the 5 new ones), replaying real SAN move sequences via a `replay_san`
helper that uses shakmaty directly (never touching `positions.fen`/`pgn_to_fens`, both
canonical — the exact bug fixed earlier the same day) and asserting the specific,
already-hand-verified findings at each game's key tactical moment, encoded permanently so
they run under `cargo test` instead of needing a live plugin round-trip and a scratch
database each time. Every assertion had already been derived by hand against the real
position *and* confirmed live through the actual `chess-tactical-events` pipeline before
being written down here — this file is the permanent record of that verification, not a
new round of guessing. Notable finds preserved as regression coverage:
- Fried Liver: `e4` reads `outnumbered` (2 attackers, 1 defender), not `hanging` — the
  defender is `Ng5`, defending via a second knight-move line while also attacking `f7`.
  First hand-derivation expected 0 defenders and was wrong; re-checking found this.
- ICBM: the queen only becomes hanging the instant `Bg6+` vacates `d3` — the check is a
  free tempo attached to the move that clears an already-aimed file, not what wins the
  queen itself.
- Légal's Trap: `5.Nxe5` vacating `f3` opens `Qd1`'s diagonal onto `Bg4` — a real
  discovered-attack-shaped consequence of the capture, not just "the knight took a pawn."
- Drunken Bishops Gambit: the deepest and most interesting one — `11.Be2` (forced,
  interposing against `10...Re8+`) leaves the bishop pinned along the e-file; two moves
  later it nominally "defends" `f3` (1 attacker, 1 defender, `false_safety`'s own
  precondition for firing) but can't actually recapture there without breaking the pin —
  exactly why `13...Nxf3` is checkmate, not just a won piece. Both `false_defense` and
  `false_safety` correctly fire on the same square for the same reason.
- Noah's Ark: deliberately included a case the current tactical ladder *isn't* designed to
  catch — the bishop's eventual permanent entrapment on `b3` is a positional fact (no legal
  escape square, ever), not an immediate material-safety one. Only asserted the earlier,
  genuinely tactical moment (`Bb5` hanging to `...a6`) instead — an honest example of where
  this ladder's boundary sits, matching the user's own explicitly separate "later, more
  positional sensors" direction rather than a gap to paper over.

Verified: `cargo check --all-targets` clean; `cargo test` green (50 lib — two new; 9 new
`known_games` integration tests, all passing on first run against hand-derived
expectations; 28 motif tests unchanged); `cargo clippy --all-targets` zero warnings; STS
smoke test passes.

## 2026-08-01, follow-up: Alekhine's Gun — an honest boundary case, not a gap papered over

User: "oh, Alekhine's Gun ... that would be interesting too." Sourced the actual historical
game (Alekhine vs. Nimzowitsch, San Remo 1930) as raw PGN directly from a lichess study
export (`lichess.org/study/cpuqVg6h/u0Ppk2Ul.pgn`), cross-checked against a chess.com blog
that independently quoted the same first 26 moves verbatim — the most confidence this suite
has had in a source, on its longest game by far (34 moves / 67 plies, vs. the previous
longest at 15 plies).

**The honest finding**: the "gun" formation itself — `Rc2`+`Rc3`+`Qc1` stacked on the
c-file, completed at move 26 — triggers nothing in the current ladder. No
`outnumbered`/`overloaded` shows up on Black's c-file stack despite the obvious positional
pressure a human immediately recognizes. This isn't a bug to chase — Alekhine's gun is a
slow positional squeeze accumulated over many moves, exactly the category of thing that
needs the *future* positional sensors (file/zone control tracked over time) this session
has repeatedly scoped as separate, later work, not the current material-only ladder. Ran
the game live through `chess-tactical-events` specifically to confirm this absence rather
than assume it, before deciding what (if anything) to assert.

What the ladder *does* find correctly, one move past the formation: right after `27...b5`,
White's bishop on `a4` and Black's pawn on `b5` mutually hang each other — neither
defended — which is exactly why `28.Bxb5` follows in the real game. Added as
`alekhines_gun_mutual_hang_after_the_queenside_pawn_break` in `tests/known_games.rs`, with
the "gun itself isn't caught, and here's precisely why that's expected" reasoning written
directly into the test's own comment rather than left implicit — the same discipline as
Noah's Ark's entrapment-vs-tactic distinction, now with a second, more prominent example.

Verified: `cargo check`/`test`/`clippy --all-targets` clean (50 lib unchanged; 10
`known_games` tests — one new, passing on first run against hand-derived expectations; 28
motif tests unchanged); STS smoke test passes.

---

## 2026-07-31: NNUE_AUDIT.md retired — folded into PLAN.md and this archive

`NNUE_AUDIT.md` existed as a third documentation file alongside `PLAN.md`/`FINDINGS.md`,
which worked against the point of compacting the documentation into exactly two roles
(current-state vs. archive). Retired it; its content is redistributed as follows.

**Current state** (what survives, now in `PLAN.md`'s "External oracle: Stockfish gives a
score, not features" section): `chessdb nnue-eval` was renamed to `chessdb stockfish-eval`
(`src/nnue_eval_cmd.rs` → `src/stockfish_eval_cmd.rs`, struct `NnueEval` → `StockfishEval`,
output field `nnue_score` → `stockfish_score`). The rename was prompted by the user asking
"I thought we removed NNUE" — a fair question, because two different things had been
sharing the "NNUE" name: a now-deleted custom-trained net, and a UCI wrapper that just
reads Stockfish's own final score as an external oracle. The command never did anything
NNUE-specific itself (Stockfish happens to use its own NNUE internally to produce that
score, which is Stockfish's implementation detail, not this project's), so keeping "nnue"
in the name kept implying the wrong thing. The user's own framing of where this could go
later — regressing Stockfish's oracle score against *this project's own* detected/
quantified tactical and strategic features, rather than training something
architecturally NNUE-shaped from generic piece-square encodings — is now recorded in
`PLAN.md` verbatim as the deliberately-not-yet-started future direction.

**Archived history** (the rest of `NNUE_AUDIT.md`'s content, preserved here verbatim since
it no longer has a live home):

> ## 2026-05-13 Decision (still current as of 2026-07-28): Re-scoped
>
> Full NNUE training is deferred. The project imports Stockfish's built-in NNUE
> via UCI rather than training a custom net. The bullet-based training pipeline
> (`dataset_builder_cmd.rs`, bulletformat shards) was removed 2026-07-30 in a
> YAGNI pass — it had been paused with no active work since before this audit.
> `src/position_encoder.rs` (the feature-vector encoder that fed it) was
> removed the same day: it had no callers left once `dataset_builder_cmd.rs`
> was gone, so "kept for a future training pipeline" was really just
> unnoticed dead code — this doc had claimed it was a deliberate placeholder
> without that having actually been checked.
>
> **Current focus**: HUGM calibration — regressing HUGM component scores against
> Stockfish centipawn scores to tune HUGM weights. `hugm_harness` (`src/bin/hugm_harness.rs`,
> 348 lines) reads `{fen, engine_score}` JSONL and has the regression scaffolding; it
> is not yet wired into an automated tuning loop.
>
> ## Current inference command: `chessdb nnue-eval` (since renamed to `chessdb stockfish-eval`)
>
> ### Usage
> ```
> "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" | chessdb nnue-eval
> ```
> Returns: `{fen, nnue_score}` record with centipawn evaluation.
>
> Supports lists of FENs for batch processing.
>
> Implementation: `src/nnue_eval_cmd.rs` spawns Stockfish as a subprocess (UCI),
> resolving the binary from `$STOCKFISH_BIN` (default: `/usr/sbin/stockfish`).
>
> ### Remaining open items
> - Long-term: if direct `.nnue` file loading is needed (faster than UCI), implement a Rust NNUE parser. Not required now.
>
> ### Resolved since last audit
> - The old BUG-6 (Stockfish path inconsistency between `nnue-eval` and `sf_batch_eval`)
>   is gone: `src/bin/sf_batch_eval.rs` is now a 3-line stub
>   (`eprintln!("sf_batch_eval removed: use the external labeling pipeline described in NNUE_AUDIT.md")`)
>   with no hardcoded path left to be inconsistent. Labeling-corpus generation now goes
>   through `src/bin/lichess_to_jsonl.rs` / `src/bin/pgn_to_jsonl.rs` → `hugm_harness`.
>
> ## Original Audit (archived; background/history)
>
> Purpose
> - Quick research & scoping (Phase 0) for adding NNUE training/inference support.
> - Map what already exists in the repository that we can reuse, identify gaps, and propose next concrete tasks.
>
> Background (short)
> - NNUE (Efficiently Updatable Neural Network) is a lightweight, high-performance neural evaluator widely used in chess engines.
> - Key idea: a sparse, piece-list-friendly input encoding and a small dense network (feature transformer + hidden layers) that can be cheaply updated as pieces move.
>
> Current reusable pieces
> - Position encoder (`src/position_encoder.rs`): 1024-element f32 vector (793 meaningful
>   features: 768 piece-square one-hot + game-state + material balance + king position +
>   tactical summary, zero-padded). Removed 2026-07-30 along with `dataset_builder_cmd.rs`.
> - HUGM eval (`src/eval/position.rs`): ~3400 lines of handcrafted heuristics with tunable weights.
> - NNUE eval (`src/nnue_eval_cmd.rs`): UCI-based Stockfish wrapper.
>
> Policy: Stockfish evaluation handling (unchanged)
> - Do NOT persist Stockfish numeric evaluations as canonical fields in the positions table.
> - Stockfish is an external oracle for review and labeling.
> - HUGM remains the primary human-interpretable heuristic layer.

Verified: renamed command compiles clean (`cargo check --all-targets`), repo-wide grep for
`nnue`/`NNUE`/`NnueEval` confirms no remaining references outside Stockfish's own UCI
protocol strings in `stockfish.rs` (`"Use NNUE value true"`, its "Final evaluation" reply
format) — those name Stockfish's own internal option/output, not this project's naming, and
correctly stay as-is.

---

## 2026-07-31: Terms-bag → typed SensorReport migration finished

The last four holdouts in `render_explanations` — `tropism_us`, `doubled_rooks`,
`development_diff`, `initiative` — were reading `groups.*.terms.get(...)` directly, flagged
by their own doc comment as "a deliberate, narrow exception... not a gap in the migration."
Checked that framing against `SensorReport`'s actual fields: it had no home for any of the
four, so the exception was real, not stale documentation. Closed it anyway, since a
"deliberate exception" is still two sources of truth that only agree by coincidence:

- Added four scalar fields directly to `SensorReport` (`sensor.rs`): `king_tropism_us`,
  `doubled_rooks_us`, `development_score_diff`, `initiative_us`. These are whole-position
  scores, not per-piece concepts, so they don't fit `TacticalReport`/`PositionalReport` —
  flat fields on `SensorReport` itself, matching the existing `mate_in_1_exists`/`in_check`
  pattern, rather than forcing them into an unrelated struct (`KingExposure`,
  `DevelopmentInfo`) whose fields mean something different.
- Populated them once, in `build_sensor_report` (`position.rs`) — the one legitimate place
  that's allowed to read `groups.*.terms`, since it's the file's own conversion boundary.
- Rewrote `render_explanations`'s four remaining `.terms.get(...)` call sites to read the
  new typed fields instead, and deleted the now-obsolete "narrow exception" doc comment.

`render_explanations`/`render_structured_explanations` now read exclusively from
`record.sensor_report` — verified by grep across both functions' exact line ranges, zero
`.terms` matches remain. This was item #2 of a five-item "consistency pass" plan queued
earlier in this session; items #1 (module doc), #3 (`coach_derive_cmd.rs` fast/slow path
sharing `decode_state_id`), #4 (`hugm_eval_cmd.rs`'s shared `build_output_value` helper),
and #5 (`core.rs`'s `get_canonical_hash` consolidation) were already done by the time this
was checked — all five items of that plan are now complete.

Verified: `cargo check --all-targets` clean, `cargo clippy --all-targets` zero warnings,
`cargo test` (28 lib + 10 `known_games` + other integration suites, all passing), STS
smoke test (`--ignored`) passes.

---

## 2026-08-31: Fruit game — nu-plugin/shakmaty/pgn-reader had drifted stale, and forks with defended targets read as `Even`

Played a live game against the Fruit 2.1 UCI engine, driving it entirely through Nu +
the `chessdb` plugin, to shake out real usability gaps. Two separate things came out of it.

**The plugin didn't load at all.** `nu-plugin`/`nu-protocol` were pinned to `0.114` while
the installed `nu` shell was `0.115.1` — the plugin silently failed `plugin add`. While
fixing that, `shakmaty` (`0.26`, four minor versions behind `0.30.1`) and `pgn-reader`
(`0.24`, five behind `0.29.0`) turned out to be stale too — both pre-1.0 crates where every
minor bump can break the API, so brought forward in the same pass:
- `shakmaty` 0.30.1: mechanical — `Uci`→`UciMove`, dropped the `ZobristHash` trait import
  (`zobrist_hash` moved onto `Position` directly), `into_setup`→`to_setup`,
  `Board::from_bitboards`→`try_from_bitboards` (now fallible), and
  `Fen::from_position`/`play`/`play_unchecked`/`San::from_move`/`SanPlus::from_move` all
  flipped their `Move`/position argument between owned and `&`-reference.
- `pgn-reader` 0.29.0: real rewrite — `Visitor` moved from plain callback methods to a
  `ControlFlow`-based API with `Tags`/`Movetext`/`Output` associated types;
  `BufferedReader`→`Reader`, `RawHeader`→`RawTag`. Ported `core.rs`'s `GameVisitor` to the
  new shape, preserving identical behavior (same `self.error` sentinel, same accumulation
  into `self.rows`/`self.headers`).

**Blundered a piece three separate times before the tooling was actually usable** — a
knight and a pawn from careless mental board-tracking, then (once the plugin was working
again) a bishop from `13.d5`, which self-opened the 4th rank onto the bishop on c4. From
move 14 on, every candidate move was verified via `apply-uci` (authoritative FEN, rejects
illegal moves) + `hugm-eval --verbose true` (tactical concept lists) before being played —
no further material was hung outright for the rest of the game, though the position was
already lost by then and Fruit converted with `22...Qxg2#` down the long diagonal (the g2
pawn was absolutely pinned by `Bb7`, so it could never have defended that square).

**The real finding: `ThreatGraph::find_forks` already ran a proper SEE, but threw the
result away twice.** First, the public `Fork` struct (`concept_types.rs`) only exposed
`attacker`/`targets` — `EvaluatedFork`'s already-computed `see_cp`/`consequence` fields
were dropped at the one conversion site (`position.rs`'s `TacticalReport` construction).
Second, and more interesting: `find_forks`'s own target selection (`undefended_target`,
now removed) only ran SEE on a fork target if it had *zero* defenders — so a fork where
every target happened to have exactly one defender (a rook covered by a pawn, a queen
covered by a pawn) silently reported `consequence: Even, see_cp: 0`, even when capturing
was still objectively winning material once the recapture was played out. This is exactly
what happened mid-game: `17...Ne5` forked a rook on d3 (defended once by a c2 pawn) and a
queen on f3 (defended once by a g2 pawn) — a real winning fork (`...Nxf3+ gxf3` wins the
queen for a knight) that the tool reported as nothing worth noting. Fixed by replacing
`undefended_target` with `best_fork_target`, which runs full `see_chain` on *every* target
and keeps whichever gives the attacker the best net result — defended or not. Same fix
applied to `Outnumbered` (`find_outnumbered` now also computes `see_cp`/`consequence`),
which previously had no profitability signal at all beyond raw attacker/defender counts.

**Important caveat surfaced by this fix**: making `Fork`/`Outnumbered`'s `see_cp` live for
the first time means an already-documented, still-unfixed `see_chain` bug (this file's
"`ThreatGraph::see_chain` gives wrong answers for 2+ step exchanges" entry, further up)
is now user-visible instead of silently discarded. Re-verified against that exact ground
truth and against the Ne5 fork itself: `see_chain` prices the initial capture correctly,
but every recapture from the first one onward charges the *recapturing* piece's own value
instead of what it's actually capturing — for the Ne5→Qf3 case, correct math is
`900 (queen) − 320 (knight) = 580`, the tool reports `800` (`900 − 100`, using the
recapturing pawn's own value instead of the knight's). Also independently noticed while
re-tracing this: `att_sq = recap_sq` reassigns the contested square to the *recapturing
piece's origin square* (confirmed against shakmaty 0.30.1's `Board::attacks_to(sq,
attacker, occupied) -> Bitboard`, which returns attacker origin squares, not the target) —
a second, distinct bug affecting any chain past the first recapture, not previously called
out explicitly. Neither bug was introduced by this pass; both predate it and were already
flagged as needing the actual SEE backward swap-off algorithm, not a quick patch — still
true, still deferred, not attempted here. What changed: the `Consequence` verdict (not the
raw `see_cp` number) is now the thing actually load-bearing for a caller's "is this worth
it" decision, and it landed correctly on the Ne5 case (`800` and the true `580` both clear
the `Winning` threshold) — but that's not a guarantee the bucket can never flip near a
threshold boundary. `Fork.see_cp`/`Outnumbered.see_cp`'s doc comments (`concept_types.rs`)
now state this caveat directly rather than leaving it only in this file.

Also fixed while in there: `render_explanations`'s opponent-fork phrase had `opp` (the
threat's *source*) as the sentence's grammatical subject — "White has 1 fork(s) (by
opponent)" when describing a fork *against* White, reading backwards. Now `side` (the
mover, who must actually defend) is the subject: "{side} faces N fork(s) from {opp}".
Both fork phrases and a new `outnumbered` phrase (didn't exist before, even though
`sensor.tactical.outnumbered` was already populated) now include the SEE verdict in plain
language instead of just naming the pattern.

Also added: a `final_score_white_relative` field on `PositionRecord`, purely additive —
`final_score` itself stays `us − them` (`us = chess.turn()`), the convention every scoring
function in this file already uses and that mirrors the DB's canonical (White-always-to-
move) position identity. The new field exists because comparing `final_score` across two
positions with different sides to move requires manually flipping its sign — an easy
mistake, and exactly the kind of thing a report-layer convenience field should absorb
instead of every caller re-deriving it.

Also exposed five `core.rs` functions that were fully implemented but never wired to a
`PluginCommand`: `chessdb fen-info`, `chessdb legal-moves`, `chessdb attack-summary`,
`chessdb checker-summary`, `chessdb is-legal` — cheap, single-purpose commands for
questions (material, legal moves, attacked squares, check status, "is this move legal")
that previously required paying for a full `hugm-eval` call to answer.

New regression test: `tests/known_games.rs`'s
`fruit_game_knight_fork_wins_material_even_though_both_targets_are_defended`, anchored on
the exact `17...Ne5` FEN from the game, asserting `consequence == Winning`, `see_cp > 0`,
and `hangs` pointing at the queen (not just the lower-value rook).

Verified: `cargo check --all-targets` / `cargo clippy --all-targets` clean, full `cargo
test` suite green (47 tests, including the new regression test), STS smoke test passes,
release build's plugin binary registered and round-tripped against the live nu 0.115.1
shell for every new command plus the Ne5 fork FEN and the white-relative score sign flip.

---

## 2026-09-01: Fruit-game postmortem, a `see_chain` bug worse than previously documented, `MoverFavored` narrowed to what's actually verified, and a mover-perspective audit

Ran the finished second Fruit game through Fruit's own search (not `hugm-eval`) at every
ply, converting each UCI `score cp` to a consistent White-relative curve. Two moves
accounted for essentially the entire swing from balanced to lost: `17.dxc5` (+16 → −220)
and `23.c5` (−430 → −1113, the single biggest swing of the game). Asked whether any
*existing, non-search* tool would have caught either — since "run a real search" isn't
what this project is building (see PLAN.md's "Pathfinding an exchange instead of
calculating it").

**`23.c5` — the tool already had a signal, but I misread it, and separately the number
itself turned out to be unreliable too.** The `check_move.nu` output for that move already
listed `OUTNUMBERED: white Rook@e4 ... Losing see_cp=-400` a few lines below an exciting
`material win for White (~900cp)` fork line. My first read of this (recorded earlier in
this session) was "the tool caught it and I didn't prioritize my own risk over the good
news" — a real, worth-fixing triage problem regardless. But re-deriving the *correct*
static-exchange value for that exact 2-attacker/1-defender square by hand (the standard
swap-off algorithm, backward minimax pass included) gave **+180 favoring the mover**, not
`-400` — meaning the current tool's number doesn't just need better triage, it's simply
wrong for this square. Confirmed by trying to actually fix it: hand-re-derived the
standard SEE swap-off algorithm (square drift and per-step pricing, the two bugs already
diagnosed in `see_chain`'s doc comments and the 2026-08-31 entry above, both fixed in the
derivation), got a plausible `+180` — then sanity-checked the same derivation against the
simplest possible case (pawn takes pawn, pawn recaptures, no further attackers, which must
net exactly 0 for the initiator by direct arithmetic) and got **100**, not 0. That's a real
bug in the derivation itself, not a subtlety. Per the standing "never fabricate/guess data"
rule, did not commit an unverified rewrite — `see_chain`'s three known bugs (value pricing,
square drift, missing backward pass) stay exactly as documented and deferred, not
"fixed" by something that failed its own sanity check. **Correction to the earlier read**:
tonight's actual, verified lesson from `23.c5` is narrower than first claimed — a real
triage problem was real, but the specific number that triggered it wasn't trustworthy
either, so "read the tool's existing output more carefully" is not by itself a fix here.

**`17.dxc5` — the motivating case, and where the story gets more interesting than "fixed."**
My own knight on e4 had two attackers (Bb7, Nf6) and two defenders (Bd3, Re1) — an equal
count that neither `find_hanging` (needs zero defenders) nor `find_outnumbered` (needs
attackers > defenders) can ever flag, regardless of whether the exchange is actually good
or bad by value. First attempt: new detector `ThreatGraph::find_mover_favored` computing
`see_cp` for exactly this gap via `ThreatGraph::see` (the same chain-walker `find_forks`/
`find_outnumbered` already use), reporting a square whenever the SEE value clears
`Consequence::Winning`/`Minor` despite the raw count looking safe. This correctly flagged
the real `17.dxc5` position's Nf6 knight (`see_cp: 1120`, matching the independently-found
`Qc3` fork exactly) and passed three checks: a hand-built minimal 2v2 fixture where the
known bugs happen to net near-zero (no false positive), the existing
`equal_attacker_defender_count_is_not_outnumbered` test fixture (also empty as expected),
and a direct Rust-level test bypassing the Nu/JSON layers entirely (which is what caught
the plugin-staleness issue below, separately from the correctness question).

**Then a deeper, more damaging `see_chain` bug turned up, and the detector had to be
narrowed.** Live-testing `find_mover_favored` during game 3 flagged a plain d4 pawn,
defended once by the queen, attacked once by the enemy queen down an open file, as
`Minor +100` — favorable for the attacker. That's obviously backwards (losing a queen for a
pawn is bad for whoever does it), and reproduced in a minimal, flip-independent fixture:
`3qk3/8/8/8/3P4/8/8/3QK3 b - - 0 1`. Root cause, traced by hand through `see_chain` line by
line: the function never actually *places* a capturing piece back on the contested square
— it only ever calls `discard_piece_at`, so once both the original piece and the
recapturing piece are removed from the board clone, the contested square goes fully empty.
On the very next loop iteration, `board.attacks_to` (recomputed fresh, on the now-emptied
board) finds the enemy queen still geometrically "attacks" that empty square via the
now-open file and happily lets it "recapture" a piece that isn't there anymore — a third,
phantom capture that flips the sign of the whole exchange. This is strictly worse than the
already-documented pricing/square-drift bugs: it means even the *simplest* 1-attacker/
1-defender case — previously assumed exact, per `see_chain`'s own doc comment — can be
wrong, not just approximate. A same-day attempt to fix `see_chain` properly (the standard
swap-off algorithm, backward minimax pass included) initially looked right on this position
(`+180`, plausible) but failed its own sanity check on the simplest possible input
(pawn-takes-pawn-takes-pawn, which must net exactly 0) — see the paragraph above. Not
committing an unverified rewrite left `find_mover_favored` needing an actual fix, not just a
caveat.

**The fix that shipped**: narrowed `find_mover_favored` to *exactly* 1 attacker / 1
defender — the one shape where the outcome is a single forced capture and a single forced
recapture with no further real captures possible — and compute `see_cp` **directly**
(`victim value − attacker's own piece value`), bypassing `ThreatGraph::see`/`see_chain`
entirely for this concept. No chain to walk means no square to drift and no phantom
capture to invent. Verified against both the false positive above (now correctly empty)
and a fresh, hand-verified positive case (a pawn attacking a knight defended only by a
rook: `4k3/8/8/5p2/4N3/8/8/4RK2 b - - 0 1`, `see_cp: 220`, exactly `knight(320) −
pawn(100)`). Both are now regression tests in `tests/motif_canonical.rs`
(`mover_favored_pawn_attacks_knight_defended_only_by_a_rook`,
`mover_favored_does_not_fire_when_the_lone_defender_outvalues_the_attacker`) — moved out of
`tests/known_games.rs` since they're constructed positions, not real games, matching that
file's own stated convention. **Honest cost of this fix**: the original `17.dxc5` motivating
case (2 attackers, 2 defenders) is no longer caught by anything — `find_mover_favored`'s
scope is now strictly narrower than what prompted building it. That gap is real and still
open; reporting nothing for it is more honest than reporting a number already shown to have
the wrong sign for cases far simpler than 2v2.

Also worth being precise about: even the still-in-place `find_forks`/`find_outnumbered`
`see_cp` values (unchanged by this pass) are now understood to be less reliable than their
doc comments previously claimed — "exact for the first capture and first recapture" was
also disproven by this exact bug. Their doc comments and PLAN.md's "What this deliberately
does not do" section have been updated accordingly; no code change to those two detectors
in this pass beyond the wording.

**A real bug caught along the way: a stale plugin process, not a logic bug.** First attempt
to verify `find_mover_favored` through the actual `chessdb hugm-eval` command returned
nonsense square labels (`white Knight@f3`, `black Bishop@d6` — neither matched a real piece
on the board, confirmed independently via `chessdb attack-summary`). A direct Rust-level
test calling `analyze_fen` on the identical FEN, bypassing the Nu plugin process entirely,
returned the correct answer immediately. The nu shell was talking to a plugin binary from
before the fix — `plugin add`/`plugin use` against the freshly-rebuilt release binary
resolved it. Lesson for next time: when a *newly-added* field's output looks structurally
wrong (right shape, wrong values) right after a rebuild, check whether the plugin process
is actually current before debugging the Rust logic — cross-check via a command that reads
the real board directly (`attack-summary`) and, if that disagrees with the field under
test, suspect staleness before correctness.

**Mover-perspective audit, requested directly**: renamed `BadExchange` → `MoverFavored`
(the name itself was a judgment — "bad" for whom? — where "favors the mover" states the
fact without requiring the reader to already know the perspective convention). Reworded
every `see_cp`/`consequence` doc comment (`Fork`, `Outnumbered`, `MoverFavored`, plus
`threat_graph.rs`'s module doc and `find_mover_favored`'s own doc comment) to consistently
say "the mover" instead of a mix of "attacking side"/"attacker". Explicitly did **not** add
a `Side`/color field to carry "who the mover is" — this crate's positions are always
analyzed in the canonical White-to-move frame (`CLAUDE.md`), so a literal color field would
be uselessly constant; the mover is already fully determined (the opponent of whoever owns
the attacked piece) and doesn't need its own field. Renamed the `consequence_phrase`
helper's parameter from `attacker` to `mover` to match. Redesigned the session's
`check_move.nu` scratch helper to print a "MY PIECES AT RISK" section — hanging entries,
plus any `outnumbered`/`mover_favored` entry whose consequence actually favors the
opponent — first and separately from everything else, specifically so a real risk can't be
buried under an exciting "you have a winning fork" line the way it was on `23.c5`.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (99 tests, including both new `MoverFavored` regression tests — the positive
case and the false-positive-that-must-not-return case), STS smoke test passes, release
build's plugin binary re-registered and round-tripped against the live nu 0.115.1 shell,
confirming the corrected (empty) result on the false-positive fixture, the correct positive
result on the pawn-vs-knight fixture, and the corrected explanation text in both directions.

---

## 2026-09-01 (continued): `find_mover_favored`'s 1-vs-1 restriction was itself too narrow — generalized, again live-verified against a real blunder

Playing a third game against Fruit (using the just-fixed `MoverFavored`/reordered
`check_move.nu` from the entry above), move 8 (`Nxe5`) was checked clean — `MY PIECES AT
RISK: (none)` — and played. Black replied `8...Bxd1`, winning the queen outright for a
bishop. Fruit's own post-game search confirms this was the single largest swing of the
game by far (+32 → −422 white-relative, ply 15).

**Root cause**: the knight that had just moved from f3 to e5 had been the whole time
absolutely pinning itself between the bishop on g4 and the queen on d1 (`Bg4-Nf3-Qd1`, one
diagonal) — moving it opened a direct line from the bishop straight to the queen. Not a
new or exotic pattern; the exact "moving a piece exposes something behind it" shape this
session already fixed once for a *rook* (`23.c5` in the 2026-08-31 entry). But `find_hanging`
correctly did **not** flag the queen (it has two real defenders: the king on e1, and — missed
by eye at the actual board — a knight on c3 that also reaches d1), and `find_outnumbered`
correctly did **not** fire either (1 attacker vs 2 defenders is the opposite of outnumbered).
And the just-shipped `find_mover_favored`, restricted that same day to *exactly* 1
attacker/1 defender, also didn't fire — this position is 1-vs-2, one square outside that
exact box.

**Why "exactly 1-vs-1" was the wrong restriction in hindsight**: the reasoning that produced
it was sound (avoid `see_chain`'s proven wrong-sign bug by not chain-walking at all), but
the scope was drawn one level too narrow. The actual question `MoverFavored` needs to
answer — "is the *first* exchange on this square bad for the defender" — never depended on
*how many* defenders exist, only on whether the *cheapest attacker* is worth less than the
piece it's attacking, and whether at least one real recapture exists at all (so the
question is meaningful — zero defenders is `find_hanging`'s job). A queen attacked by a
single bishop is just as lost with one defender as with three, because the attacker only
ever risks their cheapest piece to win it; a deeper question (would the attacker's side
regret continuing past the first recapture) is exactly the multi-step territory
`see_chain` still can't be trusted for, and `find_mover_favored` still doesn't attempt it.

**Fix**: generalized `find_mover_favored` (`threat_graph.rs`) to fire whenever
`attacker_count >= 1`, `defender_count >= 1`, and `attacker_count <= defender_count` (i.e.
real defenders exist and don't run out first — the gap left by `find_hanging` and
`find_outnumbered`), computing `see_cp` from the board's **cheapest** attacker rather than
assuming exactly one. Still no call into `ThreatGraph::see`/`see_chain` at all — same
direct-subtraction approach as before, just no longer gated to an artificially exact count.
Verified: the existing 1-vs-1 regression tests (`mover_favored_pawn_attacks_knight_...`,
`mover_favored_does_not_fire_...`) still pass unchanged (1-vs-1 is a special case of the
general rule), and a new real-game regression test,
`fruit_game_three_queen_lost_to_a_bishop_despite_two_defenders`
(`tests/known_games.rs`), anchored on the exact `8.Nxe5` position, asserts the queen is now
flagged (`1 attacker, 2 defenders, Winning, see_cp: 570 = 900 − 330`). Re-ran the original
`check_move.nu` checklist against the exact candidate move that was missed in the live game
(`f3e5`) and confirmed it now surfaces `MOVER_FAVORED ... Queen@d1 1v2 ... Winning` in the
"MY PIECES AT RISK" section before the move would ever be played — the actual failure mode
this session has been chasing, closed for this shape of position.

**Still explicitly open**: `render_explanations` only has a "my own risk" (`mover_favored_us`)
text block for this concept, not an "opportunity against the opponent" one (same asymmetry
`Outnumbered`'s prose already had, predating this session) — the structured
`sensor_report.tactical.mover_favored` field is complete and correct regardless, which is
what `check_move.nu` actually reads, but the natural-language explanation for "you have a
won exchange available" doesn't surface it the way "you're about to lose material" does.
Not fixed in this pass. And the original 2-attacker `dxc5`/`Nf6` case that started this
whole thread is *still* not caught (its cheapest attacker still needs correct multi-step
pricing to resolve, i.e. still blocked on `see_chain`) — this generalization widened the
box without ever revisiting that specific unresolved case.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (100 tests), STS smoke test passes, release build's plugin binary re-registered
and round-tripped against the live nu 0.115.1 shell confirming the real `8.Nxe5` position
now correctly flags the queen.

---

## 2026-09-01 (continued): `find_outnumbered`'s `see()`-backed `consequence` had the same sign-flip bug, caught live before a knight was actually lost, and fixed at the source

Fourth game against Fruit, played with the generalized `find_mover_favored`/reordered
`check_move.nu` from the two entries above. Move 19, candidate `Nd4` was checked before
playing it: `check_move.nu` reported `MY PIECES AT RISK: (none)` — clean — but the raw
`hugm-eval` output underneath it also showed `outnumbered=1` in the summary line, unlisted
in the filtered section. Pulling that entry directly showed `Knight@d4, 2 attackers (pawn
e5, bishop c5) vs 1 defender (pawn c3), consequence: Losing, see_cp: -360`. `check_move.nu`'s
filter explicitly excluded `Losing`/`Even` outnumbered entries from "my pieces at risk" —
reasonable *if* `consequence` were reliable, since "Losing" should mean the exchange is bad
for the attacker (i.e. safe for me). Directly simulating the capture (`e5xd4`) before
trusting the label showed otherwise: the eval crashed from roughly even to −2289 — the
knight is plainly just lost to the cheaper pawn. `find_outnumbered` was still calling
`self.see()` (confirmed by reading `threat_graph.rs:541` at the time) — the exact multi-step
`see_chain` machinery already documented as buggy (2026-09-01, earlier entry: phantom
captures on squares its own walk already emptied) and already known to be unused by
`find_mover_favored` for precisely this reason. `find_outnumbered` was the one detector that
still routed through it, and its sign was wrong on a real, live board.

**Immediate fix** (before the source bug was addressed, to keep playing safely):
`check_move.nu`'s filter was changed to stop trusting `outnumbered[].consequence` at all —
it now surfaces every `outnumbered` entry on the mover's own pieces regardless of
consequence, with the printed line flagged `[see_cp/consequence UNVERIFIED - see_chain
bug]`. The raw `attacker_count > defender_count` fact (from `attackers_to`/`by_color`, not
from `see()`) was never in question — only the priced verdict layered on top of it was.

**Root fix**: `find_outnumbered` (`threat_graph.rs`) was rewritten to price `see_cp` the
same way `find_mover_favored` does — direct subtraction, `victim_value − cheapest_attacker_value`,
no call into `see()`/`see_chain` at all. This is a first-exchange-only approximation (same
scope limitation `find_mover_favored` already carries and documents), not a full multi-step
SEE, but it's provably correct for that first exchange, unlike the buggy chain it replaces.
`find_forks` is now the only remaining consumer of `see()`/`see_chain` — its `see_cp`/
`consequence` should still be treated as unverified for 2+ attacker/defender positions,
same caveat as before, just narrower in scope than it used to be.

New regression test, `fruit_game_four_outnumbered_knight_was_mislabeled_safe_by_the_buggy_see_chain`
(`tests/known_games.rs`), anchored on the exact live FEN (`r4rk1/2p1q1pp/p3b3/2b1pp2/2PNn3/2P5/P1Q1BPPP/1R3RK1
b - - 1 19`), asserts the knight is now flagged `consequence: Winning, see_cp: 220 = 320 − 100`
— the correct sign, matching what the direct capture simulation showed. All prior
`find_outnumbered` tests (`outnumbered_piece_is_detected`,
`outnumbered_piece_survives_the_flip_with_real_terms`) pass unchanged, since neither
asserted on `see_cp`/`consequence` — those two fields were the only thing this fix
touched.

Also, once the move-19 trap was correctly avoided (played `Rbe1` instead of `Nd4`), the
rest of the game continued as a straightforward, steadily losing position rather than a
single missed tactic — White had been structurally worse since the early-middlegame
piece odyssey (`2.Nc3 d4 3.Nb5 a6 4.Na3`) and every subsequent forced exchange
(`21...Nxc3`, `22.Qb3`, `26...Nd3`, `27.Nxd3`) was independently checked and confirmed to
be the best available reply, not an avoidable blunder — confirmed against Fruit's own
`fruit_analyze.sh` search postmortem, whose score curve tracks smoothly downward through
those exchanges rather than jumping sharply at any one of them (the only sharp jumps are at
forced captures already recognized as forced, e.g. ply 43's `−545 → −894` after `22.Qb3`,
which was independently verified to be the best of the four candidates checked). Resigned
around move 30 (`Bxf2`, eval ≈ −3864) as a practical stopping point rather than playing out
an already-decided king hunt.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (101 tests, including the new `find_outnumbered` regression), STS smoke test
passes, release build's plugin binary re-registered and round-tripped against the live nu
0.115.1 shell, confirming the corrected `see_cp`/`consequence` directly through
`chessdb hugm-eval` on the exact live position.

---

## 2026-09-01 (continued): sign-convention audit — `Concept`/`GatedIssue` retyped from `Side` to `Mover`, `unflip_phrase` deleted, `final_score_white_relative` removed

After the `find_outnumbered` fix above, user request: "we need a way to audit and simplify
the sign convention to minimize flipping... I like letting it just be mover vs non mover...
and let the client keep track of if that is white or black." A full audit of every
color/sign-flip mechanism in the eval crate turned up **five separate layers**, not one:

1. `normalize_to_white_to_move`/`flip_colors` (`canonical.rs`) — mirrors the board
   internally so `compute_groups`/`ThreatGraph` always run in one "White to move" frame.
   Architecturally sound (mirrors the DB's canonical-position pattern) — kept as-is.
2. `unflip_piece_ref`/`unflip_square_str` — corrects every `PieceRef` (square + color) back
   to real board terms. Necessary: squares/colors are board facts a caller needs to actually
   play the move, not a sign convention — kept as-is.
3. `GatedIssue.side = GatedIssue.side.other()` — a structural color-swap, same pattern as #2
   but applied to `Concept`/`GatedIssue`'s one non-`PieceRef` color field.
4. **`unflip_phrase`** (`canonical.rs`) — a blanket find/replace of the literal words
   `"White"`/`"Black"` inside already-rendered English sentences (`GatedIssue.phrase`),
   needed because `concepts.rs` baked color words into `format!` strings *while still in the
   internal flipped frame* (`material_imbalance`, `bishop_pair`, `doubled_pawn`, and ~40
   other phrase sites). Verified it round-tripped correctly today (a real Black-to-move,
   Black-up-material test came back "Black is up 298cp" correctly) — but it was a landmine:
   any future phrase that didn't route color through `us_color`/`them_color`, or that
   legitimately needed the word "white"/"black" for something unrelated (e.g. a
   "light-squared bishop"), would have silently corrupted text headed straight into the
   `chess-coach` LLM prompt (`ai/mod.nu`).
5. `final_score_white_relative` — a separate, simple sign-flip
   (`if side_to_move==White {final_score} else {-final_score}`) duplicating what any client
   can compute in one line from `final_score` + `side_to_move`. Used directly by this
   session's `check_move.nu`.

Layers 1-2 are a *different* kind of correction (real board facts) and stayed untouched.
Layers 3-5 all existed to translate an internal, mover-relative computation back into real
White/Black terms for a value-judgment field — exactly the "sign convention" the user wanted
minimized. The pattern already done right: `Fork.see_cp`/`Outnumbered.see_cp`/
`MoverFavored.see_cp` need none of this, because they're computed directly relative to "the
mover," never mention White/Black, and the client derives real color from `piece.color`
itself (no server-side flip, nothing to get wrong).

**Fix**: extended that same pattern to `Concept`/`GatedIssue`.

- New `Mover` enum (`concept_types.rs`): exactly two variants, `Us`/`Them` — never a real
  color, and structurally incapable of needing a flip (`Us` always means "whoever
  `side_to_move` says is to move," by definition, in every frame). Serializes to `"us"`/
  `"them"`; `Display` renders `"the mover"`/`"the opponent"`.
- `Concept.side: Side` → `Concept.mover: Mover`; `GatedIssue.side: Side` → `GatedIssue.mover:
  Mover`. Every one of the ~40 construction sites in `concepts.rs` (forks, pins, skewers,
  material_imbalance, bishop_pair, doubled_pawn, king_exposed, development, center_control,
  etc.) updated — mechanical in most cases, since `us_color`/`them_color` (the existing local
  variables) already meant exactly `Mover::Us`/`Mover::Them` semantically, just typed `Side`
  because reusing `Side` was how this shipped originally.
- Every `format!` phrase rewritten to use `Mover`'s `Display` instead of a literal color
  word or `Side`'s `Display` — e.g. `"White is up {n} centipawns in material"` →
  `"{Mover::Us} is up {n} centipawns in material"` → renders `"the mover is up 298 centipawns
  in material"`.
- `unflip_sensor_report`'s `GatedIssue` block (`.side.other()` + `unflip_phrase`) deleted
  outright — nothing color-shaped is left in that struct to correct. `unflip_phrase` itself
  deleted from `canonical.rs` (its only caller).
- `final_score_white_relative` removed from `PositionRecord` entirely; `final_score`'s doc
  comment updated to show the one-line client-side computation
  (`if side_to_move==White {final_score} else {-final_score}`) instead of promising a
  precomputed field. `check_move.nu` (this session's scratch tool) updated to compute it
  itself — the exact "let the client keep track of if that is white or black" the user asked
  for.
- `ai/mod.nu`'s `chess-coach` system-prompt example schema updated (`"side": "white"` →
  `"mover": "us"`, with a note distinguishing `mover` — always translated, never a color —
  from `data`'s piece references, which correctly stay real color/square).
  `chessdb/sync.nu`'s `tactical_events` table was checked and needed no change: its `side`
  column reads straight from the raw failure-lattice structs' `piece.color` (real,
  `PieceRef`-backed), never from `Concept`/`GatedIssue` — a different, correctly-real-color
  field that was never part of this problem.

One test needed more than a mechanical rename:
`king_exposed_concept_is_invariant_to_side_to_move` asserted the *same* `.side` came back
regardless of who was to move — correct under real-color labeling (the same real king's
exposure is a fixed fact), but backwards under mover-relative labeling, where the same real
fact is *supposed* to flip between `Us`/`Them` as `side_to_move` flips. Renamed to
`king_exposed_concept_tracks_the_real_king_relative_to_whoever_is_to_move` and rewritten to
assert `movers[0] != movers[1]` instead. (A first attempt at this rewrite also asserted the
severity magnitude stayed invariant between the two calls — that turned out to be false for
this position, 121 vs 204, because `king_safety.blended` legitimately incorporates
side-to-move-dependent terms like tempo/mobility; not a bug, just a wrong assumption in the
new test, removed.)

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (101 tests — same count, one test renamed/rewritten, none added or removed), STS
smoke test passes, release build's plugin binary re-registered and round-tripped against the
live nu 0.115.1 shell — confirmed `final_score_white_relative` is gone from `hugm-eval`'s
output and `gated_issues[].mover`/`.phrase` read `"them"`/`"the opponent is up 298cp..."`
(never a color word) on a real Black-to-move, White-up-material position.

---

## 2026-09-01 (continued): Fifth Fruit game — a forced-move ranking where the static tool and a real search engine actually disagreed

Fifth game against Fruit (White), played with the post-audit `Mover`-based `check_move.nu`.
Opening: 1.e4 d5 2.exd5 Nf6 3.d4 Nxd5 4.Nc3 Nc6 5.Nf3 Bg4 6.Qd3 (a real, if minor,
inaccuracy per Fruit's own search — see below) Nb4 7.Qd1 Ncxd4 8.Be3 (pinned in place: every
move of the f3 knight reopens the Bg4→Qd1 diagonal and hangs the queen outright, verified
live before playing — see the `f3d4`/`f3d2` checks below) Nxc2+ 9.Qxc2 Nxc2+ 10.Ke2 Nxa1,
down a queen and a rook for two knights by move 11. Resigned in spirit around move 11 (eval
≈ −4000) and ran the finished game through `fruit_analyze.sh` for the postmortem.

**Correction (same session, caught before this was left standing): the actual queen loss was
*not* a single-ply blind spot.** `d1c2` (`Qxc2`, the forced-or-worse response to
`8...Nxc2+`) was checked with `check_move.nu` before it was played, and it *did* correctly
show `HANGING: Queen@c2 ... safe_to_capture=true` right there in "MY PIECES AT RISK" — the
danger was visible at the very first ply, because the second black knight (still on d4 from
`7...Ncxd4`) was already attacking c2 before Black made any further move at all; no reply was
needed to "create" the threat. The real mistake was three moves earlier, at move 6 (`Qd3`)
— see below, where a genuine one-ply-deeper visualizer *does* catch it. Recorded here rather
than silently deleted, since getting this wrong once is itself worth keeping as a reminder to
verify "the tool didn't see it" claims against the actual saved output before writing them
down.

**Built the visualizer this game's postmortem argued for, and it immediately paid for
itself — three moves before the actual blunder, not at the blunder itself.** Prompted
directly ("there is a two move visualizer no? it is okay to speculate moves ahead just not
calculate specific order"): `check_move_2ply.nu` (session scratchpad), a breadth-first, not a
search — after the candidate move, it enumerates *every* legal opponent reply (`chessdb
legal-moves`'s `mobility_uci`, a new field added to `MobilitySummary` since `chessdb
apply-uci` only accepts UCI) and re-runs the same "MY PIECES AT RISK" check on each resulting
position, with no ranking, no opponent "best move" chosen, nothing minimaxed — pure
enumeration of what's reachable, deliberately staying on this project's side of the
"pathfind the graph, don't calculate the exchange" line (`PLAN.md`). Run against the actual
move 6 (`6.Qd3`, which `check_move.nu`'s single-ply check had shown as completely clean —
`final_score_white_relative: 53`, "MY PIECES AT RISK: (none)"), it immediately surfaced
`MOVER_FAVORED: Queen@d3 ... Winning` after *six different* knight replies (`Nxe3`, `Nb4`,
`Nf4`, `Nc6-b4`, `Nc6-e5`, `Bf5`) — including the exact `...Nb4` that was actually played and
led, three moves later, to the queen being lost outright. `check_move.nu`'s single-position
check structurally cannot see this: nothing attacks the queen in the position right after
`Qd3` itself, only in several of the positions one ply past it. This is the real,
concretely-demonstrated case the "not a search, just enumeration" design is for — worth
formalizing (promoting out of the scratchpad, wiring into the regular per-move check) in a
future pass; not done in this one.

**A sharper, more novel finding: at the actual forced juncture, the static tool's own
ranking of the two legal replies disagreed with what Fruit's real search preferred.** After
`8...Nxc2+`, exactly two legal replies exist (`chessdb legal-moves` confirmed it): `Kd2` and
`Qxc2`. `check_move.nu` ranked them by `final_score_white_relative` immediately after each —
`Qxc2`: −187, `Kd2`: −1374 — so `Qxc2` looked like the clearly-better, "least bad" choice by
a wide margin, and that's what got played. But re-running the finished game through
`fruit_analyze.sh` shows Fruit's own search, at that exact position, preferred **`Kd2`**
(`ponder e1d2` at ply 16) over `Qxc2` — the opposite ranking. The static tool's snapshot
eval at each resulting position is plausible in isolation (`Kd2` does walk into a discovered
attack on the exposed king, which the tool correctly flagged as `MOVER_FAVORED ... Rook@a1
... Winning`), but a real search evidently finds `Qxc2`'s consequences (losing the queen to
the second knight's fork, unconditionally) worse over the following few plies than `Kd2`'s
immediate exposure. This is the concrete, numbers-attached version of "that's Stockfish
thinking, not what we can do" from earlier in this session (2026-08-31 entries): a forced,
two-option decision where correctly reading *both* resulting positions' immediate tactics
still wasn't enough to match a real engine's choice, because the engine is comparing full
lines, not single resulting snapshots. Not something to fix in this pass — recorded as a
concrete, load-bearing example of the gap between this project's static failure-lattice and
an actual search, for whenever that boundary needs re-justifying.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (101 tests, no new ones needed — `mobility_uci` is a same-order sibling of the
already-tested `mobility_san`, no new logic to assert on beyond "shakmaty's own `UciMove`
formatting," already exercised elsewhere in `core.rs`), release build's plugin binary
re-registered and round-tripped against the live nu 0.115.1 shell — confirmed `legal-moves`
now returns `mobility_uci` and `check_move_2ply.nu` runs end-to-end against the real move-6
position above, both by hand-reading its output and cross-checking a couple of the flagged
replies (`Nb4`, `Nxe3`) directly against `chessdb hugm-eval` on the resulting FEN.
`check_move.nu`'s output for both legal replies to `8...Nxc2+`, and `fruit_analyze.sh`'s
full move-by-move White-relative curve for the game, are the source data for the earlier
findings in this entry.

---

## 2026-09-01 (continued): Sixth Fruit game — `mate_in_1_exists` was fully computed and completely invisible in the field actually being read

Sixth game against Fruit (White), using the previous entry's 2-ply visualizer alongside
`check_move.nu`. A pawn-grabbing queen infiltration down the h-file/kingside diagonal
(`13...Qxf3`, `15...Qxh3`) got progressively worse despite every candidate being checked
before playing. Move 17 (`Qa5`) was checked and read clean — `MY PIECES AT RISK: (none)`,
score `-1019` (bad, but nothing forced) — and played. Black replied `17...Qxh2#`: the queen
delivering mate, defended by the bishop on d6 down the long a3–f8/h2 diagonal, king boxed in
by its own `f1` rook and `f2` pawn.

**Root cause: `sensor_report.mate_in_1_exists` was already `true` on the position right after
`17.Qa5`, and neither `render_explanations` nor `render_structured_explanations` ever
mentioned it.** Verified directly: `chessdb hugm-eval --verbose true` on that exact position
returned `mate_in_1_exists: true` in the structured `sensor_report`, but the `.explanations`
field — the one field `check_move.nu` (and every other live-play check this whole session)
actually prints and reads — said nothing about it at all. Grepping `position.rs` confirmed
why: `mate_in_1_exists` is computed once in `build_sensor_report` and stored on
`SensorReport`, but its only consumer was `extract_concepts` (`concepts.rs`), which only
ever reaches a caller through the ELO-gated `gated_issues` path (`--player-elo`) —
never called by this session's plain `--verbose true` checking. The single most decisive
fact a chess position can have (severity 1000 in `extract_concepts`'s own ranking, above
even a full queen's material swing) was fully computed and completely unreachable from the
one output actually being trusted.

**Fixed at the source, not just patched in the scratch script**, since this affects every
caller of `.explanations`/`explanations_structured`, not just this session's tooling: both
`render_explanations` and `render_structured_explanations` (`position.rs`) now check
`sensor.mate_in_1_exists` directly and unconditionally — no `--player-elo` required — and
push it as the very first entry, ahead of every other phrase, matching
`extract_concepts`'s own priority ordering. New regression test,
`fruit_game_six_mate_in_1_was_computed_but_never_surfaced_in_explanations`
(`tests/known_games.rs`), anchored on the exact real position (`r3k2r/2p2ppp/p2bp3/Qp6/
3P4/2P1B2q/P1P2P2/1R3RK1 b kq - 1 17`), asserts both renderers now carry the warning and
that it's first in `render_explanations`'s output. Also added an explicit, louder check to
`check_move.nu` itself — `sensor_report.mate_in_1_exists` printed as its own `!!! MATE IN 1
EXISTS !!!` line before even "MY PIECES AT RISK," since a script this session has been
trusting all game should never depend on a human noticing one sentence buried in a paragraph
of explanation text for the single most catastrophic possible signal. Verified live against
the exact position: the warning now appears, first, through the rebuilt release plugin.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (102 tests, the new mate-in-1 regression passing on the first run), STS smoke
test passes, release build's plugin binary re-registered and round-tripped against the live
nu 0.115.1 shell, confirming the new warning appears (and appears first) on the exact
position that caused the loss.

---

## 2026-09-02: Seventh Fruit game — avoiding `hugm-eval`'s untested score, the `position-eval` skill's reasoning worked early, and the multi-step verify habit caught several illusory captures late

Seventh game against Fruit (White), explicitly avoiding `final_score_white_relative` as a
ranking basis per the user's request ("avoid hugm, it is not battle tested... build a
position evaluation skill"). Practical outcome: material stayed level or favorable through
roughly move 20, then a fork (`23...Nb3`, hitting both Ra1 and Bd2) started a decline that
ended in a clearly lost rook ending (down a bishop and three pawns) by move 30 — resigned in
spirit there.

**Where the qualitative-reasoning approach genuinely changed a decision, correctly.** Move 8
(after `7...Qd5`): the raw score favored `Qd3` (-197) over `Qe2` (-228), and `Qd3` was
literally the square that got the queen harassed for the rest of the previous (sixth) game.
Checked the actual knight geometry independently (`b4`/`e5` both attack `d3`, neither
attacks `e2`) before trusting the intuition, confirmed it, and played `Qe2` against the
score's preference — the queen was never once threatened by a knight hop for the rest of
this game, unlike last time. A second case at move 18: instinct said "contest the open
d-file with `Rad1`," but checking `positional.open_files` directly showed White's own `d4`
pawn still blocked that file for a white rook there — the naive positional instinct was
wrong and the score's preference (`Re2`) was actually correct. Both cases used the same
discipline: pull the structured facts, reason from them explicitly, don't take either the
score or a generic heuristic ("open files are always good") at face value.

**The multi-step "does this survive the opponent's real reply" habit (established two games
ago) caught at least four separate illusory-capture traps this game, all with the same
shape**: a snapshot immediately after MY capture shows a huge score
(`Nc4-e5` "fork" at +208, `Rxd4` at +387, `Rxd4` again at +49, ...) because the tool is
scoring the position *before* Black's forced recapture, not after. Every one of these was
checked by directly applying Black's best/only reply (`chessdb apply-uci` + `chessdb
legal-moves`) before committing, and three of the four turned out to be real traps (the
first `Rxd4` walked into `fxe5`/`Rxd4` counter-recaptures worth -620 to -1546; the second
`Rxd4` attempt walked into a rook fork on `Bd3`/`Nc4` worth -1919) — while the *third* `Rxd4`
(move 29, after the knight had relocated to `d6` and physically blocked the d-file) was
verified genuinely safe and played correctly. Same mechanical check, three different correct
verdicts depending on the exact position — this is precisely why "check the specific
position, don't apply a rule of thumb" matters even for a pattern that's bitten the same way
repeatedly.

**Where the deeper checking still had a real gap.** `fruit_analyze.sh`'s postmortem shows
the single largest White-relative swing of the game (-283 → -642, ply 55) at move 28, where
both `Nc4` and `Bd3` were simultaneously attacked (`27...Bb5`) and neither could be saved.
Fruit's own suggested reply from that exact position was `Bf1` (a bishop retreat never
included in this session's checked candidate list — only `Nd6`/`Ne3`/`Na5`/`Bc2`/`Be2`/`Bb1`/
`Na3` were tried). Checked `Bf1` directly against `check_move.nu` after the fact: it scores
*worse* (-1208) than the move actually played (`Nd6`, -1049) by this project's own static
eval — another confirmed instance of the `Kd2`-vs-`Qxc2` gap from two games ago, where a real
search and this project's static evaluator disagree at a genuinely hard position, and
neither can fully resolve which is right without deeper search this project deliberately
doesn't do. Recorded as another concrete data point for that same, already-documented
boundary — not something to chase further in this pass.

Verified: no code changes this entry (live-play + postmortem only). All specific claims
above (the `Qe2`/`Qd3` knight-geometry check, the `d4`-pawn open-file blockage, all four
illusory-capture verifications, and the `Bf1` comparison) were independently checked against
`chessdb legal-moves`/`apply-uci`/`hugm-eval` before being written down, not asserted from
memory.

---

## 2026-09-02 (continued): Eighth Fruit game — the score stripped from `check_move.nu` entirely, a real deviation reasoned out from a known trap, and an honest limit on what avoiding one pattern actually buys

Eighth game against Fruit (White), with `final_score`/`final_score_white_relative` removed
from `check_move.nu`'s output entirely (not just deprioritized) per direct user feedback:
"the scores, the numbers are not reliable... they cloak real visibility and counter the
skill of reasoning." This game replayed the seventh game's opening near-identically (both
sides largely repeated their moves through move 25), which made it a clean natural
experiment: same position, deliberately different decision at the one point a real
improvement had been reasoned out.

**The repeat validated three earlier findings cleanly**, using only structural facts, no
score: `Qe2` over `Qd3` at move 8 (still never harassed by a knight all game), the `d4`-pawn
blocking the `d`-file for `Rad1` at move 18 (still correctly avoided), and the same
illusory-capture pattern from four separate `Rxd4`/knight-foray attempts, each re-verified
against Black's actual best reply before committing rather than assumed from last time.

**The one deliberate deviation, and an honest result.** At move 26 (retreating the e5
knight under attack from `25...f6`), last game's `Nc4` led directly to `27...Bb5` forking
both the knight and the bishop — the single most damaging tactical shape of that whole game.
This time, checked every legal retreat square first and specifically verified (via
`chessdb legal-moves`) that `Nf3` cannot be forked the same way, since `Bb5` doesn't reach
`f3` the way it reaches `c4` — confirmed correct, and played `Nf3` instead of repeating
`Nc4`. It didn't help. Black played `26...Nxf3` immediately, and the recapture (`27.gxf3`)
opened a completely different problem: a bishop that later reached `h5` had a clean,
empty diagonal straight to `d1` (`h5-g4-f3-e2-d1`), forcing the same "save the more valuable
piece" trade-off game seven hit, just via an entirely different geometric route, ending in
the same kind of decisive material loss (down two bishops and two pawns by move 30).
Re-running the finished game through `fruit_analyze.sh` confirms this wasn't a wash — Fruit's
own real search shows `26.Nf3` cost 335cp more than continuing with `26.Nc4` would have
(ply 51: −228 → −563, the largest single swing of the game), and Fruit's own suggested reply
to `25...f6` was `Nc4`, the exact move this session deliberately avoided. Checked both
candidates directly against this project's own static tool at that position too — neither
showed a red flag for the other, confirming the difference lives in consequences several
moves deep that no single-position check (structural reasoning or static eval alike) can
see.

**Why this is worth keeping, not walking back.** The reasoning that produced `Nf3` was
sound on its own terms — it correctly, verifiably closed the *specific* hole `Nc4` opened
last time (confirmed: `Bb5` really cannot fork a knight on `f3`). What it couldn't do, and
what nothing in this project's design does, is see that closing one specific hole doesn't
guarantee the position doesn't have another one nearby that a real multi-ply search would
find and a single-position (however carefully reasoned) check will not. This is the same
`Kd2`-vs-`Qxc2` / `Bf1` boundary from the two entries above, now with a third confirmed
instance, and with the added, more pointed data point that *this session's own reasoning*,
not just the discarded numeric score, is equally subject to it. Reasoning from structured
facts is still the right replacement for trusting an untested formula — it produces
verifiable, checkable claims instead of a fake-precise number — but it is not a substitute
for search, and shouldn't be oversold as one.

Verified: no code changes this entry (live-play + postmortem only, following up on the
`check_move.nu` scratch-tool edit already made this session). The `Nf3`-cannot-be-forked
claim, the `Nc4` structural comparison, and the `fruit_analyze.sh` swing/bestmove numbers
above were all checked directly before being written down.

---

## 2026-09-02 (continued): Ninth game — a queen-pawn opening with a real plan, a genuine positional/tactical find it produced, and a real misread under pressure at the end

Ninth game against Fruit (White), direct user feedback after the eighth game: "your
evaluation strategy is very defensive tactically and you are missing positional wins... try
again with a queen pawn opening." Audited all eight prior games' move-selection pattern and
found the actual failure mode wasn't (only) the score, addressed already this session — it
was that "tactically safe" had quietly become the entire decision procedure. Filter
candidates through `check_move.nu`, discard anything that hangs material, play whichever
survivor looked like normal development — never actually comparing the survivors'
*positional* consequences against each other, and never starting from an actual plan.
Updated the `position-eval` skill with an explicit fix for this (name a plan before
generating candidates; compare survivors against each other, not each one alone against
"is this acceptable") before playing.

**Opened 1.d4** for the first time this session (all eight prior games were 1.e4 into the
same Scandinavian line). Black met it with a Nimzo-Indian (`1...Nf6 2.c4 e6 3.Nc3 Bb4`).
Played the classical `4.Qc2` specifically so that if Black ever takes on c3, the queen
recaptures instead of a pawn — no doubled pawns regardless of what Black does. Black did
take (`6...Bxc3`), `7.Qxc3` kept a clean structure and the bishop pair, matching the plan
exactly.

**A real positional/tactical find this produced, not a coincidence.** Black's `10...g5` and
`12...f6` seriously weakened their own king (still on g8 behind both pushed pawns) — a
genuine long-term target identified and acted on with a coherent plan (`11.Be3`→`12.O-O-O`
to get White's own king to safety on the opposite wing→`13.h4` directly challenging the
overextension), not just "avoid losing material" move by move. Separately, at move 9
(`9.cxd5`), verified via `chessdb legal-moves`/re-checking Black's actual best reply
(`exd5`) that capturing on d5 opened the c-file for a genuine, survives-the-recapture fork
on both black knights (`Qc2` already covered `e4` on the diagonal; capturing removed
White's own blocking pawn on `c4` and revealed the file to `c6`) — a real structural
consequence of the position, found by checking, not assumed from a raw fork-detector number
(which as usual needed independent verification: the two capture moves that "executed" the
fork, `Qxe4`/`Qxc6`, both actually hung the queen back to a defended pawn/piece the naive
`Fork.see_cp` didn't price correctly — same known, documented `see_chain` unreliability as
every previous game, caught the same way).

**The position was already decisively worse by around move 19** (a queenside knight
maneuver Black executed, `...Nd4`/`...Nf5`/`...Ng3` forking pieces repeatedly, was not fully
parried — `fruit_analyze.sh` shows the swing from roughly even to around −250 across moves
19–24) before the final, decisive error. **The actual last mistake was a misjudgment, not a
missed signal.** At move 24, facing a genuine triple fork (`23...Ne2` hit `Bc1`/`Rg1`/`Qg3`
at once) with every legal response flagged as costly, `check_move.nu`'s output for the move
played (`24.Qh3`) explicitly showed `MOVER_FAVORED (opponent, despite count looking safe):
Queen@h3 1v2 see_cp=800 consequence=Winning` — read, at the time, as a softer warning than a
plain `HANGING` line, when in this specific case it meant exactly the same thing: the g4
pawn (present the whole time, simply not weighted as seriously as a piece) captures the
queen outright next move, `consequence: Winning` at `see_cp: 800` being about as unambiguous
as a warning gets. Played it anyway, comparing it against equally-bad alternatives without
registering that this one specific line was a forced, total queen loss rather than a
survivable cost. `24...gxh3` won the queen for a pawn, ending the game.

**What actually needs fixing, precisely stated:** not a tooling gap — `check_move.nu`
printed the `MOVER_FAVORED` line in the same "MY PIECES AT RISK" section, at the same
visual level, as `HANGING`. The gap was in *how the two lines got weighted* when scanning
several bad options under time pressure: `HANGING` was reflexively treated as maximally
severe, `MOVER_FAVORED ... Winning` was read as a softer, "worth a second look" caveat by
habit, even though the field's own definition (a piece that "looks defended by count" but
the mover still wins the exchange outright) makes it every bit as forcing as `HANGING` once
`consequence: Winning` is attached. Read `attacker_count`/`defender_count` and
`consequence` together, every time, rather than pattern-matching on which of the two labels
a line starts with.

Verified: no code changes this entry (live-play + postmortem only; the `position-eval`
skill edit was made and verified in the previous turn, before this game). The `cxd5` fork's
survival past `exd5`, both illusory-capture verifications (`Qxe4`/`Qxc6`), and the exact
`gxh3` recapture claim were all checked directly against `chessdb legal-moves`/
`hugm-eval` before being written down.

---

## 2026-09-02 (continued): `king_exposure` couldn't distinguish a bare king-file from thin flank shelter — traced to a real move in game nine, fixed at the source

User pushback on the ninth-game postmortem above: "you were down significantly before that
[the final blunder], positionally you allowed your king safety to erode with the rook
attack... what you opened up was exploited better by fruit than you." Traced the claim
precisely rather than taking it on faith: White's own c-pawn was traded away at move 9
(`9.cxd5 exd5` — a real, independently-justified tactical point, it opened the fork on both
black knights covered in the entry above), then three moves later `12.O-O-O` castled the
king directly onto that now-completely-pawnless file. Checked `sensor_report.positional
.king_exposure` on the position right after castling: **empty — no exposure signal fired at
all**, at the exact moment White's king landed on a file a rook could walk straight down.
It did, several moves later (`19...Rxe2`, `20...Rc2+`, `21...Rc3`), directly contributing to
the collapse documented in the entry above (a second, independent problem from that game's
final blunder, exactly as the user's pushback said).

**Root cause, found in `extract_king_exposure` (`position.rs`):** `shelter_files` counts how
many of the three files centered on the king (flank-left, king's own, flank-right) have *any*
friendly pawn *anywhere* on them, then only fires the whole signal when fewer than 2 of the
3 qualify. At the position in question, White had a pawn on `b2` (flank) and `d4` (flank,
and two ranks back) — 2 of 3 — so the detector read "shelter present" even though the
king's own file (`c`) was entirely bare. The metric can't distinguish "the file directly in
front of the king is open" (a specific, well-known, materially worse danger — direct
rook/queen access) from "a flank file happens to be thin," because it averages all three
files into one undifferentiated count.

**Fix:** added `king_file_open: bool` to `KingExposure` (`concept_types.rs`) — true whenever
the king's own file has zero friendly pawns, checked and reported independently of
`shelter_files`, and now also a standalone trigger for the whole detector firing (previously
only `attacker_count > 0 || shelter_files < 2` could fire it; now
`|| king_file_open` too, so this exact shape — flanks nominally intact, own file bare — can
never again read as "no exposure"). New regression test,
`fruit_game_nine_castling_onto_a_pawnless_king_file_read_as_zero_exposure`
(`tests/known_games.rs`), anchored on the real position right after `12.O-O-O`, asserts
`king_file_open: true` alongside the unchanged `shelter_files: 2` — pinning down precisely
that this is the exact gap the old metric had, not a general sensitivity increase.

**What this doesn't fix, deliberately left as a judgment call, not a tool responsibility:**
the detector now reports the fact correctly, but *choosing not to castle onto a bare file in
the first place* is a decision that has to happen before the move is played, informed by
checking both candidate castling squares — a tool firing accurately on the position that
already exists doesn't substitute for that proactive check. Added to the `position-eval`
skill directly (`.claude/skills/position-eval/`) as an explicit step: check
`king_exposure`/pawn structure on *both* castling candidates before choosing one, and treat
`king_file_open` as a materially worse signal than a thin flank file.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (103 tests, the new king-file regression passing on the first run), STS smoke
test passes, release build's plugin binary re-registered and round-tripped against the live
nu 0.115.1 shell, confirming `king_file_open: true` now appears on the exact real position
that used to read as clean.

---

## 2026-09-02 (continued): Tenth Fruit game — Réti opening, a real fork traced correctly, a `HANGING` read under-weighted, and a Fruit-search-confirmed collapse point

Played White with the Réti (`1.Nf3`), intending `2.c4` against `1...d5`; Black played
`1...Nc6` instead, so the game transposed into a general hypermodern setup (`c4`, `g3`,
`Bg2`, `O-O`, `d3`, `Nc3`) rather than the actual gambit line. Full move list (UCI):
`g1f3 b8c6 c2c4 g8f6 g2g3 e7e5 f1g2 f8c5 e1g1 d7d6 d2d3 e8g8 b1c3 c8g4 h2h3 g4e6 d1b3 a8b8
c1e3 c5e3 f2e3 h7h6 d3d4 c6a5 b3b5 c7c6 b5a4 a5c4 a4b3 c4e3 b3a3 e3f1 a1f1 d8b6 f1d1 e5e4
f3e1 d6d5 e1c2 b6c7 c2e3 c7g3 e3f1 g3g5 g1h1 g5h4 h1h2 f6g4 h2g1 g4f2 d1e1 f2h3 g2h3 e6h3`.
Resigned after `27...Bxh3` at `-1375cp` material (roughly a rook and a piece down, no
compensation, no mate threat but no realistic path back against an engine).

**A real find, correctly verified (move 14):** `find_outnumbered` read the `c4` pawn as
safe as a pawn (`2v1 consequence: Losing` for the attacker) — correct at the time. Once
`14...Nxc4` actually happened, that was a *new* tactical fact, not the same one re-asked:
Black's `Be6` bishop, on the same diagonal, now defended the knight and threatened the
queen if White recaptured. `check_move.nu` on the candidate `Qxc4` correctly showed
`HANGING: Queen@c4` with the `Be6` fork behind it; avoided it and retreated `15.Qb3`
instead. Worth naming plainly: "is this square defended" and "is the piece that just moved
onto this square defended" are different questions, and the second one only exists to ask
*after* the capture happens, not before.

**The real structural collapse, per this game's own Fruit-search postmortem
(`fruit_analyze.sh`, White-relative eval), wasn't the final blunder — it was two moves
already documented elsewhere in this file:** normalizing every ply's engine score to a
consistent White-relative sign turns up two sharp jumps, not a gradual slide:

- `29.Qb3 / 29...Nxe3` (the queen retreat off the `c4` attack, immediately followed by the
  knight forking `Rf1`/`Bg2`): `-187cp → -381cp`, a ~194cp swing — the fork that actually
  decided the game, not the earlier `Qxc4??` trap that was correctly avoided.
- `41.Nce3 / 41...Qxg3` (Black's queen capturing the `g3` pawn, the exact king-file-erosion
  pattern from the ninth game's postmortem above, this time on White's kingside instead of
  queenside): `-382cp → -552cp`, a ~170cp swing.

Everything after that (`-552` through `-857` by the game's end) is the engine converting an
already-decisive advantage, not a sequence of fresh blunders — confirmed directly by the
search, not assumed.

**A judgment lesson, not a tool bug, at move 27:** `check_move.nu` on the candidate `Bxh3`
(recapturing the knight that had just given check) printed both `HANGING: Bishop@h3` *and*
`HANGING: Rook@e1` in the same "MY PIECES AT RISK" block, plus a `Qh4 -> Bh3, Re1` fork
below it. Read this as "the rook falls to the fork regardless, so at least trade off the
knight first" — correct reasoning about the rook (it was already lost: Black's own prior
move vacated `f2`, opening the `h4–e1` diagonal directly onto it, independent of anything
White plays), but the `HANGING: Bishop@h3` line wasn't *also* about that same fork — it
meant Black's `Be6` bishop had a direct, immediate, unrelated recapture on `h3` on the same
diagonal it had used at move 14. Fruit's actual reply (`27...Bxh3`) confirmed exactly that:
a plain bishop trade, not the fork playing out. The move played was still correct (roughly
equal to the alternative king moves, ~10cp worse by the postmortem, and it at least
removed Black's active knight from the board) — but the *reasoning* that got there treated
two independent "HANGING" lines as one connected threat instead of two separate ones. When
`check_move.nu` lists 2+ of my own pieces as hanging in the same block, treat each as an
independent claim requiring its own "why" before comparing candidates, not one shared
story.

Verified: no code changes this entry (live-play + `fruit_analyze.sh` postmortem +
documentation only). Fork/outnumbered verifications at moves 14 and 27 were checked
directly against `check_move.nu` and `chessdb legal-moves` before being written down; the
two named collapse points were read directly off `fruit_analyze.sh`'s output (700ms
movetime per ply), normalized to a consistent White-relative sign by hand rather than
eyeballing the raw alternating-perspective numbers.

---

## 2026-09-02 (continued): single-move screening isn't calculation — built forcing-line tools, found a second fork in game ten's own critical position that live play missed

Named directly: the reason ten straight games against Fruit have ended in decisive losses
isn't primarily the score-reliance or stopping-at-safe patterns already fixed above —
those were real, but the deeper gap is that `check_move.nu` only ever screens *one*
candidate move at a time. It cannot tell you what a forcing sequence leads to two or three
plies out, because the danger can be sitting quietly in a position beyond the one ply it
checks. That's calculation, not move-screening, and nothing in the toolset did it.

Built two new scratchpad tools around the same "structural facts, never a score" discipline
as `check_move.nu`:

- `forcing_moves.nu <history>` — lists every legal check and capture for the side to move
  (tagged straight from `mobility_san`'s own `x`/`+`/`#` notation, no plugin logic added),
  unranked. This is the actual branch list a human calculation starts from.
- `calc_line.nu <history> "<candidate line>"` — applies a whole calculated variation move by
  move and prints hanging pieces, forks, king exposure, and raw material (by piece count,
  never a formula) at *every* ply, not just the last one, stopping cleanly if any move in
  the line turns out illegal.

**Validated directly against a real, already-documented position from this file:** ran
`calc_line.nu` on the exact `14...Nxc4 15.Qb3 15...Nxe3` sequence from the tenth game's
postmortem above. It reproduced the known `Nxe3` fork on `Rf1`/`Bg2` — but also surfaced a
**second, simultaneous fork that live play never saw**: at that same position, `Be6` sits on
the intersection of two open diagonals, hitting `Qb3` (`e6-d5-c4-b3`, all three squares
empty) and the `h3` pawn (`e6-f5-g4-h3`, also clear) at once. Checked the geometry by hand
against the resulting FEN
(`1r1q1rk1/pp3pp1/2ppbn1p/4p3/3P4/1QN1nNPP/PP2P1B1/R4RK1 w - - 0 16`) rather than trusting
the tool's own fork detector blind — both diagonals really are clear. The `see_cp=1130`
attached to that fork is not being trusted as a number (still the known `see_chain`
unreliability flagged earlier in this file); the geometric fact of the double attack is what
was verified and is what's real. The position after move 15 was worse for White than the
live-play entry above already documented, and the only reason it surfaced now is that the
line was calculated as a whole instead of reacted to one ply at a time.

**Folded into the `position-eval` skill directly** (`.claude/skills/position-eval/`) as a
new required step for sharp/unclear positions: enumerate forcing branches with
`forcing_moves.nu`, walk chosen lines with `calc_line.nu` to a quiet position, *then* apply
the skill's static material/king-safety/structure/activity method to that resulting
position. Calculation gets you to the position worth judging; it doesn't replace the
judging, and the tools still never rank or score anything — every branching decision stays a
reasoned judgment call, same as before.

Verified: both scripts run cleanly against the live plugin (`chessdb apply-uci`,
`chessdb legal-moves`, `chessdb hugm-eval --verbose true`, all already-shipped commands —
no Rust changes this entry). `forcing_moves.nu` correctly reduced 37 legal moves at the
pre-`14...Nxc4` position down to exactly the 4 real captures and 0 checks; `calc_line.nu`
correctly detected and reported an illegal-move stop condition during script development
before being fixed, confirming the guard works, not just the happy path.

---

## 2026-09-02 (continued): Eleventh Fruit game — resigned on move 12 after hanging a bishop, and the cause was a reading error, not a visibility gap

User clarified the actual point of these games directly: developing tools *and* judgment
for when a fact is genuinely invisible (needs a new tool) versus when it's visible but
uncalculated (needs deeper, deliberate calculation) versus — this game's real lesson — when
it's visible, already surfaced by an existing tool in plain language, and simply misread.
Relocated the live-play tools out of the session scratchpad into the repo first
(`nu_plugin_chessdb/scripts/play/`, see its `README.md`) since they'd been sitting in a
session-tied `/tmp` path that wouldn't survive an actual restart, only a compaction.

Played `1.e4`, Fruit answered with a Scandinavian (`1...d5`), transposing after `2.exd5
Nf6 3.c4 c6 4.Nc3 cxd5 5.cxd5 Nxd5 6.Nxd5 Qxd5 7.Nf3 Nc6 8.d4 Bf5 9.Qb3` into a fully
calculated, verified sequence: `9.Qb3` creates a mutual attack with Black's centralized
queen along the `b3-c4-d5` diagonal, and `9...Nxd4??` was checked and confirmed (via
`calc_line.nu`) to lose the queen outright to `10.Qxd5`, since nothing defends the queen
once the knight moves away from the diagonal. Black correctly avoided that and traded,
`9...Qxb3 10.axb3`, reaching a sound, level position — doubled White b-pawns as a real
long-term factor, offset by an open a-file and a clear development lead.

**The actual blunder, move 11:** `10...Be4` centralized Black's bishop, attacking `Nf3`
(safely defended by `g2`). `check_move.nu` on the candidate `11.Bd3` printed
`HANGING: Bishop@d3 value=330 safe_to_capture=true` — this was read as "an even trade,
Black captures and I've lost nothing," by direct (and wrong) analogy to the `9.Qb3` mutual
attack a few moves earlier, where the *same shape* of warning had resolved into a fair
trade. It doesn't. The load-bearing difference: in the queen case, a **third piece** (`a2`
pawn) independently defended `b3`, giving a real recapture after `Qxb3`. In the bishop
case, nothing else White owned reached `d3` at all — checked by hand afterward: no rook,
king, knight, or pawn attacks that square. `safe_to_capture=true` on a candidate's own
resulting position already answers the only question that matters ("can this be captured
with no recapture") directly and unconditionally; treating it as ambiguous by pattern-
matching to a structurally different earlier position was the entire error. Fruit played
`11...Bxd3`, and there was no recapture — confirmed directly (`chessdb apply-uci` replay +
`hugm-eval`): `-958cp`, a clean bishop down, bishop pair now Black's, zero compensation
(Black's king not exposed, nothing of White's touches the piece, it retreats next move
uncontested). Cross-checked against `fruit_analyze.sh`'s own search: score sat near `-53`
(White-relative) through move 10, then crashed to `-392` the instant `11.Bd3` was played —
almost exactly one bishop's value — and Fruit's own suggested move at that exact ply was
the safe `11.Be3`, never `Bd3`. Resigned; no realistic recovery path against real engine
play from a clean, uncompensated piece deficit this early.

**Explicitly not a tool problem.** `check_move.nu` produced the correct, unambiguous
answer before the move was ever played — this is a genuine negative-control data point
that the score-stripping and structural-fact discipline built up over the first ten games
is sound; the failure mode this game exposed is a new, distinct one: overriding a plain,
already-correct answer with a similarity-based shortcut instead of reading it as absolute.
Fix is a reading rule, not a tool change: `safe_to_capture: true` on a candidate's own
piece is a terminal veto by itself, full stop, regardless of whether the position
resembles an earlier one that resolved differently — the resemblance itself is exactly
what needs re-checking (specifically: is there an independent third defender of the
landing square, not just symmetric mutual attack) before assuming the earlier case's
resolution carries over.

Verified: no code changes this entry (live play + `fruit_analyze.sh` postmortem +
documentation only). The `9...Nxd4??` refutation and the `11.Bd3` blunder's lack of any
recapture were both checked directly against `calc_line.nu`/`chessdb apply-uci` before
being written down, not assumed from board-visualization alone — the second check is what
caught the misread this entry describes, just one move after it happened live rather than
during the live move itself.

---

## 2026-09-02 (continued): `chessdb square-control` — a spatial view of what one piece controls, prompted directly by the `Bd3` blunder

User's own diagnosis of the `Bd3` blunder above, offered directly rather than waited for:
mental file/rank-difference arithmetic ("does this diagonal reach that square") is exactly
the wrong way to answer a question the engine already answers correctly internally —
proposed padding a FEN with marks for every square a piece controls as a more
"topographical" way to see it, instead of a flat list of square names to place by hand.

`Board::attacks_from(sq)` (shakmaty, occupancy-aware — stops at the first blocker in each
direction, includes squares held by either side) was already used internally
(`attacked_squares` in `core.rs`, backing `chessdb attack-summary`'s whole-board view), but
nothing exposed it for one specific piece. Added `core::square_control(fen, square)` —
returns the piece on that square (`None` if empty) and every square it controls, wrapped as
`chessdb square-control --square <sq>`. Deliberately a thin wrapper around the same tested
primitive `attack_summary` already uses, not new geometric logic: the whole point was to
stop re-deriving this by hand, so the fix can't itself be another hand-derivation.

Four unit tests pin down the behavior directly (`core.rs`, `square_control_tests`): an empty
square returns `None`/`[]`; a corner knight (`Nb1` in the start position) controls exactly
its 3 reachable squares; a boxed-in bishop (`Bc1`, start position) controls exactly its two
blockers (`b2`, `d2`) and nothing past them — the blocker-awareness a hand-rolled diagonal
walk would have to get right on its own; an invalid square is a labeled error, not a panic.

Built `nu_plugin_chessdb/scripts/play/control_map.nu` on top of it — an 8x8 grid renderer
that marks empty-and-controlled squares `x`, own pieces it defends `(P)`, enemy pieces it
attacks `[p]`, and the piece itself `*B*`, reading everything else straight off the FEN.
The rendering script does no geometry of its own (file/rank math, diagonal walks) — it only
places characters at squares `chessdb square-control` already named, which is the actual
fix: the class of code that caused the blunder doesn't get a second chance to exist here.

Validated directly against the real blunder position: rendering Black's bishop on `e4` (the
position right before `11.Bd3` was played) shows `d3` marked `x` — a bare "don't go there"
signal, no diagonal arithmetic required, right where the actual mistake happened. Also
validated the control/mobility distinction on a pawn (`e4` white pawn controls `d5`/`f5`
only, not its forward push square `e5`) — confirming `controls` answers "what does this
piece see," not "where can it legally move," which matters most exactly for pawns.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean, full `cargo test`
suite green (107 tests passing, 1 pre-existing ignore, the 4 new `square_control` unit
tests included), release plugin rebuilt and `plugin add`/re-registered against the live nu
shell, `chessdb square-control` and `control_map.nu` both smoke-tested live against real
positions (start position, the actual game-11 blunder FEN, an isolated pawn) before being
trusted.

---

## 2026-09-02 (continued): Mining shakmaty for more topological visibility, and making "defer to shakmaty" a standing principle

User asked for a full planning-and-review pass mining shakmaty for more visibility like
`square-control`, with an explicit standing instruction: chessdb always defers to shakmaty
when it can provide something, not just for live-play tooling but as an architectural
principle (`CLAUDE.md`, new "Chessdb defers to shakmaty for anything geometric" section).
Three research passes (full shakmaty 0.30.1 source read; full chessdb hand-rolled-geometry
audit; baseline of already-exposed tooling) turned up a precise, bounded set of real gaps,
planned in `/home/mjfarnbach/.claude/plans/snug-popping-turtle.md` and executed in two
waves.

**Wave 1 — `chessdb square-attackers` (additive).** `Board::attacks_to(sq, attacker,
occupied)` is the reverse of `attacks_from` — "what attacks this square" instead of "what
does this piece see." `square-control` only answers the first question; the second is
arguably the more directly useful one for "is it safe to move a piece here," and works on
an empty target square (no piece has to be sitting there yet). Added `core::SquareAttackers`
/`square_attackers`, `chessdb square-attackers --square <sq>`, and
`scripts/play/attackers_map.nu` (same grid-rendering shape as `control_map.nu`, distinct
legend: `<P>` white attacker, `{p}` black attacker). Also added `is_light: bool` to the
existing `SquareControl` struct (bishop color-complex reasoning). Validated directly against
the real `Bd3` blunder position: `d3` shows `0` white attackers, `1` black — a more direct
"don't go there" signal than `square-control` gave, since it doesn't require first guessing
which enemy piece to check.

**Wave 2 — replace hand-rolled geometry with shakmaty primitives, tiered by risk.**

*Tier 1 (trivial, mathematically identical formulas):* `chebyshev_distance` now calls
`Square::distance` directly (shakmaty's own doc-test confirms it's exactly
`max(file_dist, rank_dist)`, the same formula the hand-rolled version computed).
`piece_coordination_score`'s per-axis distance casts now use `File::distance`/
`Rank::distance` — notation only, the Manhattan sum itself is unchanged (no shakmaty
primitive computes that directly).

*Tier 2 (`detect_skewers` — a genuine algorithm rewrite, A/B-verified before shipping).*
Hand-walked 8 hardcoded direction tuples one square at a time via
`File::offset`/`Rank::offset`, checking occupancy manually — already flagged as a
deliberately-deferred gap earlier in this file. Rewritten to mirror its sibling
`detect_pins`'s pattern: for each slider, `reach` is its current attack set via
`attacks::rook_attacks`/`bishop_attacks`; for each enemy piece in `reach` (`front_sq`),
recomputing reach with `front_sq` removed reveals at most one new occupied square along
that exact ray (`reach_behind & !reach`), which is the "behind" candidate. Implemented
under a temporary name alongside the original, ran both against 8 real positions (the two
dedicated skewer tests plus 6 FENs already covered by `tests/known_games.rs`) for both
colors, asserted byte-identical `(count, examples)` output — passed on the first try, no
bugs found — then deleted the old implementation and renamed. New permanent regression
test, `runs_cleanly_on_every_known_game_and_motif_test_fen`, keeps that same position set
as a standing check.

*Tier 3 (feeds the tuned, normally-unread `final_score` — same discipline, one real
finding).* `pawn_structure_score`'s candidate/weak front-span walk now reuses the file's own
existing `in_front` bitboard-shift helper instead of re-deriving the same forward span by
manual rank-stepping (an in-project duplication fix as much as a shakmaty one).
`piece_activity_score`'s and `pawn_structure_score`'s pawn-capture-destination computations
now use `attacks::pawn_attacks(color, sq)` directly instead of manually deriving diagonal
squares from `File::offset`, matching what `pawn_attack_mask` already did elsewhere in the
same file. `extract_king_exposure` was surveyed and **deliberately left unchanged** — it
already uses `File::offset`/`Bitboard::from(File)` (legitimate shakmaty primitives, not raw
arithmetic); the only "manual" part is checking 3 specific adjacent files individually,
which is unavoidable for "how many of these 3 files have a pawn" (a single combined-mask
popcount would miscount doubled pawns on one file as two files) — not a real gap.

**The one real finding: `king_safety_score`'s shield/storm loop looked like two independent
per-file queries and wasn't.** The original loop's `break` — triggered by finding the first
enemy pawn ascending from rank 1 — exits the *entire* loop, not just the storm half, so
"nearest own pawn" (shield) and "nearest enemy pawn" (storm) are coupled: once an enemy
pawn is found, own-pawn tracking stops too, even if an own pawn sits at a higher rank that
was never reached. For Black in particular, whose enemy (White) pawns typically sit on the
low ranks the ascending loop reaches first, this means `shield_rank` in the original
algorithm almost never actually sees Black's own rank-7-ish pawns — a real, load-bearing
quirk of the existing tuned formula, not a bug to fix (behavior had to be preserved
byte-for-byte, whatever it was).

A first rewrite split shield/storm into two independent `Bitboard::first()`/`.last()`
queries per file — clean-looking, compiled, and passed the *entire* `cargo test` suite
(no test asserts `king_safety.blended`'s exact numeric value on any position). Caught only
by the explicit before/after numeric diff this pass's own discipline requires: captured
`groups.{king_safety,pawn_structure,piece_activity,tactical}.blended` and `final_score` for
9 real positions with the new code, `git stash push -- src/eval/position.rs` to isolate the
pre-Wave-2 baseline, rebuilt, recaptured the same values with the old code, diffed.
`pawn_structure`/`piece_activity`/`tactical`/`final_score` matched exactly everywhere
(confirming the other Tier 2/3 rewrites above are genuinely behavior-preserving) —
`king_safety` differed on **every single position**, largest gap `-217` on the plain start
position. Reverted the split back to the original coupled loop (kept using shakmaty's own
`Rank`/`Square`/`Bitboard` types throughout, as it already did — the sequential coupling is
inherent to what the computation actually does, not a sign of avoiding a library primitive),
rebuilt, re-diffed: byte-identical across all 9 positions, `final_score` included.

This is the whole reason the A/B-diff discipline exists rather than trusting `cargo test`
alone for anything touching the tuned scoring table — recorded here as a genuine "tried,
caught by verification, reverted" result, which is a more valuable outcome than either
silently not attempting it or shipping the subtle bug undetected.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean throughout, full
`cargo test` suite green (113 tests passing, 1 pre-existing ignore — measured directly,
not derived), `cargo test --test sts_positional -- --ignored` passes, the 9-position numeric
A/B diff byte-identical end to end, release plugin rebuilt and `plugin add`/re-registered
against the live nu shell, `chessdb square-attackers` and `attackers_map.nu` smoke-tested
live against the real `Bd3` blunder position and a constructed both-sides-attack position.

---

## 2026-09-02 (continued): one shared board-overlay convention instead of each grid script inventing its own legend

User named the actual problem with `control_map.nu`/`attackers_map.nu` directly: `()`
meant something different in each script, and asked for a real convention —
"interoperable with the conventional 64-bit map" and able to "visualize concepts like
control, overlap." Answered the design question first (is layering multiple bitboards on
one grid even legible — yes for 2 layers, workable for 3, not beyond that, since it's
combinatorial: N boolean layers means 2^N states to distinguish with one glyph per
square), then built it once approved.

**The convention** (`nu_plugin_chessdb/scripts/play/board_overlay.nu`): any layer is just
a *square set* — a `list<string>` of algebraic squares, exactly what `controls`/
`attacked_by_white`/`attacked_by_black`/etc. already return from the plugin, and exactly
what a `shakmaty::Bitboard` is, just serialized as square names instead of a raw u64 — no
adapter needed, any existing or future command returning a square list is a valid layer
for free. Fixed glyph grammar, not caller-chosen: layer 1 → `()`, layer 2 → `[]`, layer 3
→ `{}`, 2+ layers active on one square → `<>` regardless of which combination overlapped
(distinguishing the exact combination would need a glyph per combination, illegible past
2 layers anyway — the per-layer counts printed in the header answer that precisely if
needed), an optional `--highlight` square → `*X*`, overriding everything.

Rerouted both existing scripts onto it and added a genuinely new one:
- `control_map.nu`: the one "controls" set split into 3 mutually-exclusive layers by
  occupant (own piece defended / enemy piece attacked / empty controlled) — `<>` never
  fires here by construction, which is correct, not a gap.
- `attackers_map.nu`: white-attackers layer 1, black-attackers layer 2 — these genuinely
  can overlap (a square attacked by both colors), so `<>` is real and meaningful here.
- `control_overlap.nu` (new): whole-board version, White-controls vs. Black-controls, no
  single square of interest — built on `chessdb attack-summary`'s
  `attacked_by_white`/`attacked_by_black`, which has returned whole-board data since
  before this session but was never rendered. Answers "who controls the center," "is this
  square actually contested" — questions neither of the other two scripts, both scoped to
  one piece or one square, could answer at all.

Verified all three against known-good output: `control_map.nu`/`attackers_map.nu` rerun
on the real `Bd3`-blunder position and the game-11 bishop's exact 9-square control set
(2 own/1 enemy/6 empty, matching the counts already established when `square-control` was
built) — output semantically identical to before, now through the shared renderer.
`control_overlap.nu` checked on the start position (0 contested, as expected — no contact
yet) and a real middlegame position with genuine central tension (7 contested squares,
correctly marked `<>`).

Verified: no Rust changes this entry — pure Nu-side consolidation and one new script, all
built on already-shipped, already-tested plugin commands (`square-control`,
`square-attackers`, `attack-summary`). Each script smoke-tested live against real
positions before being trusted, per the same discipline as every other tool in this
directory.

---

## 2026-09-02 (continued): Twelfth Fruit game — the board-overlay tools used live for the first time, a real oversight caught cleanly, and an honest look at where the position actually turned

User asked to actually watch a game and see the board-overlay tools (`attackers_map.nu`,
`control_overlap.nu`) used live, not just built. Played White, Nimzo-Indian (`1.d4 Nf6
2.c4 e6 3.Nc3 Bb4 4.e3 Ne4 5.Qc2 f5 6.a3 Bxc3+ 7.bxc3 O-O 8.Nf3 b6 9.Be2 Ba6 10.O-O Nc6
11.Bb2 Na5 12.c5 Bxe2 13.Qxe2 bxc5 14.dxc5 Nb3 15.Rd1 Nxc5 16.Nd4 Qh4 17.g3 Qh6 18.f3 Nd6
19.Nb5 Nxb5 20.Qxb5 Qxe3+ 21.Kh1 Rb8 22.Qc4 Rxb2`), resigned at `-1253` with no tactics
or compensation available.

**`control_overlap.nu` used to assess a real Nimzo-Indian tabiya** (move 5, `Nc3` under
pressure from both `Ne4` and `Bb4`): 33 White-controlled squares, 32 Black, 7 contested —
`c3` itself correctly shown as a genuine stack `<N>`, matching `hugm-eval`'s own
`outnumbered` finding on that square (`consequence: Even`, not a real threat).

**`attackers_map.nu` used repeatedly for real move-selection decisions, not just
demonstration:** caught that `9.Bd3` (the originally planned developing move) would have
*blocked my own queen's diagonal* onto `e4` rather than adding a second attacker as
intended — `Bd3` only showed 1 white attacker on `e4`, not 2, because it sits directly on
the `c2-e4` line the queen was already using. Switched to `9.Be2` instead, which doesn't
self-block. Also used before `9.a3`/candidate captures to confirm real 2v1/3v1 attacker
balances (`e4`, `d4`, `c4` at various points) rather than counting pieces by eye.

**A real oversight, caught cleanly by the existing discipline rather than becoming a live
blunder.** After `21...Rb8`, checked `22.Rd3` via `check_move.nu` and saw `HANGING:
Queen@b5 value=900 safe_to_capture=true` in the output — correctly read as "don't play
this move," but the *diagnosis* was wrong: the queen wasn't hanging *because of* `Rd3`, it
was already hanging to `Rb8` down a fully open b-file (`b7`/`b6` both empty) the moment
Black played `21...Rb8`, independent of anything White does next. `check_move.nu` applies
the candidate and evaluates the *resulting* position's hanging list, so it caught the
problem correctly regardless of which candidate was checked first — the safety net held —
but attributing the cause to the wrong move meant nearly wasting a check on `Qxc5` (also
hanging, for an unrelated reason — `Qe3`'s own diagonal) before checking retreat squares
systematically. Checked seven queen destinations directly (`a4`, `b3`, `b4`, `b6`, `c4`,
`c5`, `d3`, `e2`, `a5`, `a6`) to find the two (`a5`, `c4`) that don't also hang the queen —
both still lose the bishop (`Rb8` was blocking a second problem: once the queen leaves the
b-file, `Bb2` is undefended down the same file), confirming this was a genuinely forced
loss, not a move-order error. **Lesson for the skill, not the tool:** after any opponent
move, check the raw current position's own hanging list directly before evaluating
candidates, so causes get attributed correctly from the start — the candidate-check safety
net still catches the problem either way, but a wrong diagnosis costs a wasted check and
risks missing that *multiple* pieces are compromised by the same root cause (the open
b-file), not just one.

**Where the position actually turned, per `fruit_analyze.sh`'s own search (White-relative,
normalized by hand):** two real drops, not a single blunder. `19.Nb5` itself: `-147cp →
-296cp`, a ~149cp decline — the move was calculated and verified sound in the sub-line
actually checked (`a6` allowing `Nxc7` winning a clean pawn, confirmed via `calc_line.nu`
against the real position), but Fruit's search rated the move below its actual best
regardless of that specific line, and Black's simpler practical reply (`19...Nxb5`
immediately, sidestepping the calculated trap) meant the calculated upside never
materialized. `22.Qc4` (the forced retreat): `-353cp → -669cp`, a larger ~316cp decline —
but this reflects the underlying structural vulnerability (the queen's only legal
recapture square on move 20 sat on the same open file as Black's rook, four moves before
that consequence actually landed), not a fresh error in *which* retreat square got picked;
every one of the seven checked was comparably bad or worse.

Verified: no code changes this entry — live play, `attackers_map.nu`/`control_overlap.nu`
used and relayed throughout (not just at the end), `calc_line.nu` used to verify the
`Nb5`/`Nxc7` sub-line before committing to it, `fruit_analyze.sh` postmortem run and
normalized to a consistent perspective before drawing conclusions from it.

---

## 2026-09-02 (continued): stripping `see_cp`/`consequence`/`centipawns` from every live-play tool — the aggregate-score lesson applied one level deeper

Direct user pushback after Game 12: still being misled by scores, even after `final_score`
was stripped from every live-play tool weeks earlier in this same session. The per-fact
`see_cp`/`consequence` fields on `Fork`/`Outnumbered`/`MoverFavored` entries had survived
that earlier cleanup on the reasoning that each is tied to one concrete, individually-
tested exchange rather than a summed formula — but that distinction doesn't actually hold
up: `find_forks` is still backed by the known-buggy `see_chain` (documented, deferred), and
even the direct-subtraction pricing `find_outnumbered`/`find_mover_favored` use is still a
*computed valuation*, not a raw fact. Game 12's own `19.Nb5` is a real instance of this
having cost something: the fork's `see_cp=900`/`consequence: Winning` looked decisive, the
underlying calculation (verified via `calc_line.nu`, not just trusted) *was* real in the
specific sub-line checked, but Fruit's own search still rated the move below its actual
best (`fruit_analyze.sh`, `-147cp → -296cp` swing) — leaning on the number, even a
per-fact one, produced a move that was good but not the best available.

**Removed from every script's output, not just deprioritized:** `see_cp`/`consequence` on
forks/outnumbered/mover_favored (`check_move.nu`, `check_move_2ply.nu`, `calc_line.nu`),
and the server-generated `.explanations` prose entirely from `check_move.nu` (it embeds
`see_cp`/`consequence`/tropism/initiative scores by construction — no way to launder
individual numbers out of generated sentences without regexing them, and the structured
counts plus the board-overlay tools already cover the same ground). **What stays:**
`attacker_count`/`defender_count` (plain counts, not valuations), piece identity and
standard value (a fixed constant — same numbers the skill already has you count material
with by hand, not a search result), and fork/skewer *target lists* (who's involved, not
whether the exchange is worth it). Forks now print as `attacker=... -> target1, target2`
with no verdict attached.

**New tool, `scripts/play/material.nu`:** raw piece-count material for both sides, and
nothing else — deliberately never touches `material.balance.centipawns`. This was a real
gap: game 11/12 resignation decisions were made by directly reading `.centipawns` values
(`-958`, `-1253`) as a threshold check, exactly the "just check if it's decisive" reflex
the whole score-stripping effort exists to prevent, just one level removed from
`final_score` itself. Now there's no path to that field that doesn't require deliberately
asking for the full `hugm-eval` record — the natural tool for a material check surfaces
only counts.

**`.claude/skills/position-eval/SKILL.md` updated**: step 2a no longer tells the reader to
"read `consequence`/`see_cp`... where present" as part of judging whether a tactical
danger is real — replaced with an explicit instruction to verify via `calc_line.nu`
(walk the real capture sequence, read the resulting raw piece list) or
`attackers_map.nu`/`control_map.nu` (see directly what defends what), never by reading a
precomputed verdict. `safe_to_capture: true`/`false` on `hanging` entries stays fully
trusted — it's a direct legality/capture fact, not a valuation, and nothing about this
change touches it.

Verified: all four edited scripts (`check_move.nu`, `check_move_2ply.nu`, `calc_line.nu`,
plus new `material.nu`) smoke-tested live against real positions from this session's own
game history (the game-10 `Bxh3` fork position, the game-12 resignation position) —
confirmed no `see_cp`/`consequence`/`centipawns` appears anywhere in any script's output,
and that the remaining structural facts (counts, target lists, piece values) still print
correctly. No Rust changes — every field removed from these scripts' output was already
being computed and returned by `hugm-eval`; this is purely a Nu-side consumption-discipline
change, matching how the original `final_score` strip-out was done.

---

## 2026-09-02 (continued): no raw FEN either — the same principle one level further

Direct user follow-up, same conversation: a FEN string is exactly the same class of thing
as a score — a compact, symbolic encoding that has to be mentally parsed back into a board
to actually mean anything, and mis-parsing one by eye is precisely the arithmetic that hung
a bishop in live play (`11.Bd3??`, this file, earlier 2026-09-02 entry). Confirmed and
applied: every script that previously printed `fen: ...` for a human to read now renders an
actual grid instead, via `board_overlay.nu`'s already-established convention, extended to a
"plain board" mode (called with an empty layer list and no `--highlight`) — the legend
section is skipped entirely in that mode, since there's nothing to key.

`check_move.nu` and `check_move_2ply.nu` now render the resulting/attempted position with
the candidate's destination square highlighted (`--highlight`, the moved-to square parsed
from the UCI string) instead of printing its FEN. `calc_line.nu` renders every ply of a
calculated line as its own grid, highlighting that ply's destination square — genuinely
more useful than before, not just equivalently safe: watching the actual board change
square by square through a calculated sequence is closer to how a human calculates than
reading a string of algebraic notation was. `forcing_moves.nu` renders the starting
position once before listing branches. `material.nu` — deliberately left without a grid:
its whole output is already a `list<string>`-adjacent aggregate (piece counts by role),
which satisfies the "list of pieces you can hand-calculate from" alternative the user named
directly; adding a grid there would be redundant with its stated single purpose. FEN
strings still exist inside every script, as plain internal variables threaded between
`chessdb apply-uci`/`chessdb hugm-eval` calls — that's necessary plumbing, not something
printed for a human to read, and nothing about this change touches it.

Verified: all five affected scripts (`check_move.nu`, `check_move_2ply.nu`, `calc_line.nu`,
`forcing_moves.nu`, `material.nu`) smoke-tested live, including each one's illegal-move
error path (confirmed a grid renders there too, showing the pre-attempt position, instead
of a bare FEN in the error message) and `board_overlay.nu`'s new empty-layer/no-highlight
"plain board" mode specifically (confirmed the legend section is correctly omitted, not
just left blank). `grep`-confirmed zero remaining `"fen: ` string literals anywhere in
`scripts/play/*.nu` after the edits. No Rust changes.

---

## 2026-09-02 (continued): Thirteenth Fruit game — an actual checkmate, and the real lesson wasn't where the drama was

First game this session to actually end in checkmate rather than resignation from a clearly
decisive material deficit. Played White, a Scandinavian transposing into a Nimzo-ish
structure (`1.e4 d5 2.exd5 Nf6 3.d4 Nxd5 4.Nf3 e6 5.c4 Bb4+ 6.Nbd2 Nf6 7.a3 Be7 8.Bd3 Nc6
9.Qc2 Nxd4 10.Nxd4 Qxd4 11.O-O O-O 12.Nf3 Qb6 13.Rd1 c5 14.Bg5 h6 15.Bf4 Rd8 16.Rac1 Bd6
17.Bxd6 Rxd6 18.Ne5 Bd7 19.Nxd7 Rxd7 20.Qc3 Rad8 21.b4 Ng4 22.h3 cxb4 23.axb4 Qxf2+ 24.Kh2
Qh4 25.Rg1 Nf2+ 26.Kh2 Rxd3 27.Qe5 Rxh3+ 28.gxh3 Qxh3#`), used every tool built this session
throughout, including live-verifying two real traps mid-game (`Qb3??` hanging directly to
`Qb6`, and a mutual-attack `Qxd3` that loses the queen to a knight recapture) before they
were ever played — the "check current position first" and "safe_to_capture terminal veto"
disciplines both held under real pressure. Got mated anyway.

**Two genuine misses worth naming precisely, and one that wasn't a miss at all.**

1. Move 21 (`b4`), threatened by a mutual-attack fork the tool flagged as `mover_favored`
   on `Qc3`: verified concretely with `calc_line.nu` that a naive `bxc3` was actually
   illegal (wrong side-to-move in the first test — a real methodology bug in the check
   itself, caught and corrected before trusting the wrong conclusion), then correctly
   re-verified with the right move order that `bxc3` *would* have won the queen for a pawn
   had the threatening pawn not been removed — `axb4` was the right, verified call. This
   discipline worked exactly as designed.
2. Move 23 (`axb4`) was followed by `23...Qxf2+`, a real miss: the pre-move analysis
   correctly identified and calculated the *knight's* `Nxf2` fork (`Rd1`/`Bd3`/`h3`, with
   `Kxf2` confirmed illegal via a discovered check from `Qb6`), but never checked whether
   the *queen itself* could capture `f2` directly — a different move with a different
   consequence (check, a second free pawn, no recapture available) that happened to share
   a destination square with the knight fork already under investigation. Two different
   pieces threatening the same square is not one threat to verify once; each needed its
   own check, the same lesson [[chessdb_hanging_lines_independent]] already named for
   `HANGING` entries, now confirmed for candidate opponent replies too.
3. **Move 25 (`Rg1`) is where the game was actually lost, and it looked completely
   clean at the time.** `check_move.nu` correctly reported nothing hanging — genuinely
   true, no single-ply tactic existed. `fruit_analyze.sh`'s postmortem (normalized,
   White-relative) shows this exact move as by far the largest swing of the entire game:
   `-418cp → -1800cp`, dwarfing every other jump including the `Qxf2+` pawn loss two
   moves earlier. The real problem: abandoning the semi-open d-file entirely (moving the
   *other* rook away from it) while the king was already under active pressure from a
   knight and queen, without calculating several moves ahead through the forcing sequence
   that followed (`25...Nf2+ 26.Kh2 26...Rxd3` — infiltrating exactly because nothing
   contested that file anymore). A single-ply "does this hang anything" check cannot see
   this; only calculating the actual forcing continuation several plies deep (`calc_line.nu`,
   already built and used successfully elsewhere in this same game) would have. The
   position-eval skill's existing "calculate forcing lines before judging a sharp
   position" section already says this — the miss here was recognizing the position
   *was* sharp (an active king hunt already underway, not a quiet position) and needed
   that treatment, not that the tool or the discipline was missing.

Verified: `fruit_analyze.sh` run over the complete game and normalized by hand
(White-relative sign) before drawing any conclusion about where the real error was — the
dramatic-looking `Qxf2+`/`Nf2+` sequence was not, in fact, where the largest evaluation
swing happened; `Rg1` was. No code changes this entry — live play, postmortem, and
documentation only.

---

## 2026-09-02 (continued): "track ideas across games" redirected to "make and re-evaluate a plan" — the actual skill worth exercising

User pushback on a proposed durable Fruit-tendencies/opening-idea tracker (memory across
games): "I'm more interested in your ability to make a plan and evaluate it than remember
it." Correct redirect — the game 13 failure wasn't a missing reference to consult, it was
that a plan, once formed, was never re-checked against a position whose character had
already changed. `.claude/skills/position-eval/SKILL.md` gained a new section, "Re-evaluate
the plan every move — don't just state it once and coast," making explicit what "Don't stop
at safe" and "Calculate forcing lines" already implied separately but never stated as one
recurring loop: after every opponent reply, ask not just "what's the positional theme" but
"has the position stopped being the kind of position that theme applies to" — specifically,
has the opponent started delivering forcing moves in sequence, which means every candidate
(including ones that look like quiet consolidation) needs `calc_line.nu` treatment until the
sequence actually resolves, not just the ones that look sharp on sight.

Verified: no Rust or Nu-tool changes this entry — a skill-file addition only, directly
citing the exact game 13 evidence (`fruit_analyze.sh`'s `-418cp → -1800cp` swing on
`25.Rg1`) already established in the entry above rather than a new claim.

---

## 2026-09-02 (continued): "distraction/hyperfixation" diagnosis — checked against the data, one hypothesis held and one didn't

User's sharper read on the game 13 failure: not a knowledge or planning gap so much as
getting distracted — hyperfixating on whichever threat/number is currently in front of the
reasoning, at the cost of the wider picture. Tested this against the actual game rather
than just agreeing with it.

**First hypothesis, checked and rejected:** that `king_exposure` (already computed, just
not surfaced in `check_move.nu`'s default output) was sitting there unread right before
`25.Rg1` and would have caught it. Queried the real position directly — `king_exposure`
was genuinely `null` at that exact moment. It's a *near*-king detector (adjacent-square
attacker count, shelter files); neither `Ng4` nor `Qh4` had reached that ring yet, still a
couple of moves out. The signal hadn't fired because the danger was still converging, not
because it fired and got ignored — this specific fix would not have caught this specific
game. Naming a plausible-sounding mechanism and checking it against real data rather than
trusting it on the strength of sounding right is the same discipline this whole file is
built on; a wrong hypothesis, caught before it became a memory, is a fine outcome.

**Second hypothesis, checked and confirmed:** that `23...Qxf2+` was missed not from a
missing tool but from going deep on the *first* specific threat found instead of wide
across the whole square first. Verified directly: `attackers_map.nu f2` on the real
position right before `23.axb4` shows `f2` attacked by **both** `Qb6` and `Ng4`
simultaneously — the tool that would have shown this was already built, already used
successfully elsewhere in this exact game, and simply wasn't run on this square before
`calc_line.nu` was pointed at the one piece (`Ng4`) a fork list had already named. The
knight's fork was verified correctly; the queen's independent attack on the same square
was never asked about at all.

Added to `.claude/skills/position-eval/SKILL.md`: "Wide before deep — check the whole
square, not just the piece already worrying you." Whenever a square becomes contested —
in a fork's target list, a hanging entry, anywhere — `attackers_map.nu`/`square-attackers`
on that square comes first, establishing the full attacker/defender picture, before
`calc_line.nu` goes deep on any one of the threats it turns up.

Verified: `king_exposure` checked against the real position (not a guessed FEN — caught and
corrected the same guessed-FEN mistake mid-check before trusting the first, wrong result),
`attackers_map.nu`'s `f2` output checked against the real pre-`axb4` position. No Rust or
Nu-tool changes — a skill-file addition, and one hypothesis explicitly logged as tested and
rejected rather than quietly dropped.

---

## 2026-09-02 (continued): Fourteenth Fruit game — the newly-named "wide before deep" pattern caught itself happening live, twice, in the same game

Played White, Queen's Indian/IQP structure (`1.d4 Nf6 2.c4 e6 3.Nf3 Bb4+ 4.Bd2 Be7 5.e3 c5
6.Bd3 Nc6 7.O-O O-O 8.Nc3 d5 9.cxd5 exd5 10.dxc5 Bxc5 11.Qb3 Nb4 12.Be2 b6 13.a3 Nc6 14.Rfd1
Bf5 15.Rac1 Qd7 16.Nb5 Ne4 17.Bc3 Rfd8 18.Nd4 Nxd4 19.Nxd4 Nxc3 20.Qxc3 Bxe4 21.f3 Bg6
22.b4 Bxd4 23.Qxd4 Bf5 24.h3 h5 25.Qxd5 Qxd5 26.Rxd5 Rxd5 27.Rc7 Rd7 28.Rc5`), resigned
after `28...bxc5` at a completely decisive material deficit (no rooks left at all, Black
still had two). Roughly even, actively-played middlegame right up to move 24 — the whole
game turned on two errors, both the exact same shape, both self-caught only after the fact
by checking `fruit_analyze.sh`'s normalized score.

**Error one, move 25 (`Qxd5`).** Checked that `Rd1` independently defended the capture
square before playing it — correct application of the `Qb3`-style "mutual attack needs a
real third defender" rule from earlier this session. What wasn't checked: Black had *two*
pieces on the contested file (`Qd7` and `Rd8`), not one, so after the queens traded the
second black rook recaptured mine with nothing left to answer it — down a full rook for a
pawn instead of the even (or better) trade the single-defender check implied. Confirmed by
`fruit_analyze.sh`: `-443cp → -931cp` two moves later is actually the *second* error (below);
this one alone was `-77cp` at ply49-51 net, real but survivable — the game was still
fighting a genuinely bad but not lost position after it.

**Error two, move 28 (`Rc5`), is where the game was actually decided, and it is the exact
"wide before deep" failure named earlier this same session — caught live, in the act, not
in hindsight.** `check_move.nu`'s own "MY PIECES AT RISK" section printed
`MOVER_FAVORED (count alone said safe, flagged anyway): Rook@c5 1v1 -- verify with
calc_line.nu` — the tool correctly flagged the exact danger, in the exact section designed
to be checked first. Attention went instead to the *other* fact in the same output
(`Bf5` hanging, a positive-looking discovery) and the explicit instruction to verify the
rook's own flagged warning was skipped. `b6` — the same pawn already identified defending
`c5` at move 24 in this identical game, already written down as a reason to avoid that
exact square — captured the rook for free four moves later. `fruit_analyze.sh` confirms
this was the single largest swing of the whole game: `-443cp → -931cp`.

Both errors share one root cause: checking "does a favorable-looking fact hold up" instead
of checking "does *every* attacker/defender on this square hold up," and in the second
case, literally reading past a warning already labeled "check this first" because a more
interesting fact was sitting next to it. This is not a new finding — it is the same pattern
`chessdb_wide_before_deep` (memory, 2026-09-02) already named, recurring in the very next
game, which is itself informative: naming a pattern and updating a skill file doesn't erase
the underlying tendency on its own — it has to actually change what gets looked at in the
moment, and this game shows that hadn't fully taken yet.

Verified: `fruit_analyze.sh` run over the complete game and normalized by hand before
identifying which of the two candidate errors was actually decisive (the second, not the
first, despite the first being the more "interesting" miscalculation to explain). No code
changes — live play and honest documentation only.

---

## 2026-09-02 (continued): Fifteenth Fruit game — a queen trap from grabbing material without checking the retreat, plus a misread flag label

Played White, Scandinavian-into-classical-center structure (`1.e4 d5 2.exd5 Nf6 3.d4 Nxd5
4.Nf3 e6 5.Bd3 Nc6 6.O-O Be7 7.Nc3 Nb4 8.Nxd5 exd5 9.a3 Nxd3 10.Qxd3 O-O 11.Bf4 Rfe8 12.Re1
Be6 13.Ne5 Bd6 14.Rad1 f6 15.c4 c6 16.Qb3 Rb8 17.cxd5 Bxd5 18.Qa4 Qc7 19.Qxa7 Ra8 20.Qxa8
Rxa8 21.Rdc1 fxe5 22.dxe5 Qf7 23.exd6 Qxf4 24.d7 Qg4 25.f3 Bxf3 26.Kf2 Qg2+ 27.Ke3 Rd8
28.Rcd1 Bxd1 29.Rxd1 Qh3+ 30.Kf2 Rf8+ 31.Kg1 Qg4+ 32.Kh1 Qxd1+ 33.Kg2 Qf3+ 34.Kg1`),
resigned at `34...` with White down to a bare king and 4 pawns against queen + rook + 4
pawns — no compensation, no realistic swindle, confirmed by exhaustively enumerating White's
one legal move (`Kg1`) and the total absence of any White piece besides pawns.

**The decisive error, move 19 (`Qxa7`).** Checked the capture itself correctly —
`b7` was genuinely undefended (verified directly via `attackers_map.nu`, not the fork
label), and the queen's landing square had zero attackers at that exact moment. What
wasn't checked: whether the queen would have anywhere safe to go *if* Black attacked it
next move. Black played `19...Ra8`, and a full enumeration of all 8 legal queen moves from
`a7` (via `chessdb legal-moves`, not guesswork) showed every single one lost material —
`a4`/`a5`/`a6`/`b6` all hung outright, `Qc5` looked clean by count alone but was actually a
queen sitting 1v1 against a bishop with only a pawn recapture (a second near-miss, caught by
hand-computing the real values instead of trusting the "MOVER_FAVORED" label — see below),
and `Qxa8`/`Qxb7`/`Qb8` were all bad trades or worse. `Qxa8 Rxa8` (queen for rook, the
least-bad option found) was forced, netting −400 material on top of the pawn just won,
setting the material deficit that never recovered. **The lesson is distinct from "wide
before deep": it's not about missing a threat on the square just captured, it's about not
checking one ply further — does grabbing this piece leave me with a safe square to retreat
to if it's attacked back — before playing the grab.** See new memory
`chessdb_verify_retreat_before_grabbing`.

**A second near-miss in the same recovery sequence: the "MOVER_FAVORED" label is not
reliable when the flagged piece is far more valuable than its attacker.** Checking `Qc5` as
a retreat candidate, `check_move.nu` reported `MOVER_FAVORED (count alone said safe,
flagged anyway): Queen@c5 1v1` — read in isolation, "favored" sounds safe. Verified via
`attackers_map.nu` instead of trusting the label: the queen (900) sat attacked by a bishop
(330) with only a pawn (100) able to recapture — `Bxc5 dxc5` is −570 for White, not
favorable by any real accounting. The count-vs-count ratio the label describes doesn't
carry piece-value context; a queen and a rook can both be "1v1" and mean opposite things.
Already covered in principle by `feedback_dont_surface_untested_scores`
(don't trust a label, verify the value), but this is the sharpest concrete instance yet —
worth keeping as the canonical example.

**A third, smaller, self-inflicted loss at move 28 (`Rcd1`) — misreading which piece a flag
named.** `check_move.nu` flagged `MOVER_FAVORED (count alone said safe, flagged anyway):
Rook@d1 1v1` after playing `Rcd1`. Read this as referring to the standing `d7` pawn
situation (a fact that actually had been resolved earlier in the same output) rather than
parsing that the named piece — `Rook@d1` — **was the rook just moved, under direct attack
from `Bf3` along the `f3-e2-d1` diagonal.** Never checked it with `calc_line.nu` as the
flag's own text instructs. Black played `28...Bxd1`, winning the exchange (rook for
bishop, ≈ −170) for free — a loss that simple correct reading of the flag's own subject
would have caught. The flag format names the piece at risk explicitly (`Piece@Square`); the
fix is to read that name literally as "this is the piece to verify," not to assume context
from whatever else was checked earlier in the same response.

**What worked.** Two separate mate threats were caught and correctly resolved this game,
both via exhaustive candidate enumeration rather than pattern-matching: (1) after `24...Qg4`,
`d7d7` promotion and the `Rc1`/`Re2` development candidates were both individually checked
and both showed a live `Qxg2#` (the `g2` pawn absolutely pinned to the king by `Qg4`,
`Bd5`/later `Bf3` denying the king recapture) — `f3` (blocking the bishop's diagonal
support of `g2`) was identified and played as the only move that actually defused it,
confirmed by the tool's mate warning disappearing only for that candidate. (2) The forced
king walk after `Qxg2+` (`Ke3`, `Kf2`, `Kg1`, `Kh1`, `Kg2` in sequence, four separate checks)
was navigated by exhaustively checking every legal king move at each step rather than
picking the first-seeming safe one — `Ke1` and `Kf1` were both confirmed to lose to a
follow-up mate and correctly avoided in favor of the one square that didn't.

**A methodology note carried over from earlier games, still recurring: hand-typing a FEN
between tool calls produced a wrong position at least once this game** (an `attackers_map.nu
e5` check right after the `exd6` line used a guessed FEN with stale piece placement) — caught
by comparing against a properly re-derived FEN before trusting the result, consistent with
the standing practice, but worth noting it keeps happening and the fix (always regenerate
via move-history replay, never hand-copy) needs to actually become automatic rather than a
catch-after-the-fact habit.

**On `fruit_analyze.sh`'s postmortem numbers for this game: deliberately not used as the
primary diagnostic.** The raw cp trace shows an implausibly large swing at move 14 (`Rad1`,
`+10cp → -172cp` in the normalized trace) that contradicts the direct, hand-verified safety
check done live at the time (no hanging pieces, no tactics, confirmed via `calc_line.nu` and
`attackers_map.nu` against real board facts). Each ply in `fruit_analyze.sh` is analyzed by
a fresh, independent 600ms search with no continuity from the previous ply, which is exactly
the kind of noisy, unverified score `feedback_dont_surface_untested_scores` warns against
surfacing — so this postmortem's diagnosis above is built entirely from hand-verified
material facts (legal-move enumeration, direct attacker/defender counts, piece values) from
live play, the same standard applied during the game itself, not from the score trace.

Verified: full game replayed via `check_move.nu`/`calc_line.nu`/`attackers_map.nu` live
throughout; the queen-trap sequence re-verified after the fact via `chessdb legal-moves`
enumerating all 8 candidate squares; the two misread-flag incidents re-verified via
`attackers_map.nu` against freshly-derived FENs. No Rust or Nu-tool changes this game — live
play and honest documentation only, plus two new memory entries capturing the two genuinely
new lessons (retreat-checking before grabbing material, and reading flag piece-names
literally).

---

## 2026-09-03: Sixteenth Fruit game — "over-defended" isn't safe when the attacker is cheaper than the target, twice in one game

Played Black for the first time this session (all fifteen prior games were White). Réti/QGD
structure: `1.Nf3 d5 2.Nc3 Nf6 3.e3 Bf5 4.Bb5+ c6 5.Be2 e6 6.Nh4 Bg6 7.O-O Bd6 8.f4 Nbd7
9.Nxg6 hxg6 10.d4 O-O 11.Bd2 Qc7 12.Bd3 Rae8 13.Qf3 e5 14.fxe5 Bxe5 15.dxe5 Nxe5 16.Qg3 Nh5
17.Qh4 Nxd3 18.cxd3 Qb6 19.Na4 Qc7 20.Bb4 a6 21.Rf3 b5 22.Bxf8 Rxf8 23.Nc5 Qb6 24.Qe7 Nf6
25.Rxf6 gxf6 26.Qxf8+ Kxf8 27.Nd7+ Kg8 28.Nxb6 Kg7 29.Rc1 b4 30.d4 a5 31.Rxc6`, resigned after
`31.Rxc6` left White up a clean rook and knight (R+N+6P vs bare K+6P) with zero compensation
— confirmed by `material.nu`, not estimated.

**The decisive, twice-repeated error: reading "more defenders than attackers" as safe
without checking whether the single attacker is cheaper than the piece it's attacking.**
Moves 1–13 were clean, level, ordinary development — no tactics, no errors, matching the
pattern the user asked about (games consistently stay even through the opening). The first
real damage came at move 14. `14...Bxe5` recaptured a pawn, and `check_move.nu` flagged the
resulting bishop `MOVER_FAVORED (count alone said safe, flagged anyway): Bishop@e5 1v3` —
read as safe because three black pieces "defended" e5 against one white attacker. The one
attacker was White's `d4` pawn. A pawn (100) capturing a bishop (330) nets the attacker +230
*the instant it captures* — the three defenders only ever get to recapture the *pawn's*
value back, they cannot undo the fact that a cheap piece just ate an expensive one. `15.dxe5`
did exactly that: bishop lost for a pawn, no recapture chain fixes it. The identical shape
recurred at move 22: `Rf8` sat `MOVER_FAVORED ... 1v2` (defended by rook + king against one
bishop), read as safe by the same flawed count-only logic. `22.Bxf8` won the exchange (rook
for bishop, ~+170 for White) outright, and `Kxf8`'s recapture only ever recovers the
*bishop's* value, not the rook's. Two losses from the same unrecognized gap, in one game,
without the pattern being noticed as the same mistake the second time it happened.

**Why this is distinct from the existing "don't trust the label" memories.** Game 15's
Qc5/Rd1 incidents (`feedback_dont_surface_untested_scores`) were about misreading *which*
piece a flag named, or trusting a `consequence`/`see_cp` verdict outright. Here the count was
read correctly and the right piece was identified both times — the error is that "defenders
≥ attackers" was treated as sufficient by itself, when it structurally cannot be: defender
count only determines what happens if an exchange continues *past* the first capture: it
says nothing about whether that first capture already favors the attacker. Whenever a single
attacker is worth less than the piece it threatens (a pawn vs. a bishop; a minor piece vs. a
rook — the textbook "winning the exchange" shape), the position is not safe regardless of
how many pieces queue up behind it. New memory: `chessdb_defender_count_vs_attacker_value`.

**What worked, extensively, in the second half of the game — the same value-checking
discipline, applied correctly, over and over.** Once behind, calculation was noticeably more
careful: `24...Nf6` was flagged `OUTNUMBERED 2v1` (queen + rook vs. one pawn defender) and
verified rather than either blindly trusted or blindly avoided — hand-tracing the actual
capture chain showed the pawn recapture would win back *whichever* higher-value piece
initiated (queen or rook), so the "outnumbered" square was actually excellent for Black, the
mirror image of the move-14/22 mistake applied correctly this time. Real traps were caught
and avoided before being played: `Qxb2` and `Qb4` both looked like they won a pawn or
centralized safely but were refuted by `Na4` covering the landing square or `Qh4` raking the
rank (`attackers_map.nu`, checked before playing, not after); `24...Re8` looked like it
attacked White's queen but actually just hung the rook to the adjacent queen with check
(adjacency doesn't grant the mover priority — it was White's move next); `Ng4`/`Nf4` were
each independently checked and each independently found to hang to direct rank/file
attackers and correctly discarded, twice, in different positions.

**A three-move sequence around moves 25–27 that looks like a Black blunder in the PGN but
wasn't one.** `25.Rxf6` and `26.Qxf8+` were both *White* giving up material by the same
exchange-value logic documented above (rook for knight, then queen for rook) — real engine
inaccuracies at Fruit's 1000ms search depth, not points scored by Black's play, and reading
them as anything else would repeat exactly the "don't trust an unverified score" mistake in
reverse (crediting a result without checking whether it was earned). Both recaptures
(`25...gxf6`, `26...Kxf8`) were forced/correct regardless — declining either was strictly
worse (verified via `chessdb legal-moves`, which showed only two legal replies to the
`Qxf8+` check). The `27.Nd7+` fork that followed (hitting the king and, simultaneously, the
undefended queen on `b6`) was a genuine tactic and not a preventable error: both legal king
moves left the queen equally lost, confirmed directly rather than assumed, and accepting the
queen at move 26 was still correct despite the follow-up because declining it
(`26...Kh7`) was strictly worse by a wide margin. Sometimes the best available sequence still
loses material to a shot several moves down the forcing line, and calculating that honestly
(rather than assuming any loss must trace to an earlier mistake) is itself part of the
discipline.

**Resignation.** After `31.Rxc6`, material was White: 6P+1N+1R, Black: 6P only — an exact
rook-and-knight deficit with pawns level and zero pieces left on the board for Black. No
fortress or pawn-race consideration offsets a whole rook and knight against a lone king;
presented via `AskUserQuestion` per the established protocol (Game 15) rather than deciding
unilaterally, and the user selected resignation.

**On whether this session's games justify splitting `position-eval` into
opening/middlegame/endgame-specific skills (explicit user question this session).** Declined
— see the reasoning recorded in this session's response, not repeated here in full: openings
across all games in this project (including this one) have been consistently clean with zero
recorded errors, so there is no evidence of an opening-specific gap to fill. The "stays even
until the late middlegame" pattern the user observed is better explained by *when* real
tactical exchanges start happening (naturally not during simple development) than by any
missing phase-specific strategic knowledge — every recorded loss, across all six-plus
documented games, has been a tactical-verification failure (misread label, missed retreat
check, and now this game's defender-count-vs-attacker-value gap), not a positional or
strategic one. The fix each time has been sharpening the *existing* verification discipline,
not adding phase-specific content — this game's new memory continues that pattern rather
than breaking from it.

Verified: full game replayed live via `check_move.nu`/`attackers_map.nu`/`calc_line.nu`;
both decisive exchanges (moves 14 and 22) re-derived from fresh FENs and confirmed via
`attackers_map.nu`'s direct attacker/defender lists, not assumed from the flag text; the
`Nd7+` fork's inevitability confirmed via `chessdb legal-moves` showing only two legal
replies to the prior check, both losing the queen equally. No Rust or Nu-tool changes — live
play, honest documentation, and one new memory entry.

---

## 2026-09-03: `chessdb square-swap-list` (a real x-ray-aware sensor report), the nuon-everything migration, and a real bug found doing it

Two related pieces of work from the same session as game 16, both prompted by direct user
requests rather than in-game incidents.

**1. `chessdb square-swap-list` — the raw exchange picture game 16 needed and didn't have.**
Requested as a compact ply-indexed notation for "what attacks/defends this square, including
x-ray reveals" (user's own sketch: `0ply q 1ply NbPQ 2ply B` — a queen threatened by a
knight, pawn, and queen, defended by a bishop, with a second bishop revealed as an x-ray).
Implemented as a new Rust primitive rather than a Nu script, per this crate's own
architectural principle (`chessdb defers to shakmaty` — computing x-ray reveals means
removing pieces from occupancy and re-querying `Board::attacks_to`, exactly the class of
computation that belongs on the Rust side, not hand-rolled FEN surgery in Nu):

- `core::square_swap_list` recursively removes each ply's attackers' origin squares from
  the board's occupancy bitboard and re-queries `Board::attacks_to` for both colors until no
  new attacker appears — each new ply is exactly the set of pieces whose line to the square
  was blocked by a piece that has now been "used" in an earlier ply. Notation case is
  mover-relative (uppercase = the position's side-to-move's own pieces, lowercase = the
  opponent's — per the existing `chessdb_mover_not_color` convention, real color is kept
  alongside on every entry, never replaced by the mover-relative label), sorted ascending
  by standard piece value within each ply.
- New Rust unit tests (`square_swap_list_tests`) — the x-ray one in particular was
  cross-verified against a *different*, older, already-trusted command
  (`chessdb square-attackers`) on the same FEN with the blocking rook manually deleted from
  the FEN string, rather than trusting the new function's own output as its own proof, after
  the user directly asked whether the tests were "handing the answer rather than testing for
  it." (Verified: `square-attackers` on the full-occupancy FEN returns exactly `[e3]`
  attacking the target; on the same FEN with `e3`'s rook removed, exactly `[e1]` — matching
  the new function's `1ply`/`2ply` split precisely, independent of the new code.)
- `nu_plugin_chessdb/scripts/play/square_swap_list.nu` wraps it, built nuon-native from the
  start (see below) rather than needing its own migration.

**2. The nuon-everything migration.** Explicit user direction: "I would appreciate being
spoken to in nuon... I did mean specifically the tools both input and output." Every script
in `scripts/play/` (`check_move.nu`, `check_move_2ply.nu`, `calc_line.nu`,
`attackers_map.nu`, `control_map.nu`, `control_overlap.nu`, `forcing_moves.nu`,
`material.nu`, `board_overlay.nu`) converted: move-history input is now a nuon list literal
parsed with `from nuon` (never a hand-joined space string, and never a raw FEN positional
argument, which three of these scripts — `attackers_map.nu`/`control_map.nu`/
`control_overlap.nu` — had required until now, the exact hand-typed-FEN risk this whole
tool set otherwise avoids); output is one nuon record per call (`| to nuon --indent 2`, no
`print`, no ascii art. `board_overlay.nu`'s bracket-legend grid — built 2026-09-02
specifically to avoid hand-parsing a FEN, and protected by its own standing memory — was the
one real design fork this required: an ascii grid can't itself be a nuon value, only wrap
one. Resolved by explicit user choice ("structured record only") — the grid is gone, the
underlying goal isn't: `board_overlay.nu` is now two pure functions, `fen-to-board` (occupied
squares as `square -> {color, role}`, real board color) and `history-to-fen` (the shared
move-replay loop every script used to hand-duplicate). Full rationale and the "don't dump the
whole board on every call" corollary (a narrowly-scoped script like `control_map.nu`
shouldn't return all 32 pieces just because they're available — noise, not signal) recorded
in `chessdb_nuon_everything` (new memory, supersedes `chessdb_board_overlay_convention`).

Nu detail worth keeping: a script invoked directly from an OS shell (`nu script.nu <args>`)
cannot bind a `list<string>`-typed positional parameter from argv — Nu's script-CLI parser
rejects `nu script.nu '[a b c]'` even though the identical text parses cleanly through
`from nuon`. Every script's move-history parameter therefore stays `moves: string` at the
signature boundary and is parsed explicitly (`$moves | from nuon`) in the body — not a design
choice, a confirmed Nu limitation.

**3. A real, previously-hidden bug found doing the migration, not incidental to it.**
Cross-verifying `forcing_moves.nu`'s new nuon output against a known Fool's Mate position
(`1.f4 e5 2.g4 Qh4#`, replayed via `history-to-fen`, never hand-typed) showed
`checkmate_available` coming back empty when it should have named `Qh4#`. Root cause:
`core::mobility_summary` built `mobility_san` with `San::from_move`, which deliberately omits
the `+`/`#` check/checkmate suffix; `SanPlus::from_move` (shakmaty) computes it correctly by
playing a clone of the position and checking the result. This means `forcing_moves.nu`'s
entire CHECKS list and CHECKMATE-AVAILABLE detection — both implemented as string-matching
that suffix, and explicitly named in the position-eval skill's "calculate forcing lines"
section as the tool to reach for — had silently returned empty for the tool's entire
history, across every game this session and (as far as this entry can tell) before it too.
Capture (`x`) annotation was unaffected, only the check/mate suffix was missing. Fixed at the
source (one-line call swap, `pos.clone()` since `SanPlus::from_move` consumes its position
argument); a fully-qualified `shakmaty::san::SanPlus` path was needed at the call site since
`pgn_reader` already exports an unrelated type of the same name, already imported in this
file for PGN parsing. Regression-tested against both a mating position (the Fool's Mate
above) and a non-mating check (`1.e4 d6 2.Bb5+` — chosen over the more "obvious" `1.e4 e6
2.Bb5` specifically because that one is *not* actually check, the d7 pawn still blocks the
diagonal; caught by testing rather than assumed, a small real instance of the same
hand-verification discipline this file exists to document). While writing this test, a
first draft hand-typed a FEN with a fabricated en passant square that didn't correspond to
any real capturing pawn — caught before committing by re-deriving the FEN via
`history-to-fen` and diffing against the hand-typed guess, the exact recurring risk class
`chessdb_defers_to_shakmaty`/this file's own history keeps naming, this time inside test
code rather than live play.

**Lesson for future tooling changes, stated explicitly because it generalizes:** a
structural rewrite of a tool's I/O shape (nuon-ification, or any other refactor) is a real
opportunity to cross-check the tool's underlying facts against independently-known
positions — it should not be treated as a pure format port that assumes the old behavior was
already correct. The check/mate suffix bug would not have been found by a mechanical
find-and-replace migration; it was found specifically because verifying the *new* output
against a known chess fact was already the standing discipline for every change in this file.

Verified: `cargo check --all-targets` / `cargo clippy --all-targets` / `cargo test` clean
throughout (35 tests passing, up from 30: 5 new `square_swap_list_tests`, 1 new
`mobility_summary_tests`); release plugin rebuilt and re-registered; every converted script
smoke-tested against a known fact (`material.nu` against this exact game's recorded material
at move 9; `attackers_map.nu`/`square_swap_list.nu`/`control_map.nu` against the pre-existing
Rust unit tests' own known positions; `calc_line.nu`/`forcing_moves.nu` against the Fool's
Mate mate-in-1). New memory: `chessdb_nuon_everything` (supersedes
`chessdb_board_overlay_convention`, which is kept, marked superseded, for its historical
reasoning).

---

## 2026-09-03 (continued): `chessdb board-probe` — one comprehensive shakmaty-backed nuon report, and where highlighting actually belongs

Follow-up to the nuon migration above, same session. User asked to design a standardized
"highlight" vocabulary/marker scheme so ascii/bitboard/FEN output could consistently show
meanings like attacker/defender/hanging/pinned. Working through two rounds of that design
(a candidate 24+-role vocabulary drawn from every square-bearing fact this crate already
produces; a wire-shape question of grouped-by-role vs. grouped-by-square) surfaced a
sharper distinction the user named directly: **nuon doesn't need a highlight scheme at
all.** It can label squares/pieces with explicit role fields, grouped however is clearest
for a given report, because it's already fully expressive structured data — no encoding
scheme is needed to attach meaning to it. A highlight *scheme* (brackets, single-character
markers, a fixed grammar) is only a real problem for formats that genuinely can't carry an
extra labeled field: ascii (a fixed character grid), a bitboard (fixed 64 bits), a FEN
(fixed grammar) — those have to compress meaning *into* the representation itself instead
of alongside it.

**Resolution:** chessdb's job stops at producing the full, honest nuon report. Rendering,
filtering, or any highlight/marker scheme for a constrained format is explicitly a
separate, downstream client's responsibility — not something to build into this crate.
Given that, the user's actual next ask was direct: "use shakmaty to probe the board of all
information and then compile that into a single nuon."

**`core::board_probe`** composes the already-parsed `Chess`/`Board` once (not by
re-calling `square_control`/`square_attackers`/`fen_info`/`checker_summary`/
`mobility_summary`, each of which would reparse the FEN) into one record:
- All 64 squares (`Square::ALL`), each with `occupant` (`Option<PieceOnSquare>`),
  `is_light`, `controls` (`Board::attacks_from`, empty on an empty square), and
  `attacked_by_white`/`attacked_by_black` (`Board::attacks_to`, both colors, occupied or
  not — the same primitive `square_attackers` already uses, just for all 64 squares in one
  pass instead of one square per call).
- Position-level state: side to move, castling rights, en passant square, halfmove/
  fullmove counters, check/checkmate/stalemate/insufficient-material, `checkers` (square
  list), full legal move list in both SAN and UCI (`SanPlus::from_move`, the same fix from
  the entry above, so this was correct from the moment it was written rather than
  inheriting the missing-suffix bug).
- Raw material counts per color (`Board::by_piece(Piece{color,role}).count()`) — computed
  directly, deliberately *not* routed through `hugm-eval`/`SensorReport`, so a comprehensive
  probe never has to pull in any tuned-formula field alongside the facts that are actually
  wanted, matching `chessdb_material_nu`'s standing "raw counts, never a computed sum" rule.

Deliberately excludes `square_swap_list`'s recursive x-ray plies — computing that for all
64 squares by default would be the expensive operation repeated 64 times when almost all
of them aren't contested; stays a targeted, one-square-at-a-time call.

New Rust unit tests (`board_probe_tests`) cross-verify against facts already established by
*older* tests rather than trusting the new function's own output — `c3`'s attackers
against `square_attackers_tests::square_attacked_by_exactly_one_side`, `c1`'s bishop
control against `square_control_tests::sliding_piece_control_stops_at_the_first_blocker`,
and a Fool's-Mate-position checkmate check whose FEN was derived through the actual
move-application chain (`history-to-fen`), not hand-typed, continuing the same discipline
from the `SanPlus` bug entry above.

`scripts/play/board_probe.nu` wraps it, nuon-native from the start (move-history nuon
string in via `from nuon`, one `chessdb board-probe | to nuon --indent 2` record out).
Live-smoke-tested against a real Ruy Lopez position (`e4 e5 Nf3 Nc6 Bb5`): 64 squares
present, `b5`'s bishop occupant and 7-square control list correct, material counts correct.

Verified: `cargo check --all-targets` / `cargo clippy --all-targets` (one
`bool_assert_comparison` lint fixed) / `cargo test` clean (70 lib tests, up from 66 — 4 new
`board_probe_tests`); release plugin rebuilt and re-registered. New memory:
`chessdb_board_probe` (the design-conversation resolution — nuon needs no highlight
scheme, chessdb never renders/filters, that's a separate client's job — plus when to reach
for this vs. the narrower single-square scripts).

---

## 2026-09-03 (continued): the shakmaty-1:1 architecture — four rust-composed commands built earlier the same day, removed the same day, replaced by leaf commands + nu composition

Explicit user direction, arriving directly on the heels of the `board-probe` work above:
"I want the chess db plugin to be a 1-1 mapping of shakmaty functions, basically
translating their output to nuon, and accepting their output as nuon ... instead of a skill
asking the plugin for the right questions, a tree of reports [is] compiled as a graph ...
in nushell." Confirmed explicitly a turn later: "let native shakmaty rule and we are just
building layers of visualization, nushell nuon first and the ai agent will live on that
level, but a client can take the information and build a board with highlights and filters
based on report specifics."

**What this meant concretely, worked out through a short design exchange:** `Board::attacks_to`
already takes `occupied` as an explicit shakmaty parameter, not something derived
internally — so exposing it 1:1 means occupancy becomes a caller-supplied bitboard, which
is exactly what lets nushell perform `square_swap_list`'s recursive x-ray removal itself
(call the leaf with the full occupancy, subtract squares with an ordinary `where` filter,
call again) with no rust loop at all. Reading further into shakmaty found `attacks::attacks
(square, piece, occupied)` — one dispatcher covering every role, plus `ray`/`between`/
`aligned` — pure geometry, no board or position, lower-level and more general than
`Board::attacks_from`/`attacks_to` (themselves just this dispatcher combined with the
board's own piece-placement bitmaps). That settled the leaf boundary: expose
`attacks::attacks`/`ray`/`between`/`aligned` and `Board::occupied`/`by_color`/`by_role`/
`by_piece`/`piece_at`/`Square::is_light` as individually thin commands
(`geom-attacks`/`geom-ray`/`geom-between`/`geom-aligned`/`board-pieces`/`board-piece-at`/
`square-is-light`), and compose everything above that — `attacks_to`, `attacks_from`, the
swap-list recursion, the whole-board probe — in nushell instead of rust.
`fen-info`/`checker-summary`/`legal-moves`/`apply-uci`/`is-legal`/`canonicalize-fen`/
`attack-summary` were explicitly scoped OUT of this decomposition (flagged, not silently
decided): each is already close to 1:1 or is a genuine whole-board primitive, not the
per-square geometric composition this principle targets.

**The fork this forced, and the resolution: deprecate-and-remove, not deprecate-and-keep.**
The same session had built four rust-composed commands earlier the same day —
`square-control`, `square-attackers`, `square-swap-list`, `board-probe` — each of which
loops/composes shakmaty primitives internally in Rust. Explicit user choice: remove all
four, not keep them as convenience shortcuts alongside the new leaf layer. One candidate
exception was raised (keep `square-swap-list` specifically, since its recursive-removal
algorithm seemed like it might need to live where the `Bitboard` type lives) and explicitly
declined by the user: "bitboard is great for shakmaty, and that can be used for all
shakmaty things, keep the bitboard at all levels and let the client translate into FEN,
PGN, ascii-board, etc." — confirming the recursion itself belongs in nushell too, once
occupancy is just a plain square list.

**Verification, not blind deletion — matching this crate's own `detect_skewers` A/B
precedent.** `nu_plugin_chessdb/scripts/play/shakmaty_compose.nu` was built first
(`attacks-to`, `attacks-from`, `swap-list`, `board-probe`, each composed from the new leaf
commands), then diffed byte-for-byte against the rust command it was meant to replace,
*before* that rust command was touched:
- `attacks-to` vs. `square-attackers`: matched on the start position's `c3` (0 defenders
  each, cross-checked earlier this session against a pre-existing Rust test) and on an
  empty-target-square case.
- `attacks-from` vs. `square-control`: matched on the start position's `c1` bishop
  (`b2`/`d2`, the same fact `sliding_piece_control_stops_at_the_first_blocker` established).
- `swap-list` vs. `square-swap-list`: matched exactly, including the ply-notation string,
  on the dedicated x-ray test position (doubled rooks, `0ply n 1ply R 2ply R`) and on two
  real Game 16 positions (one with an occupied target square, one with an empty one).
- `board-probe` vs. `board-probe` (rust): matched on every field for a real Ruy Lopez
  position, with one real (non-bug) discrepancy found and fixed along the way — the nu
  composition and the rust command's internal bitboard iteration produced the same
  attacker/defender *sets* in different *orders*. Not a correctness bug (order was never
  part of the contract), but real enough to be worth catching properly rather than assuming
  — confirmed by comparing every one of the 64 squares' records after sorting both sides,
  then adding `sort` to the shipped nu functions so the difference doesn't recur for a
  future comparison.
- Performance sanity-checked, not just correctness: `swap-list` on a real midgame position
  ran in ~15ms despite being many small plugin round trips instead of one rust loop;
  `board-probe` (composed as O(pieces) round trips — compute what each piece attacks once,
  then invert into "who attacks this square" per square in nu, rather than O(64 × pieces) —
  re-querying attacks-to fresh at every one of the 64 squares) ran in ~52ms. Both fine for
  interactive use.

Only after every comparison matched were `square_control_cmd.rs`/`square_attackers_cmd.rs`/
`square_swap_list_cmd.rs`/`board_probe_cmd.rs` and their `core.rs` structs/functions/tests
deleted, and `control_map.nu`/`attackers_map.nu`/`square_swap_list.nu`/`board_probe.nu`
(the only four scripts that had called the removed commands directly) rewired onto
`shakmaty_compose.nu`. Every other script (`check_move.nu`, `calc_line.nu`,
`forcing_moves.nu`, `material.nu`, `control_overlap.nu`) was unaffected — none of them ever
called the four removed commands directly — and a full smoke test across all nine
`scripts/play/*.nu` tools confirmed nothing else broke.

Verified: `cargo check --all-targets`/`cargo clippy --all-targets` clean throughout; 8 new
leaf-layer unit tests, cross-verified against either an older pre-existing test's known
fact (the knight-in-the-corner control fact, the `c1` bishop fact) or shakmaty's own doc
examples (`between(B1,B7)` → b2..b6, `aligned(A1,B2,C3)` → true), never against this new
code's own output in isolation; `cargo test` clean at 60 lib tests (down from 78 immediately
before removal — 18 tests deleted alongside their four commands, matching exactly); release
plugin rebuilt and re-registered; full byte-for-byte A/B verification (above) before any
deletion; end-to-end smoke test across every `scripts/play/*.nu` script after the rewire.
New memory: `chessdb_shakmaty_1to1` (the standing architecture); `chessdb_square_control`
and `chessdb_board_probe` updated to point at it rather than describing stale rust commands.

---

## 2026-09-03 (continued): `full_report.nu` — one comprehensive report, and a real score-leak bug caught before it shipped

Immediate follow-up, same session: "let's work on completeness of the report, the skill
then becomes how to read the report, what is most important down to least important."
Explicit priority order for this work, given directly: shakmaty-1:1 (done, above) first,
compositing the report second, filtering third — "we can filter out scores if they have a
similar identifier in their name" rather than a hand-audited per-field allowlist.

**`shakmaty_compose.nu`'s `strip-scores`** — a generic, recursive record/list walker that
drops any key matching `/score|_cp$|centipawn|consequence/i`, everything else passes
through unchanged. Deliberately blunt, not exhaustive, per the stated priority (filtering
is the lowest of the three). One known, accepted imprecision: `development_score_diff` (a
legitimate structured fact this skill has always read, not a computed valuation) matches
the `score` pattern and gets stripped too — not fixed, since the stated priority is speed
over per-field precision here.

**`shakmaty_compose.nu`'s `full-report`** merges `board-probe`'s geometric/structural
report with `chessdb hugm-eval --verbose true`'s `sensor_report.tactical`/`.positional`
(and `mate_in_1_exists`/`king_tropism_us`/`initiative_us`/`doubled_rooks_us`), filtered
through `strip-scores`. `full_report.nu` wraps it — one call now returns everything a
position evaluation needs: all 64 squares' geometry plus every tactical/positional
detector fact, nothing computed-valuation left in it.

**A real bug found building this, not incidental to it.** Checking what `strip-scores`
would need to filter meant re-examining `check_move.nu`/`check_move_2ply.nu`/`calc_line.nu`'s
own field access — and found that this same session's earlier nuon migration had silently
reintroduced exactly the leak `check_move.nu`'s own header comment promises doesn't happen.
The *old*, pre-nuon `check_move.nu` never leaked `consequence`/`see_cp` because it
selectively `print`ed only specific fields (`attacker_count`, `defender_count`, ...) — the
filtering was implicit in what got printed. The nuon rewrite switched to *returning whole
filtered lists* (`$t.outnumbered | where {...}`) instead of selectively printing fields,
which preserved every field on each surviving record, `consequence`/`see_cp` included.
Confirmed directly: the same Game 16 position that had an `outnumbered` entry showed
`consequence`/`see_cp` present in the raw `sensor_report.tactical.outnumbered` but no
longer showed them after applying `strip-scores` in `check_move.nu`. A second, distinct
instance of the same class: `calc_line.nu` returned `$s.material.balance` wholesale, which
carries a `centipawns` field `material.nu` has always deliberately avoided — `centipawn`
wasn't originally in `strip-scores`'s pattern (no literal `_cp` substring in the word
"centipawns"), so it had to be added once this was found. Fixed by applying `strip-scores`
to the tactical/sensor data pulled in `check_move.nu`, `check_move_2ply.nu`, and
`calc_line.nu`; verified no `consequence`/`see_cp`/`centipawn` string appears anywhere in
any of their output on the same real positions used to find the leak.

**`.claude/skills/position-eval/SKILL.md` rewritten around `full_report.nu`.** The skill's
substantive reasoning (the priority-ordered walk: tactics/safety first — can override
everything below it — then material, king safety, pawn structure, piece activity last) is
unchanged; what changed is the mechanical "how do you get this data" section, which used to
say `"<FEN>" | chessdb hugm-eval --verbose true` and now says
`nu scripts/play/full_report.nu '[<uci moves>]'` — one call, nuon list-of-moves input
matching every other tool, no raw FEN. `material.balance.white`/`.black` references
updated to `full_report.nu`'s flattened `material_white`/`material_black` fields. The
"wide before deep" section's citation of the (now-removed) `chessdb square-attackers`
command updated to note `full_report.nu`'s own `squares.<sq>.attacked_by_white/black` as an
equally valid source once the report is already loaded, alongside `attackers_map.nu`.

Verified: `strip-scores` tested against both a position with real `consequence`/`see_cp`
values present (confirmed via the raw, unfiltered `sensor_report` first, then confirmed
absent after filtering — not just "assumed the filter worked") and a clean position;
`full_report.nu` smoke-tested end to end with a `grep -i "score|_cp|centipawn|consequence"`
sweep across its full output on a real Game 16 position, zero matches; `check_move.nu`/
`check_move_2ply.nu`/`calc_line.nu` re-verified the same way after their fix. No Rust
changes this entry — all nu-side. New memory: `chessdb_full_report` (the comprehensive
report and the `strip-scores` filter), and a note added to the general "nuon migration"
lesson that switching from selective-print to whole-record-return is a real, easy-to-miss
way to un-filter something a print statement used to filter implicitly.
