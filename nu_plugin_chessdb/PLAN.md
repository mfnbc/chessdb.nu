# PLAN: what this is, and what it found

This document exists so a new reader — or future us — can understand the current
architecture and its best-supported discoveries without reading `FINDINGS.md` start to
finish. `FINDINGS.md` is the archive: the full, chronological build log, every wrong turn
and every correction, in the order they happened. This document is the compacted,
current-state view distilled from that archive, organized by idea instead of by date, kept
sharp and focused rather than growing forever — new architecture-level discoveries belong
here; the detailed narrative of how they were found belongs in `FINDINGS.md`. If something
here goes stale as the code changes, trust the code and fix this file, not the other way
around.

## The one-paragraph version

This system is **not a chess engine**. It does not search, and it does not calculate the
value of an exchange the way a real engine (or a human doing careful analysis) does. What
it does instead is build one shared map of **control** — who can reach which square, for
both players, in a single move — and read **emergent facts** off that map: a piece with no
defenders, a defender that's secretly overloaded, a piece that only looks safe because you
didn't check who's really guarding it. The goal is narrower and more honest than "evaluate
this position": it's to help answer *did you calculate this exchange correctly before you
entered it* — the same judgment a coach makes watching a student's game, not the judgment
an engine makes searching twenty moves deep.

## Control and continuity: the one map everything is built on

Take any square on the board. Some number of White pieces can move there in one move; some
number of Black pieces can too. `ThreatGraph::control(square, color)` is just: *(pieces of
`color` that reach this square) − (pieces of the other color that reach this square)*.
That's it — no piece values, no move ordering, just counting.

The important, slightly counterintuitive fact is that this is **one number, not two**.
`control(sq, White)` and `control(sq, Black)` are always exact negatives of each other on
every square, on every position — proven directly (`control_is_one_shared_map_not_two_
independent_ones`, `threat_graph.rs`). There's no such thing as "White's control map" and a
separate "Black's control map" that happen to agree; there's one shared map of contested
squares, and asking about it from either side is just reading the same number with a sign
flip. That single fact — **continuity**, in the sense used throughout this project's
history — is the substrate everything else below is built from: `attackers()` (who,
specifically, contributes to a square's count), `zone_control()` (control summed over a
group of squares, e.g. the ring around a king), `checkers()` (which piece(s), specifically,
are giving check right now), and `is_in_check()` (just: is that list non-empty).

## The failure lattice: one question, asked at increasing depth

Ask "is this piece actually safe?" and there's a ladder of increasingly careful ways to
check, each one catching a mistake the previous one couldn't see:

