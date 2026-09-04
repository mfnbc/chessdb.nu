---
name: position-eval
description: Produce a reasoned, commentary-style chess position evaluation by reading `full_report.nu`'s comprehensive nuon report in priority order — deliberately without relying on hugm-eval's aggregate numeric score, which is an untested hand-tuned formula.
---

# Position evaluation (read the report, don't trust a score)

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
stated. `scripts/play/full_report.nu` (2026-09-03) strips every computed-valuation field
(`final_score`/`aggregated.*_cp`/`engine_score`/per-fact `consequence`/`see_cp`/
`centipawns`) out of the report *before it's ever printed* — a generic name-pattern filter
(`shakmaty_compose.nu`'s `strip-scores`), not a promise to look past a number sitting right
there. That's the fix that actually holds: a number that was never in the output can't be a
standing invitation to shortcut past the reasoning below, regardless of what was decided
last time it came up.

What **is** trustworthy: the individual structured facts the filtered report is built
from — `tactical.*` and `positional.*`. Each of those lists (hanging pieces, forks, pins,
outposts, isolated pawns, ...) is independently regression-tested against real,
hand-verified positions (`nu_plugin_chessdb/tests/`) and does not depend on the aggregate
formula at all. This skill's job is to take that structured, individually-solid data and do
the part a fixed linear formula structurally cannot: weigh which facts actually matter *in
this position*, the way a strong player or commentator does, and write the reasoning
down — not compute a fake-precise number.

## Input

A move history, passed as `args` — a nuon list literal of uci moves (e.g. `[e2e4 e7e5]`),
matching every other tool in `scripts/play/`. If you're screening a candidate move mid-game
(the `check_move.nu` workflow), apply the move first via `chessdb apply-uci`, then pull the
resulting position's full report — this skill evaluates one static position, it doesn't
replay moves itself.

## Method

### 1. Pull the one comprehensive report, once

```
nu scripts/play/full_report.nu '[<uci moves>]'
```

This single nuon record — `shakmaty_compose.nu`'s `full-report`, `board_probe.nu`'s
shakmaty-derived geometric/structural facts merged with the tactical/positional detector
layer — has everything below. Read `side_to_move`, `tactical.*`, `positional.*`,
`material_white`/`material_black`, `mate_in_1_exists`, `in_check`, `king_tropism_us`,
`initiative_us`, and `squares` (every one of the 64 squares' occupant/controls/
attacked_by_white/attacked_by_black — reach into this directly for "what attacks/defends
this specific square" instead of a separate `attackers_map.nu`/`control_map.nu` call when
the report is already loaded). There is no score field left to ignore — `strip-scores`
already removed `final_score`/`engine_score`/`aggregated.*`/every per-fact `consequence`/
`see_cp`/`centipawns` before this report was ever generated.

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
`attacker_count` vs `defender_count` where present), and who does it favor.

