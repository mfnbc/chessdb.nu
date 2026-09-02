# Live-play tools (Fruit games)

These are the tools used to actually play and calculate against the Fruit UCI
engine, developed and hardened over ten games (see `FINDINGS.md`'s dated
entries for the specific incidents behind each one). Moved here from a
session-tied scratchpad on 2026-09-02 so they survive a session restart, not
just a context compaction — nothing in these scripts is one-off scratch work.

All of them assume `chessdb` (this crate's plugin, `nu_plugin_chessdb`) is
already registered in the active nu shell (`plugin add` / `plugin use`). None
of them print or rely on `final_score`/any aggregate numeric score, and
(2026-09-02, extending that same discipline) none of them print `see_cp`/
`consequence` either — those per-fact SEE valuations looked more trustworthy
than the aggregate because each is tied to one concrete exchange, but
`find_forks` is still backed by the known-buggy `see_chain`, and a real game
(Game 12) shows relying on the direct-subtraction-priced ones can still lead
to a move Fruit's own search rated below its actual best. What these scripts
print instead: raw counts (`attacker_count`/`defender_count`), piece identity
and standard value (a fixed constant, not a search result), and fork/skewer
*target lists* — the structural facts, never the valuation. When one of
these flags fires, that's the cue to actually calculate the exchange
yourself (`calc_line.nu`, `attackers_map.nu`/`control_map.nu`), not to read
a verdict off this output. See `.claude/skills/position-eval/SKILL.md` for
the full reasoning behind both.

Same principle applied one level further (2026-09-02): none of these scripts
print a raw FEN either. A FEN is exactly the same class of opaque,
hand-parsed encoding as a score — decoding one correctly by eye is the exact
arithmetic that hung a bishop in live play. Every tool that shows a position
now renders an actual grid instead, via `board_overlay.nu`'s shared
convention; `material.nu`'s aggregate piece-count output already satisfies
the "list of pieces you can hand-calculate from" alternative, so it doesn't
need a grid too. FEN strings still exist internally, purely as plumbing
between `chessdb` calls — never as something printed for a human to parse.

- **`check_move.nu <history> <candidate>`** — screens one candidate move:
  applies it, reports hanging/outnumbered/mover-favored pieces on your own
  side first (deliberately before anything else), renders the resulting
  position as a grid (destination square highlighted), then fork target
  lists and discovered attacks. The fast, mechanical, always-run-first
  filter. No server-generated prose (`.explanations`) — that text embeds
  `see_cp`/`consequence`/tropism/initiative scores by construction; the
  structured counts and the grid cover the same ground without a number
  attached.

- **`material.nu <history>`** — raw material by piece count for both sides,
  nothing else. Deliberately never touches `material.balance.centipawns` —
  even a simple, untuned sum-of-standard-values is still a number inviting
  "just check if it's decisive" instead of actually judging compensation and
  activity. Count the imbalance yourself from the printed piece counts using
  standard values (pawn=1, knight/bishop=3, rook=5, queen=9), per the
  position-eval skill.

- **`check_move_2ply.nu <history> <candidate>`** — after playing the
  candidate, enumerates *every* legal opponent reply (no ranking, no "best
  reply" picked) and reruns the same own-pieces-at-risk check on each.
  Breadth-first enumeration, not a search. Renders a grid only on an illegal
  candidate (showing the position it was attempted from) — a grid per reply
  across every branch would be too much output for what this tool answers.

- **`forcing_moves.nu <history>`** — renders the starting position as a grid,
  then lists every legal check and capture for the side to move (from
  `mobility_san`'s own notation), unranked. The branch list a real
  calculation starts from.

- **`calc_line.nu <history> "<candidate line>"`** — walks a full calculated
  variation move by move, rendering each ply as a grid (destination square
  highlighted) alongside hanging pieces / fork target lists / king exposure /
  raw material, not just at the last ply. Stops cleanly on an illegal move.
  Use with `forcing_moves.nu`: enumerate the forcing branches, then walk the
  testing ones here to a quiet position before judging it. This is *the* way
  to answer "is this exchange actually good" — by watching the position and
  raw piece list change ply by ply, not by reading a precomputed valuation.

- **`fruit_move.sh "<uci history>" [movetime_ms]`** — asks the real Fruit
  UCI engine for its actual move from a position (default 1000ms). This is
  Fruit's real search — use it to get the opponent's actual reply, never as
  a stand-in for your own calculation.

- **`fruit_analyze.sh "<uci history>" [movetime_ms]`** — for each prefix of
  a finished game's move list, asks Fruit to search that position and
  prints its own score (from whoever's turn it is at that ply — the score's
  perspective flips every ply, normalize by hand before comparing across
  plies). Used for postmortems, not live play.

- **`board_overlay.nu`** — not run directly; the shared grid-rendering
  convention every tool above and below renders through (2026-09-02,
  replacing each having its own bespoke legend, then extended to replace raw
  FEN output too). Called with an empty layer list and no `--highlight` for
  a plain position render — the legend section is skipped entirely in that
  case, nothing to key. The convention: any layer worth
  showing is just a *square set* (`list<string>` of algebraic squares) —
  exactly what `controls`/`attacked_by_white`/`attacked_by_black`/etc.
  already return, and exactly what a `Bitboard` is on the Rust side, no
  adapter needed. Up to 3 layers get a fixed bracket each — `()`, `[]`,
  `{}`, in the order passed — and a square landed on by 2+ layers at once
  is a **stack**, always rendered `<>` regardless of which layers are in
  it — the same idea as NetHack showing a pile of items as one glyph
  instead of trying to draw each one; the per-layer counts printed in the
  header are the ": look" that says what's actually in a stacked square.
  A `--highlight <square>` always wins and renders `*X*`. See its own
  header comment for the full rationale, including why this stops being
  legible past ~3 layers.

- **`control_map.nu "<FEN>" <square>`** — renders every square the piece on
  `<square>` controls, split into 3 layers by what's on each square (own
  piece defended / enemy piece attacked / empty controlled) — mutually
  exclusive by construction, so `<>` never fires here, that's expected.
  Built on `chessdb square-control`, directly in response to the `Bd3`
  blunder (`FINDINGS.md`, 2026-09-02): a spatial view instead of mentally
  computing "does this diagonal/file/knight-jump reach that square," which
  is exactly the arithmetic that went wrong live. Reach for this before any
  move that places a piece on a square you haven't independently confirmed
  is safe.

- **`attackers_map.nu "<FEN>" <square>`** — the reverse question: every
  piece that attacks `<square>`, white as layer 1 `()`, black as layer 2
  `[]`. Unlike `control_map.nu`'s three layers, these two genuinely can
  stack on the same square — a real contested square shows `<>`. Built on
  `chessdb square-attackers`. More directly useful than `control_map.nu` for
  "is this square safe to move to" — it doesn't require first guessing
  which enemy piece to check, and works on an empty target square. Reach
  for this one first when the question is about the destination square
  specifically, `control_map.nu` when it's about a specific piece's full
  reach.

- **`control_overlap.nu "<FEN>"`** — whole-board version: every square
  White controls, Black controls, or both (a contested-square stack) at
  once, no single square of interest. Built on `chessdb attack-summary`,
  which has returned whole-board `attacked_by_white`/`attacked_by_black`
  since before this convention existed — this view was just never rendered
  until `board_overlay.nu` made a 2-layer stack grid a one-call thing.
  Reach for this for whole-position questions ("who controls the center,"
  "is this outpost square actually safe long-term") that the other two,
  both scoped to one piece or one square, can't answer.

## Method (see `.claude/skills/position-eval/SKILL.md` for the full version)

1. `check_move.nu` any candidate before playing it — cheap, catches the
   single most urgent question.
2. For a sharp or unclear position, don't stop at "nothing hangs": use
   `forcing_moves.nu` + `calc_line.nu` to actually calculate forcing lines
   to a quiet position before judging it.
3. Judge the resulting quiet position on structural merit (material, king
   safety, structure, activity) — never on a score. Compare 2+ safe
   candidates against each other with a real plan, not just against "is
   this safe."