1. **Is it attacked at all?** The shared precondition every rung below starts from.
2. **Is it defended at all?** `find_hanging` (zero raw defenders — nothing can recapture)
   and `find_outnumbered` (defenders exist, but there aren't enough of them) are two halves
   of the *same* raw-count question, split only because the certainty differs: zero
   defenders is a guaranteed loss; being outnumbered is a softer signal, since piece values
   could still make the trade fine — a pricing question this system deliberately never
   answers (see below).
3. **Is a defender actually free to help?** `find_overloaded` asks this from the
   defender's side ("am I already the *only* thing defending something else?");
   `find_false_defense` asks it from the attacked piece's side ("are *all* my defenders
   pinned off the one line that would let them legally recapture here?"). Different
   vantage point, different strength of constraint — overload is soft (legal but costly
   to abandon), a pin is hard (illegal to move off).
4. **Does that change the verdict the raw count gave?** `find_false_safety` is the rung
   above both: it fires exactly when the raw count alone said "safe" but discounting
   defenders that are pinned-off or overloaded elsewhere reverses that verdict. This is the
   mistake a player who "counted correctly" can still make — the count was right, a hidden
   commitment on the defending piece wasn't seen.

Every struct in this family carries the *raw* numbers alongside the conclusion (attacker
count, defender count, which specific piece is compromised and why), not just a yes/no —
because the point isn't to hand over a verdict, it's to make the verdict traceable back to
a named piece and a named reason. That traceability is the actual product here, more than
any individual concept.

## Pathfinding an exchange instead of calculating it

The hardest design problem this project ran into: a piece with zero raw defenders isn't
necessarily really "hanging." Sometimes the piece that would capture it walks into
something worse — a queen sac that looks free but leads to a mating attack is the classic
shape. A real engine answers this by searching the position after the capture. This system
deliberately doesn't, because that's explicitly the real engine's job to do later, not this
one's job to approximate badly now.

`ThreatGraph::collapse_criticality(square)` is the alternative: for every piece touching a
contested square (attacker, defender, or occupant), clear the *entire* local cluster off
the board, then place just *one* candidate back at a time and ask what the resulting board
looks like — does that candidate's own king end up in check, does some other piece
(anywhere on the board) become newly undefended, does either king's safety swing. No
capture order, no choice of "best" response, no search: a structural question ("if this
piece is the one left standing here, is that safe for it") answered by direct
substitution and re-reading the same `control` map, once per candidate.

Two earlier designs were rejected before this one:
- **Substituting the least-valuable attacker onto the square** (mimicking how a real
  static-exchange evaluator orders captures) — rejected because choosing *which* piece
  recaptures, in *what* order, is itself a move decision, exactly the kind of calculation
  this system exists to avoid making.
- **Removing cluster pieces one at a time from an otherwise-unchanged board** — rejected
  because the other pieces still on the board contaminate the reading; you can't cleanly
  ask "if this piece alone survives here" while everyone else who was also fighting over
  the square is still sitting there.

The version that stuck — clear everyone, place one back at a time — is what actually
answers the "did you calculate right" question cleanly. `HangingPiece.safe_to_capture` is
this reasoning wired directly into the ladder: a piece with zero raw defenders is only
reported as a real `hanging_piece` concept if at least one attacker could actually capture
it without its own king ending up in check.

## How primitives become features

None of the sections above stop at being interesting facts about a position — each one is a
layer the next layer is built on, and nothing above this line collapses to an opaque score
before it needs to:

1. `control(sq, color)` — one signed number per square. The only raw primitive.
2. `attackers()`, `zone_control()`, `checkers()`, `is_in_check()` — named relations read
   directly off that one number, still nothing but geometry.
3. The failure lattice (`find_hanging` … `find_false_safety`) and `collapse_criticality` —
   composed *detectors*, each returning a typed struct that names the piece, the square, and
   the raw counts behind its conclusion, not a bare yes/no.
4. `build_sensor_report` (`position.rs`) folds every detector's output for one position into
   `SensorReport` — the single typed representation of "what did this ply's board actually
   show," replacing the older, string-keyed `EvalGroups.terms` grab-bag as the one thing
   downstream code reads.
5. `extract_concepts` (`concepts.rs`) turns a `SensorReport` into `Vec<Concept>` — the same
   shape regardless of whether a `Concept` originated from a tactical detector, a positional
   extractor, or a material scalar, so everything downstream treats them uniformly.
6. `rank_issues_for_position`/`rank_issues_for_player` turn `Concept`s into `GatedIssue`s:
   severity × ELO-relevance × confidence, ranked — the actual coaching output a player or an
   LLM sees.
7. `chess-tactical-events` (Nu layer) persists the *structured* facts from step 3 — not
   step 6's ranked narrative — per ply, per square, per concept, so the same feature can be
   read back move-to-move: this is what makes it possible to graph where a game got
   volatile, not just say so once for the position in front of you.

Every arrow in that chain is a composition of something simpler, never a re-derivation from
scratch — the "emergence" in the one-paragraph version above is this whole stack, not just
`control` on its own.

## Findings

Current-state facts about this system's behavior, each pinned down by a permanent
regression test built from a real, independently-sourced named game (`tests/known_games.rs`
— never a hand-invented position). The story of *how* each fact was discovered — false
starts, hand-derivations that were wrong on the first try, bugs found once and then found
again — lives in `FINDINGS.md`, not here.

| Fact | Primitive | Regression test |
|---|---|---|
| `control` is one shared, signed map (`control(sq, White) == -control(sq, Black)` always), not two independently-computed maps that happen to agree | `ThreatGraph::control` | `control_is_one_shared_map_not_two_independent_ones` |
| A pawn can read as `outnumbered` rather than `hanging`: the piece that vacated a blocking square can still defend it via a second, unrelated line of attack | `find_outnumbered`, Fried Liver Attack | `fried_liver_e4_outnumbered_not_hanging_because_the_attacking_knight_also_defends_it` |
| A queen can look defended by a king's mere adjacency at one moment, then become genuinely hanging the instant a different piece steps off the file it was blocking — the check delivered by that same move is a bonus tempo, not what actually wins the queen | `find_hanging`, ICBM Gambit | `icbm_gambit_queen_only_hangs_once_the_check_vacates_the_open_file` |
| A piece forced to interpose against check can, several moves later, look like an ordinary defender by the raw count (1 attacker, 1 defender) while being unable to legally recapture at all — `false_defense` and `false_safety` both correctly fire on the same square for the same reason | `find_false_defense` + `find_false_safety`, Drunken Bishops Gambit mating line | `drunken_bishops_gambit_pinned_interposer_is_a_false_defender` |
| The ladder reports **nothing** on a piece genuinely trapped over the long term (no legal escape square, ever) but not under immediate material threat — a positional fact, not a tactical one, outside this system's scope | `find_hanging`/`find_outnumbered`, Noah's Ark Trap | `noahs_ark_trap_bishop_hangs_before_it_retreats` (pins the earlier, genuinely tactical moment instead) |
| The ladder also reports **nothing** on a slow positional squeeze (a queen-and-two-rooks battery built up over many moves), even though a human recognizes the pressure instantly | `find_outnumbered`/`find_overloaded`, Alekhine's Gun (Alekhine–Nimzowitsch, San Remo 1930) | `alekhines_gun_mutual_hang_after_the_queenside_pawn_break` (pins a smaller, real tactical moment one move later instead) |
| Kings are explicitly excluded from every one of the five ladder detectors — a checked king is never reported as an ordinary hanging or outnumbered piece | all five ladder detectors | `checked_king_is_not_reported_as_hanging_or_outnumbered`, `checked_king_is_excluded_from_overloaded_false_defense_and_false_safety_too` |
| `is_in_check` is defined directly in terms of `checkers()`: checking is just asking whether that list is non-empty, not a second computation that happens to agree with it | `ThreatGraph::checkers` | `is_in_check_matches_shakmatys_own_is_check` |
| `positions.fen` is normalized so White is always shown to move; feeding it directly into the analysis pipeline as if it were the real board gives systematically wrong square and color labels whenever Black is actually to move | `chess-tactical-events` (Nu layer) | replays `moves.uci` (stored in real terms) through `chessdb apply-uci` instead of reading the stored FEN directly |

## What this deliberately does not do (yet)

- **No search, no exchange pricing.** `see`/`see_chain` (static exchange evaluation) exist
  in the codebase but are deliberately not depended on by anything described above, and are
  known to have an unfixed bug in their multi-step math (see `FINDINGS.md`). The whole ladder
  and `collapse_criticality` are built specifically to answer safety questions *without*
  that kind of calculation.
- **No positional-entrapment or long-term-pressure sensors.** Two of the findings above are
  deliberate null results, not gaps quietly left open: a permanently trapped piece (Noah's
  Ark) and a slow file-domination squeeze (Alekhine's Gun) both need a different kind of
  primitive — control accumulated or tracked *over time or over a zone*, not read once from
  a static position — which hasn't been built yet. `zone_control` is the substrate such a
  sensor would build on; nothing consumes it for this purpose yet. A separate, older,
  simpler `CenterControl` feature already exists (`extract_center_control`,
  `position.rs`) but is a plain pawn/piece-count heuristic predating this graph-based work,
  not built on `zone_control` — a candidate for unification later, not yet unified.
- **No narrative generation.** Every struct here reports facts (which piece, which square,
  which count) and a short, mechanically-generated phrase — never a synthesized
  explanation of *why a position matters* to a specific player. That synthesis is treated
  as interpretation, not quantification, and is left to whoever (or whatever LLM) is
  actually coaching a specific player at read time — see `FINDINGS.md`'s "what can be
  described vs. what can be detected" entry for the fuller reasoning.

## External oracle: Stockfish gives a score, not features

`chessdb stockfish-eval` (`src/stockfish_eval_cmd.rs`) spawns Stockfish over UCI and returns
its final centipawn score for a FEN. That's all it does — nothing about this project's own
control graph, failure lattice, or `collapse_criticality` runs through Stockfish, and
nothing Stockfish computes runs through this project's code. It's a read-only external
oracle, used by hand to label `{fen, engine_score}` JSONL that `src/bin/hugm_harness.rs`
regresses HUGM's own component scores against, to help tune HUGM's hand-set weights.
Policy: a Stockfish score is never persisted as a canonical `positions` field — it's a
calibration input, not part of this system's own evaluation.

An earlier, different idea lived under the same "NNUE" label and is gone: training a
custom NNUE-shaped net directly from a generic piece-square feature encoding
(`position_encoder.rs`, `dataset_builder_cmd.rs`, both deleted 2026-07-30 — full history in
`FINDINGS.md`). That confusion is also why the command itself was renamed from `nnue-eval`
to `stockfish-eval`: calling Stockfish for a score has never meant this project has, or is
building, its own NNUE.

The more interesting version of that old ambition, not yet started: instead of training a
net that reproduces NNUE's architecture, regress Stockfish's oracle score against *this
project's own* detected/quantified features — failure-lattice concepts,
`collapse_criticality` breakdowns, HUGM's components — instead of raw piece-square
positions. That asks how much of a black-box evaluator this project's own interpretable
feature set can already explain, which is a distillation question, not a
train-a-clone-from-scratch question.

## Where to look next

- `src/eval/threat_graph.rs` — the graph itself and every primitive/detector named above,
  each with its own doc comment explaining the "why," not just the "what."
- `tests/known_games.rs` — the regression suite: real, sourced, named games, each with the
  specific hand-verified finding it locks in written directly above the assertion.
- `FINDINGS.md` — the archive: full chronological history, including every rejected design
  and exactly why it was rejected, for anyone who wants the detail this document
  intentionally leaves out.
