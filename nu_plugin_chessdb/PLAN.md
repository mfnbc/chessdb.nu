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

OPEN: none currently tracked.
