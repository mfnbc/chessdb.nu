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

None of the above means this system never computes a real static-exchange-evaluation — it
answers a different question than `collapse_criticality` does. `ThreatGraph::see`/
`see_chain` *does* run an optimal-recapture-sequence search and return a real centipawn net
score; `Fork.see_cp`/`consequence` is the one field still backed by it (`Outnumbered.see_cp`/
`consequence` and `MoverFavored.see_cp`/`consequence` both moved to direct-subtraction
pricing instead — see "What this deliberately does not do" below). The distinction:
`collapse_criticality` asks "is capturing here even safe" (a legality/check question, no
material value involved); `see`/`see_chain` asks "if it's captured, what's the material
outcome" (a value question, once safety is already established or irrelevant).
Both are real searches over a small local exchange, deliberately kept separate because they
answer different questions — this section is about avoiding a *third* thing (a full
position search to judge whether a capture is strategically good, which is the real
engine's job), not about avoiding exchange arithmetic entirely.

## How primitives become features

None of the sections above stop at being interesting facts about a position — each one is a
layer the next layer is built on, and nothing above this line collapses to an opaque score
before it needs to:

1. `control(sq, color)` — one signed number per square. The only raw primitive.
2. `attackers()`, `zone_control()`, `checkers()`, `is_in_check()` — named relations read
   directly off that one number, still nothing but geometry.
3. The failure lattice (`find_hanging` … `find_false_safety`, now also `find_mover_favored`)
   and `collapse_criticality` — composed *detectors*, each returning a typed struct that
   names the piece, the square, and the raw counts behind its conclusion, not a bare
   yes/no. `find_forks` attaches a static-exchange-evaluation verdict (`see_cp`/
   `consequence`, always from the mover's perspective — the side that would actually
   initiate the capture, never a separate White/Black field since positions here are always
   the canonical White-to-move frame) via `ThreatGraph::see_chain` — see "Pathfinding an
   exchange instead of calculating it" above for how this differs from
   `collapse_criticality`, and "What this deliberately does not do" below for exactly which
   of these numbers are currently trustworthy (less than their own doc comments used to
   claim — see the 2026-09-01 findings). `find_outnumbered` attaches the same kind of
   verdict but, since 2026-09-01, computes it by direct subtraction instead — see below,
   `find_forks` is now the only detector still backed by `see_chain`. `find_mover_favored`
   is the rung `find_hanging`/`find_outnumbered` structurally can't reach — a piece with at
   least as many defenders as attackers (the raw count says it's fine, so neither of the
   other two detectors fires) where the *first* exchange still favors the mover because the
   cheapest attacking piece is worth less than what it's attacking. `see_cp` is computed by
   direct subtraction (piece value minus the cheapest attacker's value) — never through
   `see_chain` at all, which was tried on this exact shape first and found to give the
   wrong sign (`FINDINGS.md`, 2026-09-01). Originally shipped restricted to *exactly* 1
   attacker/1 defender, then generalized the same day after a live game lost a queen with
   *two* real defenders to a single bishop — a bad first exchange doesn't stop being bad
   just because more defenders exist, and the detector never actually needed to know the
   exact defender count beyond "at least one, and not fewer than the attacker count."
   `find_outnumbered` used the same direct-subtraction approach applied to it later the same
   day, once a live game caught `see_chain`'s sign-flip bug mislabeling a genuinely hanging
   knight `consequence: Losing` (i.e. "safe") — see the Findings table and the 2026-09-01
   (continued) entry in `FINDINGS.md`.
4. `build_sensor_report` (`position.rs`) folds every detector's output for one position into
   `SensorReport` — the single typed representation of "what did this ply's board actually
   show," replacing the older, string-keyed `EvalGroups.terms` grab-bag as the one thing
   downstream code reads. `SensorReport` isn't tactical-only: it also carries
   `PositionalReport` (outposts, pawn structure, king exposure, development, …),
   `MaterialConceptReport`, and a handful of whole-position scalars (`king_tropism_us`,
   `initiative_us`, …) — everything above is the tactical slice of a report that's actually
   comprehensive.
5. `extract_concepts` (`concepts.rs`) turns a `SensorReport` into `Vec<Concept>` — the same
   shape regardless of whether a `Concept` originated from a tactical detector, a positional
   extractor, or a material scalar, so everything downstream treats them uniformly. Every
   `Concept` carries `mover: Mover` (`Us`/`Them` — never a real color, see below), never
   `side: Side`.
6. `rank_issues_for_position`/`rank_issues_for_player` turn `Concept`s into `GatedIssue`s:
   severity × ELO-relevance × confidence, ranked — the actual coaching output a player or an
   LLM sees. `GatedIssue.mover` is the same `Mover` type, carried straight through.
7. `chess-tactical-events` (Nu layer) persists the *structured* facts from step 3 — not
   step 6's ranked narrative — per ply, per square, per concept, so the same feature can be
   read back move-to-move: this is what makes it possible to graph where a game got
   volatile, not just say so once for the position in front of you.

Every arrow in that chain is a composition of something simpler, never a re-derivation from
scratch — the "emergence" in the one-paragraph version above is this whole stack, not just
`control` on its own.

`PositionRecord.final_score` (step 4's home for the whole-position score) is `us − them`
relative to whoever is actually to move — that stays the convention everything in this file
computes with, and mirrors the DB's canonical (White-always-to-move) position identity.
There is deliberately **no** `final_score_white_relative` sibling field — a caller comparing
scores across positions with different sides to move has `side_to_move` right there and can
compute `if side_to_move==White {final_score} else {-final_score}` itself in one line. That
field existed once and was removed (`FINDINGS.md`, 2026-09-01): a sign-convention audit found
this crate had accumulated several *different* flip conventions side by side — a
White-relative score, mover-relative `Concept`/`GatedIssue` tags that were nonetheless
un-flipped back to real color for output via a blanket text-substitution pass
(`unflip_phrase`, since deleted), and real-color `PieceRef` squares/colors (a genuinely
different, necessary kind of correction — see below). One numeric convention
(mover-relative, plus `side_to_move` for whoever wants to translate it) turned out to be
enough; every place that used to compute or consume a second, White-relative convention now
just does that one-line translation itself, matching how `Fork.see_cp`/`Outnumbered.see_cp`/
`MoverFavored.see_cp` already worked (no color field at all, client derives real color from
`piece.color`).

`Concept.mover`/`GatedIssue.mover` extend that same pattern to the two structs that can't
anchor to one piece (`material_imbalance`, `bishop_pair`, `king_exposed`, …): `Mover::Us`/
`Mover::Them`, never `Side::White`/`Side::Black`. This isn't just a stylistic rename —
`Concept`/`GatedIssue` are built entirely inside `normalize_to_white_to_move`'s internal
frame (`canonical.rs`, where the mover is always literally `White`), so a `Side` field there
either meant "the mover" in disguise (true for almost every concept) or, if a future concept
ever needed a genuinely real, mover-independent color, had no way to say so — the type
couldn't distinguish the two. `Mover` can't accidentally mean the wrong thing: `Us` always
means "whoever `side_to_move` says is to move," full stop, in every frame, with nothing to
flip. The real-color `PieceRef` fields these concepts sometimes reference (e.g. a fork's
`attacker`) are a genuinely different, orthogonal kind of information — *where a piece is* is
a board fact needing the real, un-flipped square/color; *who a finding favors* is a value
judgment that should never have claimed a color in the first place. Conflating the two inside
one `Side`-typed field is what made `unflip_phrase` (a blanket find/replace of the literal
words "White"/"Black" inside already-rendered coaching phrases, deleted) necessary, and it
was the most fragile part of the whole pipeline — any future phrase that didn't route color
through the internal `us_color`/`them_color` variables would have silently corrupted text
sent straight into the `chess-coach` LLM prompt (`ai/mod.nu`).

A handful of `core.rs` functions that predate this whole pipeline — `fen_info`,
`mobility_summary`, `attack_summary`, `checker_summary`, `is_legal` — are exposed directly
as plugin commands (`chessdb fen-info`, `chessdb legal-moves`, `chessdb attack-summary`,
`chessdb checker-summary`, `chessdb is-legal`), not just called internally. They sit
outside the numbered pipeline above (no `SensorReport`, no concepts, no ranking) — cheap,
single-purpose answers to "what are my options"/"is this legal"/"what's attacked" that
don't need paying for a full `hugm-eval` call.

**The shakmaty-1:1 architecture (2026-09-03, superseding an earlier, same-session
`square_control`/`square_attackers`/`square_swap_list`/`board_probe` generation of
rust-composed commands — all four removed, not kept alongside the new layer).** Per
explicit user direction ("a tree of reports [is] compiled ... in nushell" instead of the
plugin curating which shakmaty primitives answer which chess question, and "keep the
bitboard at all levels and let the client translate"), `chessdb` now exposes shakmaty's own
functions close to 1:1 — `chessdb geom-attacks`/`geom-ray`/`geom-between`/`geom-aligned`
(the `attacks::` module, pure geometry, `occupied` always an explicit caller-supplied
input), `chessdb board-pieces`/`board-piece-at` (`Board`'s own piece-placement bitboards),
`chessdb square-is-light` (`Square::is_light`, no board at all). Composition — "what
attacks this square," the swap-list's recursive x-ray removal, the whole-board probe —
moved to nushell (`nu_plugin_chessdb/scripts/play/shakmaty_compose.nu`), not rust: since
`occupied` is a plain `list<string>` at the leaf level, removing a square from it for an
x-ray reveal is just an ordinary nu `where` filter, no rust loop needed. Each nu-composed
replacement was verified byte-identical against the rust-composed command it replaced, on
real positions, before that command was removed (same A/B-diff discipline as the
`detect_skewers` migration below). `nu_plugin_chessdb/scripts/play/control_map.nu` and
`attackers_map.nu` (both now built on `shakmaty_compose.nu`) render their respective
outputs — geometry stays leaf-command-only; nushell composition is presentation and
aggregation, not a second computation of the underlying geometry.

This same pass also mined shakmaty for hand-rolled geometry already inside the pipeline
(`detect_skewers`, now rewritten onto `attacks::rook_attacks`/`bishop_attacks` like its
sibling `detect_pins`; `chebyshev_distance`, now `Square::distance`; a couple of duplicated
pawn-attack-destination computations, now `attacks::pawn_attacks`) and, per an explicit
standing principle (`CLAUDE.md`, "Chessdb defers to shakmaty for anything geometric"),
attempted — then correctly reverted — a rewrite of `king_safety_score`'s pawn shield/storm
loop once an A/B numeric diff against real positions showed it wasn't behavior-preserving;
see the Findings table and `FINDINGS.md` for the full account of what was and wasn't a real
gap.

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
| A piece with real defenders — even more defenders than attackers — can still favor the mover on the first exchange if the cheapest attacker is worth less than the piece it's attacking (a queen with two defenders, a king and a knight, still lost outright to one bishop). Neither `find_hanging` nor `find_outnumbered` can see this, since both are pure count comparisons. Computed by direct subtraction (cheapest attacker) rather than `see_chain`, which was found to give the wrong sign even on the simplest 1-vs-1 shape | `find_mover_favored` | `mover_favored_pawn_attacks_knight_defended_only_by_a_rook`, `mover_favored_does_not_fire_when_the_lone_defender_outvalues_the_attacker`, `fruit_game_three_queen_lost_to_a_bishop_despite_two_defenders` |
| `find_outnumbered`'s `see_cp`/`consequence` used to be priced via `ThreatGraph::see` and inherited its sign-flip bug — a real 2-attacker/1-defender knight (cheapest attacker a pawn) was labeled `consequence: Losing` ("safe for the defender") when it was actually just lost to the pawn. Switched to the same direct-subtraction pricing `find_mover_favored` uses; `find_forks` is now the only detector still backed by `see_chain` | `find_outnumbered` | `fruit_game_four_outnumbered_knight_was_mislabeled_safe_by_the_buggy_see_chain` |
| `sensor_report.mate_in_1_exists` was fully computed but reachable only through the ELO-gated `gated_issues` path (`--player-elo`) — a plain `--verbose true` call, the one this session's live-play checking actually used, could walk straight into a real, computed mate-in-1 with zero warning in `.explanations`. Both explanation renderers now check it directly and unconditionally, first, ahead of every other phrase | `render_explanations`, `render_structured_explanations` | `fruit_game_six_mate_in_1_was_computed_but_never_surfaced_in_explanations` |
| `king_exposure`'s `shelter_files` count (how many of the 3 files centered on the king have *any* friendly pawn anywhere on them) can't distinguish a bare flank file from a completely pawnless king-file — 2 of 3 files "sheltered" read as safe even when the file directly in front of the king (the specifically dangerous one — direct rook/queen access) has no pawn at all. `king_file_open` is now a separate, independently-triggering field for exactly that case | `extract_king_exposure` | `fruit_game_nine_castling_onto_a_pawnless_king_file_read_as_zero_exposure` |
| `detect_skewers` hand-walked 8 hardcoded direction tuples one square at a time instead of using shakmaty's occupancy-aware `attacks::rook_attacks`/`bishop_attacks` the way its sibling `detect_pins` already did — A/B-verified byte-identical against the old implementation across every known-game/motif test FEN, both colors, before the old code was removed | `detect_skewers` | `runs_cleanly_on_every_known_game_and_motif_test_fen`, `detects_skewer`, `skewer_negative_no_back_piece` |
| Not every hand-rolled-looking loop is a real gap: `king_safety_score`'s pawn shield/storm computation looks like two independent per-file queries but is genuinely sequential (a shared early-exit couples "nearest own pawn" to "nearest enemy pawn") — an attempted split into independent bitboard queries passed `cargo test` but was caught as a real regression only by an explicit numeric A/B diff against real positions, and was reverted | `king_safety_score` (unchanged) | no dedicated test — caught by manual `groups.king_safety.blended` A/B diff, not `cargo test` alone; see `FINDINGS.md` |
| `mobility_summary`'s `mobility_san` used `San::from_move`, which deliberately omits the check/checkmate `+`/`#` suffix — `forcing_moves.nu`'s CHECKS and CHECKMATE-AVAILABLE detection (both string-matching that suffix) had silently returned empty for the tool's entire history. Found while cross-verifying a since-removed `board_probe` command's output against a known Fool's Mate position, not from any live-play incident. `SanPlus::from_move` (plays a clone, then checks the result) is the correct call | `mobility_summary` | `checking_and_mating_moves_carry_their_san_suffix` |
| `attacks::attacks(square, piece, occupied)` is shakmaty's own single dispatcher for every piece role's geometry (pawn/knight/king ignore `occupied`; bishop/rook/queen use it for blocking) — exposed directly rather than composed, so `occupied` stays a caller-supplied bitboard at the leaf, not something derived internally from a board's current state | `geom_attacks` | `geom_attacks_knight_matches_the_independently_established_square_control_fact`, `geom_attacks_rook_on_an_open_board_covers_the_whole_rank_and_file`, `geom_attacks_rook_stops_at_the_given_occupied_blocker` |
| `attacks::ray`/`attacks::between`/`attacks::aligned` (line/betweenness/alignment geometry) were computable internally via the same rays table `detect_skewers` etc. rely on, but not queryable directly — exposed as their own leaf commands, verified against shakmaty's own doc examples rather than self-invented positions | `geom_ray`, `geom_between`, `geom_aligned` | `geom_ray_and_between_match_shakmatys_own_doc_examples`, `geom_aligned_matches_shakmatys_own_doc_example` |
| `Board::occupied`/`by_color`/`by_role`/`by_piece`/`piece_at`/`Square::is_light` were each already used internally throughout this file but never queryable directly — the leaf-layer commands that replaced `square_control`/`square_attackers`/`square_swap_list`/`board_probe`'s rust-side composition (all four removed 2026-09-03) compose these instead, in nushell (`scripts/play/shakmaty_compose.nu`), not rust | `board_pieces`, `board_piece_at`, `square_is_light` | `board_pieces_filters_match_known_start_position_placement`, `board_piece_at_matches_the_independently_established_square_control_fact`, `square_is_light_matches_shakmatys_own_doc_verified_convention` |

## What this deliberately does not do (yet)

- **Exchange pricing exists and is depended on, but `ThreatGraph::see`/`see_chain` itself
  is now known to be unreliable even on the simplest possible case, not just imprecise on
  deep chains.** `Fork.see_cp`/`consequence` (`concept_types.rs`) is live output backed by
  `see_chain`, always priced from **the mover's perspective** (the side that would actually
  initiate the capture — the opponent of whoever owns the attacked piece), never as a
  separate White/Black field, since every position here is already normalized to the
  canonical White-to-move frame. `see_chain`'s bugs (wrong per-step pricing, the contested
  square drifting past the first recapture — `FINDINGS.md`'s "see_chain gives wrong answers
  for 2+ step exchanges") were originally believed to leave the *first* capture-and-recapture
  exact. **That turned out to be false**: because the function only ever removes pieces from
  the board clone and never places a capturing piece back on the contested square, once both
  the original piece and the recapturer are gone the square reads as fully empty — and a
  completely unrelated piece that now has a freshly-opened line to that empty square gets
  treated as a further, real attacker. This flips the sign of even a bare "piece defended
  once, attacked once" exchange (`FINDINGS.md`, 2026-09-01, exact minimal reproduction
  included). An attempt to fix `see_chain` itself directly (the standard swap-off algorithm,
  backward minimax pass included) looked plausible on a real position, then failed its own
  sanity check on the simplest possible input (pawn-takes-pawn-takes-pawn, which must net
  exactly zero) — not committed, per this project's standing rule against shipping
  unverified numbers. `find_mover_favored` (see the failure lattice above) sidesteps this
  entirely by not using `see_chain` at all — it computes `see_cp` by direct subtraction
  (piece value minus the cheapest attacker's value) for real-defenders-exist,
  attackers-don't-outnumber-defenders positions, deliberately answering only "was the
  *first* exchange favorable," not "what does the whole square resolve to."
  `find_outnumbered` was switched to the same direct-subtraction pricing the same day, after
  a live game caught `see_chain`'s sign-flip bug mislabeling a genuinely hanging knight as
  `consequence: Losing` ("safe") right before it would have been played into — see
  `FINDINGS.md`'s 2026-09-01 (continued) entry and the
  `fruit_game_four_outnumbered_knight_was_mislabeled_safe_by_the_buggy_see_chain` regression
  test. `Fork.see_cp`/`consequence` is now the *only* field still backed by the still-buggy
  `see_chain` and should be treated as unverified for any case with 2+ attackers or
  defenders, not merely approximate — a correct fix still needs the real swap-off algorithm,
  done carefully enough to pass its own sanity checks, which the 2026-09-01 attempt did not.
  The safety ladder and `collapse_criticality` remain answering a different question
  entirely (is capturing even legal/safe, no material value involved) — see "Pathfinding an
  exchange instead of calculating it" above.
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