**Do not read `consequence`/`see_cp` as the answer to "is the danger real" (2026-09-02,
user feedback, FINDINGS.md).** These looked more trustworthy than `final_score` because
each is tied to one concrete exchange rather than a summed formula, but `find_forks` is
still backed by the known-buggy `see_chain`, and even the direct-subtraction pricing
`find_outnumbered`/`find_mover_favored` use is still a *computed valuation*, not a raw
fact — a real game (Game 12) shows leaning on it can lead to a move Fruit's own search
still rated below its actual best. `check_move.nu`/`calc_line.nu` no longer print these
fields at all. When `attacker_count`/`defender_count` (or a fork's target list) says a
piece might be genuinely contested, verify by actually calculating: `calc_line.nu` to walk
the real capture sequence and read the resulting raw piece list, or
`attackers_map.nu`/`control_map.nu` to see directly what defends what — never by reading a
pre-computed verdict. A real hanging piece (`safe_to_capture: true`) or a mate-in-1 makes
everything below this section close to irrelevant; that field stays trustworthy because
it's a direct legality/capture fact, not a valuation.

**b. Material** — count it yourself from `full_report.nu`'s `material_white`/
`material_black` (each `{pawns, knights, bishops, rooks, queens, bishop_pair}` — pawns=1,
knights/bishops=3, rooks=5, queens=9, standard values, not a computed score). State the raw
imbalance in plain terms ("White is up a clean pawn," "material is level but Black has the
bishop pair"). `scripts/play/material.nu` still exists for a quick standalone check
mid-game without pulling the whole report — it prints only the raw piece counts,
deliberately never a centipawn sum, so there's nothing to glance at and no reflex to
trigger.

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

## Re-evaluate the plan every move — don't just state it once and coast

Game 13 (2026-09-02, FINDINGS.md) got checkmated after playing every individual move
cleanly by `check_move.nu`'s own standard. The actual failure wasn't a missing tactic on
any one move — it was having no standing plan to check moves *against*, so nothing ever
prompted noticing that the position's whole character had changed. Consolidating with
`25.Rg1` (abandoning the only file contesting a rook, while the king was already under a
coordinated knight+queen attack) looked like ordinary, safe housekeeping in isolation. It
was the single largest evaluation swing of the entire game (`fruit_analyze.sh`,
`-418cp → -1800cp`) — invisible to a single-ply safety check, and invisible to a plan that
was never re-examined after the position turned sharp.

**The fix: after every opponent reply, before picking a candidate, explicitly ask two
questions, not just one.** (1) The usual: what does structural analysis say is the theme
here (section 2 above)? (2) The one that was missing: *has the position's character
itself changed since the plan was last stated* — specifically, has the opponent started
delivering forcing moves (checks, threats requiring an immediate response) in sequence,
rather than one isolated tactic? If so, the standing positional plan is no longer the
right frame at all — the position needs
[forcing-line calculation](#calculate-forcing-lines-before-judging-a-sharp-position-dont-react-one-move-at-a-time)
applied to *every* candidate under consideration, including ones that look like quiet
consolidation, until the forcing sequence actually resolves into a genuinely quiet
position again. A plan formed for a quiet position doesn't automatically stay valid once
the position stops being quiet — that transition has to be noticed on purpose, every
move, not assumed away because the plan already existed.

## Wide before deep — check the whole square, not just the piece already worrying you

Game 13's `23...Qxf2+` (FINDINGS.md, 2026-09-02) wasn't a missing-tool problem, and it
wasn't really a missing-calculation problem either — `calc_line.nu` correctly verified the
*knight's* `Nxf2` fork before that move was played. The actual failure: `f2` was attacked
by *two* black pieces at once (`Qb6` and `Ng4`), `attackers_map.nu` (or `full_report.nu`'s
own `squares.f2.attacked_by_black` if the report was already loaded) would have shown both
side by side in one call, and it was never run — attention went
straight from "a fork was flagged" into verifying that one specific piece's threat, never
back out to the more basic question of what, in total, attacks the square in play. This is
the general failure pattern behind more than one incident this session (also see the
`Bd3`/`Bxh3` "HANGING lines can be independent facts" findings): getting pulled deep into
the first specific threat presented, instead of surveying the square/piece widely first
and only then going deep on whichever of the (possibly several) real threats it turns up.

**The fix: whenever a square becomes contested — named in a fork's target list, a hanging
entry, or anywhere else — check *that square's* full attacker/defender picture (`attackers_map.nu`,
or `squares.<sq>` in an already-loaded `full_report.nu`) first, before calculating any
single piece's specific line.** Not "does the piece I'm
already worried about actually threaten this" — "what, in total, attacks or defends this
square, from every piece on the board." Only once that full picture exists does it make
sense to calculate any one of the threats it reveals in depth with `calc_line.nu`. Going
deep before going wide can verify a real threat correctly and still miss a second, larger
one sitting on the exact same square.

**This recurred the very next game (Game 14, 2026-09-02), in a sharper form worth naming
specifically: `check_move.nu`'s "MY PIECES AT RISK" section had *already* printed the
exact warning needed — `MOVER_FAVORED ... Rook@c5 ... verify with calc_line.nu` — and it
was still skipped, because a different, more interesting-looking fact (an enemy bishop
hanging) sat in the same output and pulled attention there instead.** That section is
labeled "check this first" and ordered first for exactly this reason; reading past a
warning already surfaced about one of *my own* pieces because something more exciting is
sitting nearby is the same distraction pattern as not running the wide check at all — it's
actually worse, since the tool had already done the wide-checking work and named the exact
danger. **Rule, not just for new squares: any `HANGING`/`OUTNUMBERED`/`MOVER_FAVORED`
entry on my own piece in "MY PIECES AT RISK" gets resolved — via `calc_line.nu`, as the
line itself says — before reading anything else in that output, full stop, regardless of
what else the output also contains.** A skill section being written down once does not
make the underlying tendency stop on its own; it has to actually change which line gets
read first, every time, including the next time it's tempting not to.

## Over-defended is not the same as safe — check the attacker's value against the defended piece's, not just the count

Game 16 (2026-09-03, FINDINGS.md) lost a bishop for a pawn (move 14) and then the exchange
(move 22, rook for bishop) via the exact same unrecognized gap, twice in the same game.
`check_move.nu`'s `MOVER_FAVORED ... 1v3` and `... 1v2` flags were both read as "more
defenders than attackers, therefore safe" — literally true by count, and wrong in effect,
because in both cases the single attacker (a pawn attacking a bishop; a bishop attacking a
rook) was worth *less* than the piece it was attacking. Defender count only governs what
happens if the exchange *continues past the first capture* — it says nothing about whether
the first capture already favors the attacker. A pawn (100) capturing a bishop (330) nets
the attacker +230 the instant it happens, before any recapture; three defenders recapturing
afterward only ever win back the *pawn's* value (100), leaving the net at -230 regardless of
how many pieces line up to retake. The identical arithmetic makes a minor piece capturing a
rook ("winning the exchange", ~+170 for the attacker) favor the attacker independent of
defender count, for the same reason: rook(500) is a higher denomination than bishop(330).

**The fix: before reading any `attacker_count`/`defender_count` pair as safe because
defenders ≥ attackers, check whether *any single attacker* is worth less than the piece
being defended.** If so, that attacker can capture and already come out ahead on the very
first exchange — the position is not safe regardless of how many defenders queue up behind
it. This is the same "verify the value, not the count" discipline as the Game 15 Qc5/Rd1
cases above, but sharper: those were about misreading *which* piece a flag named or trusting
a `consequence` verdict outright; this is about correctly reading the attacker and defender
counts and *still* concluding "safe," because a count comparison by itself is the wrong test
whenever the two sides' piece values aren't already close (pawn vs. minor, minor vs. rook).
A defender-count majority only protects against a losing *continuation* — it does not
protect against a favorable *first* capture.
