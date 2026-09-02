---
name: position-eval
description: Produce a reasoned, commentary-style chess position evaluation from chessdb's structured positional/tactical reports — deliberately without relying on hugm-eval's aggregate numeric score, which is an untested hand-tuned formula.
---

# Position evaluation (no score-trusting)

## Why this exists

`chessdb hugm-eval`'s `final_score`/`aggregated.total_cp` is a fixed linear sum of
hand-tuned weights (material table ported from the Critter engine, plus guessed
coefficients for king safety, pawn structure, development, ...). It has never been
battle-tested — no self-play, no calibration against real game outcomes, no ELO
validation. Do not use it, or any of `aggregated.material_cp`/`positional_cp`/`tactical_cp`,
as the basis for a verdict. Do not use `engine_score` either — that's a *different*
untested number, not a real engine's output despite the name.

**Don't just decline to trust it — don't look at it at all.** Deciding "I won't use the
score as the basis for a verdict" and then still having it print in the output wasn't
enough in practice: over a single game (2026-09-02, FINDINGS.md) the habit of scanning
several candidates for the highest number crept straight back in, even with that intent
stated. Strip `final_score`/`aggregated.*_cp`/`engine_score` out of whatever tool output you
read *before* looking at it (see `check_move.nu`'s own fix, same date) rather than reading
past it by discipline each time — a number sitting right there is a standing invitation to
shortcut past the actual reasoning below, every single time, regardless of what you decided
last time.

What **is** trustworthy: the individual structured facts underneath that formula —
`sensor_report.tactical.*` and `sensor_report.positional.*`. Each of those lists (hanging
pieces, forks, pins, outposts, isolated pawns, ...) is independently regression-tested
against real, hand-verified positions (`nu_plugin_chessdb/tests/`) and does not depend on
the aggregate formula at all. This skill's job is to take that structured, individually-solid
data and do the part a fixed linear formula structurally cannot: weigh which facts actually
matter *in this position*, the way a strong player or commentator does, and write the
reasoning down — not compute a fake-precise number.

## Input

A FEN string, passed as `args`. If you're screening a candidate move mid-game (the
`check_move.nu` workflow), apply the move first via `chessdb apply-uci` to get the resulting
FEN, then invoke this skill with that FEN — this skill evaluates one static position, it
doesn't replay moves itself.

## Method

### 1. Pull the full structured report, once

```nu
use chessdb *
let ev = ("<FEN>" | chessdb hugm-eval --verbose true)
let s = $ev.sensor_report
```

Read `$ev.side_to_move` and `$s.*` — `tactical.*`, `positional.*`, `material.balance`,
`mate_in_1_exists`, `in_check`, `king_tropism_us`, `initiative_us`,
`development_score_diff`. Ignore `$ev.final_score`, `$ev.engine_score`, `$ev.groups`,
`$s.aggregated.*` per the note above — don't even read them into the reasoning below.

### 2. Walk every category, in priority order — this order is itself the reasoning

Chess commentary reasons in this order because each rung can override everything below it.
Work through all of these; most will be empty for most positions, which is itself a fact
worth noting ("no immediate tactics for either side").

**a. Immediate tactics/safety — can override every other factor**
`mate_in_1_exists`, `tactical.hanging`, `tactical.mover_favored`, `tactical.outnumbered`,
`tactical.false_safety`, `tactical.false_defense`, `tactical.overloaded`, `tactical.forks`,
`tactical.pins`, `tactical.skewers`, `tactical.discovered`. For each non-empty list: whose
piece is it (`piece.color` / `attacker.color`), is the danger real right now or merely
geometric (an attacked-but-well-defended piece is not the same as one about to fall — read
`consequence`/`see_cp`/`attacker_count` vs `defender_count` where present), and who does it
favor. A real hanging piece or a mate-in-1 makes everything below this section close to
irrelevant.

