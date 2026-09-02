# Live-play tools (Fruit games)

These are the tools used to actually play and calculate against the Fruit UCI
engine, developed and hardened over ten games (see `FINDINGS.md`'s dated
entries for the specific incidents behind each one). Moved here from a
session-tied scratchpad on 2026-09-02 so they survive a session restart, not
just a context compaction — nothing in these scripts is one-off scratch work.

All of them assume `chessdb` (this crate's plugin, `nu_plugin_chessdb`) is
already registered in the active nu shell (`plugin add` / `plugin use`), and
none of them print or rely on `final_score`/any aggregate numeric score —
see `.claude/skills/position-eval/SKILL.md` for why.

- **`check_move.nu <history> <candidate>`** — screens one candidate move:
  applies it, reports hanging/outnumbered/mover-favored pieces on your own
  side first (deliberately before anything else), then forks/pins/etc. and
  prose explanations. The fast, mechanical, always-run-first filter.

- **`check_move_2ply.nu <history> <candidate>`** — after playing the
  candidate, enumerates *every* legal opponent reply (no ranking, no "best
  reply" picked) and reruns the same own-pieces-at-risk check on each.
  Breadth-first enumeration, not a search.

- **`forcing_moves.nu <history>`** — lists every legal check and capture for
  the side to move (from `mobility_san`'s own notation), unranked. The
  branch list a real calculation starts from.

- **`calc_line.nu <history> "<candidate line>"`** — walks a full calculated
  variation move by move, printing hanging pieces / forks / king exposure /
  raw material at *every* ply, not just the last. Stops cleanly on an
  illegal move. Use with `forcing_moves.nu`: enumerate the forcing branches,
  then walk the testing ones here to a quiet position before judging it.

- **`fruit_move.sh "<uci history>" [movetime_ms]`** — asks the real Fruit
  UCI engine for its actual move from a position (default 1000ms). This is
  Fruit's real search — use it to get the opponent's actual reply, never as
  a stand-in for your own calculation.

- **`fruit_analyze.sh "<uci history>" [movetime_ms]`** — for each prefix of
  a finished game's move list, asks Fruit to search that position and
  prints its own score (from whoever's turn it is at that ply — the score's
  perspective flips every ply, normalize by hand before comparing across
  plies). Used for postmortems, not live play.

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
