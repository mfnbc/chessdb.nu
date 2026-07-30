NNUE Audit & Plan — chessdb.nu

## 2026-05-13 Decision (still current as of 2026-07-28): Re-scoped

Full NNUE training is deferred. The project imports Stockfish's built-in NNUE
via UCI rather than training a custom net. The bullet-based training pipeline
(`dataset_builder_cmd.rs`, bulletformat shards) was removed 2026-07-30 in a
YAGNI pass — it had been paused with no active work since before this audit.
`src/position_encoder.rs` (the feature-vector encoder that fed it) was
removed the same day: it had no callers left once `dataset_builder_cmd.rs`
was gone, so "kept for a future training pipeline" was really just
unnoticed dead code — this doc had claimed it was a deliberate placeholder
without that having actually been checked. PLAN.md has the full history if
either is ever revived.

**Current focus**: HUGM calibration — regressing HUGM component scores against
Stockfish centipawn scores to tune HUGM weights. `hugm_harness` (`src/bin/hugm_harness.rs`,
348 lines) reads `{fen, engine_score}` JSONL and has the regression scaffolding; it
is not yet wired into an automated tuning loop (see PLAN.md Phase B/D).

## Current inference command: `chessdb nnue-eval`

### Usage
```
"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" | chessdb nnue-eval
```
Returns: `{fen, nnue_score}` record with centipawn evaluation.

Supports lists of FENs for batch processing.

Implementation: `src/nnue_eval_cmd.rs` spawns Stockfish as a subprocess (UCI),
resolving the binary from `$STOCKFISH_BIN` (default: `/usr/sbin/stockfish`).

### Remaining open items
- Long-term: if direct `.nnue` file loading is needed (faster than UCI), implement a Rust NNUE parser. Not required now.

### Resolved since last audit
- The old BUG-6 (Stockfish path inconsistency between `nnue-eval` and `sf_batch_eval`)
  is gone: `src/bin/sf_batch_eval.rs` is now a 3-line stub
  (`eprintln!("sf_batch_eval removed: use the external labeling pipeline described in NNUE_AUDIT.md")`)
  with no hardcoded path left to be inconsistent. Labeling-corpus generation now goes
  through `src/bin/lichess_to_jsonl.rs` / `src/bin/pgn_to_jsonl.rs` → `hugm_harness`.

---

## Original Audit (archived; background/history)

Purpose
- Quick research & scoping (Phase 0) for adding NNUE training/inference support.
- Map what already exists in the repository that we can reuse, identify gaps, and propose next concrete tasks.

Background (short)
- NNUE (Efficiently Updatable Neural Network) is a lightweight, high-performance neural evaluator widely used in chess engines.
- Key idea: a sparse, piece-list-friendly input encoding and a small dense network (feature transformer + hidden layers) that can be cheaply updated as pieces move.

Current reusable pieces
- Position encoder (`src/position_encoder.rs`): 1024-element f32 vector (793 meaningful
  features: 768 piece-square one-hot + game-state + material balance + king position +
  tactical summary, zero-padded). Still present, still compiles, unused by any active
  training pipeline — ready if training is picked back up.
- HUGM eval (`src/eval/position.rs`): ~3400 lines of handcrafted heuristics with tunable weights (grew from ~2800 lines at last audit; see PLAN.md for feature status).
- NNUE eval (`src/nnue_eval_cmd.rs`): UCI-based Stockfish wrapper.

Policy: Stockfish evaluation handling (unchanged)
- Do NOT persist Stockfish numeric evaluations as canonical fields in the positions table.
- Stockfish is an external oracle for review and labeling.
- HUGM remains the primary human-interpretable heuristic layer.