**b. Material** — count it yourself from `material.balance.white`/`.black`
(pawns=1, knights/bishops=3, rooks=5, queens=9 — standard values, not a computed score) and
note `bishop_pair_white`/`bishop_pair_black`. State the raw imbalance in plain terms ("White
is up a clean pawn," "material is level but Black has the bishop pair").

**c. King safety** — `positional.king_exposure` (`attacker_count`, `shelter_files`,
`king_file_open`, whichever king it's reported for) and `in_check`. A king with real
attacker pressure and a thin pawn shelter is a standing liability even with no tactic
available yet. Weight `king_file_open` specially — a pawn missing from the king's own file
is a materially worse danger (direct rook/queen access) than one missing from a flank file,
which is exactly why it's a separate field and not folded into `shelter_files`'s count.

**Before castling, check this proactively — don't wait for `king_exposure` to fire on the
position after.** A real game was lost partly to exactly this (2026-09-02, FINDINGS.md):
the c-pawn was traded away at move 9 for a genuine tactical point, then the king castled
queenside at move 12 directly onto that now-pawnless file — `shelter_files` still read 2 (a
flank-file pawn on each side counted as "shelter" even with the king's own file bare), so it
looked clean right up until a rook walked straight down that file several moves later. Check
`king_exposure`/pawn structure on *both* candidate castling squares before choosing one, not
only on whichever square got picked — a hole created several moves earlier by an unrelated,
individually-justified trade can turn a routine-looking castling move into a standing
structural liability that nothing flags until an opponent actually exploits it.

**d. Pawn structure** — `positional.isolated_pawns`, `doubled_pawns`, `passed_pawns`
(note `is_protected`), `pawn_islands`, `pawn_majority`, `pawn_breaks`. These are long-term,
not immediate — weight them accordingly against (b) and (c).

**e. Piece activity/space** — `positional.outposts`, `open_files`, `rook_on_seventh`,
`center_control`, `development` (`space_advantage`, `undeveloped_pieces` — an opponent with
2+ pieces still on their back rank in a sharp position is a real, citable factor),
`king_tropism_us`, `initiative_us`.

### 3. Synthesize — this is the part the formula can't do

Write 3–6 sentences of prose, the way an annotated game's commentary reads: name the
position's one or two *deciding* themes first, then the counterweights, then close with a
one-line qualitative verdict — never a number. Example shape (not a template to fill in
mechanically — the actual themes come from what step 2 found):

> White is up a clean pawn with no tactical compensation in sight for Black. Black's bishop
> pair offers some long-term hope, but with two minor pieces still undeveloped and the king
> stuck in the center, White's lead in development is arguably the bigger factor right now.
> Black's queenside pawn majority is a real long-term asset if the position simplifies, but
> that's a distant consideration next to the immediate structural and developmental gap.
> **Verdict: clear, probably durable edge for White.**

Verdict vocabulary to reach for (not exhaustive, match the actual read): "roughly balanced,"
"slight edge to \_\_\_," "clear advantage to \_\_\_," "winning for \_\_\_," "unclear —
compensation roughly balances the material," "critical/forcing — \_\_\_ must find X or Y."

### What this does not replace

`check_move.nu`'s "MY PIECES AT RISK" triage stays the fast, mechanical, always-run-first
filter before playing a move — it's cheap and catches the single most urgent question
("does this move hang something") without needing a full reasoned pass. Reach for this
skill when you want the fuller *why* behind a position, not as a replacement for that
per-move safety check.

## Calculate forcing lines before judging a sharp position — don't react one move at a time

Audited a tenth game (2026-09-02, FINDINGS.md) where a real fork was missed live: checking
one candidate move at a time (`check_move.nu`) caught the `Nxe3` fork on `Rf1`/`Bg2` at the
position after `14...Nxc4 15.Qb3`, but missed a second, simultaneous fork sitting in the
same position — `Be6` attacking both the queen and a pawn across two clear diagonals — that
only surfaced when the position several plies deep was actually calculated and inspected as
a whole, not reacted to one ply at a time. Single-move screening tells you whether *one*
candidate hangs something; it doesn't tell you what a forcing sequence leads to, because the
danger can be sitting quietly in a position two or three moves ahead of the move you're
actually screening.

**The fix: for any sharp or unclear position, calculate before comparing candidates —
don't generate candidates from static judgment alone.**

1. `forcing_moves.nu <history>` — lists every legal check and capture for the side to move
   (from `mobility_san`'s own `x`/`+`/`#` notation), nothing else, unranked. This is the
   branch list a real calculation starts from: forcing moves demand a response, quiet moves
   don't, so they're what you actually have to read out a few moves deep before trusting a
   verdict.
2. For each forcing branch worth following (yours and the opponent's most testing reply,
   picked by judgment, not by a tool), `calc_line.nu <history> "<candidate line>"` walks the
   whole line move by move and prints hanging pieces / forks / king exposure / raw material
   at *every* node — not just the last one. Read every intermediate position, not just the
   final one: a fork or hanging piece appearing mid-line is just as real as one at the end,
   and this is exactly the kind of thing single-move screening skips past.
3. Only once a forcing sequence reaches a quiet position (no more checks/captures worth
   following) does this skill's static method (material → king safety → structure →
   activity) apply to *that* resulting position — calculation gets you to the position
   worth judging; it doesn't replace the judging.

This is real calculation, not a search: the tool never ranks or scores a line, it only
verifies legality and reports structural facts at each node. Which branches to follow, how
deep, and what the resulting quiet position means are still entirely reasoned judgment calls
— the tool exists so that judgment is applied to a *correctly calculated* position instead of
a half-remembered or miscounted one.

## Don't stop at "safe" — this was the actual failure mode, not the score

Audited eight games' worth of move selection (2026-09-02, FINDINGS.md) and found the real
problem wasn't the score after all: it was that "safe" quietly became the whole decision
procedure. The pattern, move after move: filter candidates through `check_move.nu`, discard
anything that hangs material, then play whichever survivor looked like normal development —
never actually comparing the *positional* consequences of the surviving candidates against
each other. That's purely defensive (avoid losing) with no offensive half (actively choose
the position that's *better* for reasons beyond "nothing hangs"). A tactically-clean move
and a positionally-strong move are not the same claim, and only the first one was ever being
checked.

**The fix, concretely: when 2+ candidates survive the tactical filter, don't default to the
first reasonable-looking one — run this skill's full method (material through activity) on
*each* survivor and compare them against each other**, not just each against "is anything
hanging." Prefer whichever concretely: fixes a weakness in your own structure, creates or
targets a weakness in the opponent's (an isolated/backward pawn, a hole a piece can't be
challenged from, a color complex), improves a badly-placed piece, or gains space/activates a
plan already in motion. "Both are safe, and this one develops a piece" is not a finished
comparison — ask what plan each candidate actually serves.

**Have a plan before comparing candidates, not just a filter.** Before generating candidate
moves at all, name the position's positional theme out loud: is there a pawn structure to
aim for (an exchange-variation minority attack, a target on an isolated queen's pawn,
pressure down a half-open file), a piece that needs improving (a bad bishop to trade or
reroute, a knight that wants an outpost), or a weak square/color complex to occupy or deny?
A concrete plan generates and ranks candidates on its own; without one, move selection
degrades back into "pick whatever doesn't lose material," which is the exact failure this
section exists to name.
