//! Threat graph: a complete attack-adjacency map (`ThreatGraph::build`, one
//! pass over the board, every square, not just occupied ones) that feeds two
//! genuinely different kinds of consumer.
//!
//! - **Geometry — `control`, `attackers`, `attacks_from`, `zone_control`,
//!   `is_in_check`.** Pure overlap/count, no piece values enter into it at
//!   all: "who attacks this square, how many more of mine than theirs."
//!   Cheap, reusable, and provably symmetric — `control(sq, White) ==
//!   -control(sq, Black)` on every square, on every position (see this
//!   file's own tests) — because both are just two counts read off the same
//!   shared `attackers_to[sq]`, not two independently-meaningful maps. This
//!   is the shared substrate `position.rs`'s scoring functions
//!   (`king_safety_score`, `development_space_score`, `piece_activity_score`,
//!   `detect_outposts`) read directly, instead of each independently
//!   re-deriving the same attack facts via its own `board.attacks_to`/
//!   `attacks_from` call. See FINDINGS.md's "continuity map" thread for the full
//!   migration history — and which call sites still don't use it
//!   (`detect_skewers`, the legacy `detect_forks`). `find_hanging` and
//!   `find_outnumbered` are the two most direct piece-safety applications of
//!   `control` itself — zero defenders vs. attackers simply outnumbering
//!   real defenders, respectively; `find_outnumbered` was the completeness
//!   gap noticed only after the fancier cross-references below were already
//!   built (FINDINGS.md).
//! - **Pricing — `see`/`see_chain`, consumed by `find_forks` only now.** A
//!   genuinely different question: not "who's here" but "what would this be
//!   worth." Runs an actual capture/recapture simulation, always priced from
//!   the mover's perspective — the side that would actually initiate the
//!   capture, i.e. the opponent of whoever owns the attacked piece — never as
//!   a separate White/Black field, since every position this crate analyzes
//!   is already normalized to the canonical White-to-move frame
//!   (`CLAUDE.md`), where a literal color field would be uselessly constant.
//!   Has a known, unfixed correctness bug in its multi-step chain math
//!   (FINDINGS.md: phantom captures on squares its own walk already emptied,
//!   among others) — deliberately not depended on by anything in the
//!   geometry layer above, so that bug doesn't reach
//!   `hanging_piece`/`detect_outposts`/`king_safety_score`/etc.
//!   `find_hanging`'s recorded piece `value` happens to equal what `see`
//!   would compute for that specific case (zero defenders means no
//!   recapture to walk), but it's read directly from `piece_value`, not
//!   routed through the buggy chain logic. `find_outnumbered` used to call
//!   `see()` too; a live game caught the bug in the act (a real
//!   2-attacker/1-defender knight scored `Losing`/"safe" by `see()` while
//!   the board showed it plainly hanging to the cheaper attacker,
//!   FINDINGS.md's 2026-09-01 entries) and it was switched to the same
//!   direct-subtraction, first-exchange-only pricing `find_mover_favored`
//!   uses (below) instead. `find_outnumbered` and `find_mover_favored` both
//!   attach a real `see_cp`/`consequence` verdict, computed the same
//!   bug-avoiding way — the first two attacker/defender-*count* comparisons;
//!   the third the case a raw count can't see at all (equal-or-better count,
//!   still favors the mover by value) — see `find_mover_favored`'s doc
//!   comment. `find_forks` is the one holdout still depending on the buggy
//!   `see_chain` — its `see_cp`/`consequence` should be treated as
//!   unverified for now, same as `see_chain` itself.
//!
//! **Not built from this graph at all**: pins, skewers, and discovered
//! attacks are detected independently in `position.rs` (`detect_pins`,
//! `detect_skewers`, `detect_discovered`) using shakmaty's own
//! occupancy-aware sliding-attack primitives directly — `detect_pins` calls
//! `attacks::rook_attacks`/`bishop_attacks` itself; `detect_skewers` instead
//! hand-rolls a ray walk that doesn't yet reuse them (a known, deferred gap,
//! FINDINGS.md). Forks *are* graph-derived (`find_forks` below), but through a
//! second, independent `detect_forks` also exists in `position.rs` purely
//! for the legacy scoring engine, with a threshold that can disagree with
//! this one (FINDINGS.md).
//!
//! - **Cross-reference — `find_overloaded`, `find_false_defense`,
//!   `find_false_safety`.** A third family, deliberately distinct from
//!   pricing: notice when two already-known facts overlap, instead of
//!   simulating or pricing anything. `find_overloaded` is self-contained
//!   (pure `attackers_to`, the mirror of a fork: one piece, multiple
//!   critical defensive responsibilities). `find_false_defense` genuinely
//!   needs external input — the `pins` list `position.rs` already computed —
//!   to ask "is this piece's only defender pinned in a way that actually
//!   stops it from recapturing here" (not just "is it pinned at all": a
//!   pinned piece can still legally move along its own pin line, so this
//!   checks `attacks::between(attacker, king)`, not just pin-list
//!   membership). All three are concrete instances of the "pathfind the
//!   graph, don't calculate the exchange" principle in FINDINGS.md — a
//!   deliberate design stance, not a shortcut taken because the pricing
//!   layer above is currently buggy.
//!
//! ## The failure lattice: a ladder of validation, not a flat concept list
//!
//! `hanging_piece` → `outnumbered` → `overloaded`/`false_defense` →
//! `false_safety` are not four unrelated detectors; they're rungs of one
//! question — "is this piece actually safe?" — asked at increasing depth,
//! each rung catching a miss the previous one couldn't see:
//! 1. **Attacked at all?** The shared precondition every rung below checks
//!    first — not a concept by itself.
//! 2. **Defended at all?** `find_hanging` (0 raw defenders) and
//!    `find_outnumbered` (>0 raw defenders, still fewer than attackers) are
//!    two halves of the same raw-count comparison, split only because the
//!    certainty differs (0 defenders is a guaranteed loss, worth exact piece
//!    value; outnumbered-but-nonzero is a softer signal).
//! 3. **Is a raw defender actually free to help?** `find_overloaded` (from
//!    the defender's side: am I already the sole defender of something
//!    else?) and `find_false_defense` (from the attacked piece's side: are
//!    *all* my defenders pinned off the recapture line?) are siblings, not
//!    one step — different anchor point, different strength of constraint
//!    (overload is soft/costly, pin is hard/illegal).
//! 4. **Does that change the verdict the raw count gave?** `find_false_safety`
//!    is the rung above both: it fires exactly when the raw count alone said
//!    "safe" (`raw_defender_count >= attacker_count`) but discounting
//!    defenders that are pinned-off-line *or* overloaded elsewhere reverses
//!    that verdict. This is the fact a player who "counted correctly" can
//!    still miss — the count was right, the commitments on the pieces doing
//!    the defending weren't seen. Every struct in this family carries both
//!    the raw and revised numbers (not just a conclusion) so a coaching
//!    report — and the database rows derived from it — can show exactly
//!    which layer of the ladder a given position exercises, and eventually
//!    which layer a given player's mistakes cluster at. Positional guides
//!    (center, flanks, king-safety-as-a-zone) are the next tier above this
//!    one, not yet built — this lattice stays purely material/tactical.

use shakmaty::{attacks, Bitboard, Board, CastlingMode, Chess, Color, FromSetup, Piece, Position, Role, Setup, Square};
use crate::eval::concept_types::*;

/// Feed a hypothetical board into shakmaty for the one thing the graph
/// genuinely can't answer itself: checkmate needs legal move generation,
/// not just attack geometry. (Check no longer needs this at all —
/// `ThreatGraph::checkers`/`is_in_check` answer that directly off the graph,
/// identities included, no shakmaty round-trip.) `Chess::from_setup`
/// validates full legality (exactly one king per side, the side not to move
/// isn't in check, etc.) and simply rejects anything that doesn't hold —
/// most commonly here, a king that was itself part of the cleared cluster
/// (`collapse_criticality` builds exactly these ad-hoc boards) and isn't the
/// candidate being tested, so it's just missing. Rejection reads as "not
/// checkmate" rather than propagated as an error — best-effort, not a
/// replacement for anything reliable.
fn is_checkmate_via_shakmaty(board: &Board, turn: Color) -> bool {
    let setup = Setup { board: board.clone(), turn, castling_rights: Bitboard::EMPTY, ..Setup::empty() };
    Chess::from_setup(setup, CastlingMode::Standard)
        .map(|chess| chess.is_checkmate())
        .unwrap_or(false)
}

/// The squares a king could move to plus its own square — "the box he's
/// being put in by the opponent." Pure board geometry, relocated here from
/// `position.rs` (where `king_safety_score`/`piece_activity_score` still use
/// it as a mobility mask) since it's a zone *definition* that belongs next
/// to the primitive (`zone_control`) that reads zones, not with the legacy
/// scoring functions that happen to consume it.
pub fn king_ring(board: &Board, color: Color) -> Bitboard {
    let Some(king_sq) = board.king_of(color) else {
        return Bitboard::EMPTY;
    };
    attacks::king_attacks(king_sq) | Bitboard::from(king_sq)
}

/// Complete attack adjacency for a position.
#[derive(Debug, Clone)]
pub struct ThreatGraph {
    /// Square → pieces this square attacks
    pub attacks_from: [Bitboard; 64],
    /// Square → pieces attacking this square
    pub attackers_to: [Bitboard; 64],
    /// Piece on each square
    pub pieces: [Option<Piece>; 64],
    /// Side to move
    pub turn: Color,
    /// King squares: (white_king, black_king)
    pub kings: (Option<Square>, Option<Square>),
    /// Full board for SEE context
    pub board: Board,
}

impl ThreatGraph {
    /// Build the attack graph from a shakmaty Chess position.
    pub fn build(chess: &Chess) -> Self {
        Self::build_from_board(chess.board().clone(), chess.turn())
    }

    /// Build the attack graph from a bare `Board` + whose turn it notionally
    /// is — everything this needs (`attacks_from`, `attacks_to`, `king_of`)
    /// is pure board geometry, none of it depends on a legal `Chess`
    /// position (castling rights, move history, check status). This is what
    /// lets `collapse_king_zone_swing` build a graph on a *hypothetical*
    /// board — one piece captured, the capturer moved onto its square, both
    /// via direct `Board` surgery — without that board ever needing to be a
    /// legally reachable position.
    pub fn build_from_board(board: Board, turn: Color) -> Self {
        let mut attacks_from = [Bitboard::EMPTY; 64];
        let mut attackers_to = [Bitboard::EMPTY; 64];
        let mut pieces: [Option<Piece>; 64] = [None; 64];

        let occupied = board.occupied();

        for sq in Square::ALL {
            let sq_idx = u32::from(sq) as usize;
            pieces[sq_idx] = board.piece_at(sq);
            if pieces[sq_idx].is_some() {
                attacks_from[sq_idx] = board.attacks_from(sq);
            }
            // attacks_to needs a color (which side's attacks we want)
            attackers_to[sq_idx] = board.attacks_to(sq, Color::White, occupied)
                                 | board.attacks_to(sq, Color::Black, occupied);
        }

        let kings = (board.king_of(Color::White), board.king_of(Color::Black));
        ThreatGraph { attacks_from, attackers_to, pieces, turn, kings, board }
    }

    /// All pieces of the given color, with their squares.
    fn pieces_of(&self, color: Color) -> Vec<(Square, Piece)> {
        let mut out = Vec::new();
        for sq in Square::ALL {
            let idx = u32::from(sq) as usize;
            if let Some(p) = self.pieces[idx] {
                if p.color == color {
                    out.push((sq, p));
                }
            }
        }
        out
    }

    /// Square index helper.
    fn idx(sq: Square) -> usize { u32::from(sq) as usize }

    /// Net control of `sq`: how many more of `color`'s pieces attack it than
    /// the opponent's — built entirely from `attackers_to`, already computed
    /// once for the whole board. Positive means `color` controls this square
    /// more than the opponent does; negative means the reverse. This is the
    /// shared "whose continuity is this square in" primitive — a piece is
    /// hanging exactly when it sits on a square where its own color's control
    /// is negative (occupied by a piece that can't be defended in kind).
    pub fn control(&self, sq: Square, color: Color) -> i32 {
        let idx = Self::idx(sq);
        let ours = (self.attackers_to[idx] & self.board.by_color(color)).count() as i32;
        let theirs = (self.attackers_to[idx] & self.board.by_color(color.other())).count() as i32;
        ours - theirs
    }

    /// `attackers_to[sq]`, masked to one color — the piece-level view of the
    /// same continuity primitive `control` summarizes as a differential.
    pub fn attackers(&self, sq: Square, color: Color) -> Bitboard {
        self.attackers_to[Self::idx(sq)] & self.board.by_color(color)
    }

    /// `attacks_from[sq]` — a named accessor so external callers don't need
    /// `idx` (private). Only meaningful for an occupied square; empty
    /// otherwise, matching `board.attacks_from`.
    pub fn attacks_from(&self, sq: Square) -> Bitboard {
        self.attacks_from[Self::idx(sq)]
    }

    /// Which piece(s) are giving check to `color`'s king — the identity
    /// behind `is_in_check`, same "read it off the graph already built,
    /// don't make a second shakmaty call" reasoning, generalized from a
    /// yes/no to naming names. Empty if not in check, or if `color` has no
    /// king on the board at all (`collapse_criticality` builds exactly
    /// these — a king that was part of the cleared cluster and isn't the
    /// candidate being tested).
    pub fn checkers(&self, color: Color) -> Vec<PieceRef> {
        let king_sq = if color.is_white() { self.kings.0 } else { self.kings.1 };
        let Some(king_sq) = king_sq else { return Vec::new() };
        self.attackers(king_sq, color.other()).into_iter().filter_map(|sq| {
            self.pieces[Self::idx(sq)].map(|p| PieceRef {
                role: role_name(p.role), color: Side::from(p.color), square: sq.to_string(),
            })
        }).collect()
    }

    /// Whether `color`'s king is currently attacked — mathematically the same
    /// fact shakmaty's own `Position::checkers().any()` computes
    /// (`king_attackers` there is just `board().attacks_to(...)`, the same
    /// primitive `attackers_to` is built from — see FINDINGS.md), read here from
    /// the graph already built for this position instead of a second,
    /// separate shakmaty call. Defined in terms of `checkers` — one
    /// computation, not two that happen to agree.
    pub fn is_in_check(&self, color: Color) -> bool {
        !self.checkers(color).is_empty()
    }

    /// `control`, summed over every square in `zone` — the zone-level
    /// generalization of the same continuity primitive, for questions like
    /// "who controls the king ring" rather than "who controls this one
    /// square." Not yet used anywhere: king_safety_score currently checks
    /// only the king's own square (its own `board.attacks_to` call, not this
    /// graph), and piece_activity_score's `king_ring` usage is a per-piece
    /// existence check ("does this piece's attack reach the ring"), not a
    /// control sum — see FINDINGS.md for why neither was migrated onto this in
    /// the same pass it was added.
    pub fn zone_control(&self, zone: Bitboard, color: Color) -> i32 {
        zone.into_iter().map(|sq| self.control(sq, color)).sum()
    }

    /// Value of a piece for SEE ordering.
    fn piece_value(role: Role) -> i64 {
        match role {
            Role::Pawn => 100, Role::Knight => 320, Role::Bishop => 330,
            Role::Rook => 500, Role::Queen => 900, Role::King => 20000,
        }
    }

    /// After a capture on `sq` by `side`, check if this delivers check.
    fn delivers_check(&self, board: &Board, cap_sq: Square, side: Color) -> bool {
        let opp_king = if side.is_white() { self.kings.1 } else { self.kings.0 };
        let Some(king_sq) = opp_king else { return false };
        let occupied = board.occupied();
        // Direct: the capture square attacks the king (piece now sits there)
        if board.attacks_from(cap_sq) & Bitboard::from(king_sq) != Bitboard::EMPTY { return true; }
        // Discovered: any piece of `side` attacks the king (victim was blocking)
        board.attacks_to(king_sq, side, occupied) != Bitboard::EMPTY
    }

    /// Run SEE: mutable board clone for x-ray discovery + check interrupts.
    /// A check interrupt cancels the recapture chain (opponent must answer check).
    pub fn see_chain(&self, sq: Square, initiator: Color) -> (Vec<CaptureStep>, i64) {
        let mut steps = Vec::new();
        let mut board = self.board.clone();
        let victim_val = self.pieces[Self::idx(sq)]
            .map(|p| Self::piece_value(p.role)).unwrap_or(0);
        let mut net = victim_val;

        let victim_color = Side::from(initiator.other());
        let victim_role = self.pieces[Self::idx(sq)]
            .map(|p| role_name(p.role)).unwrap_or_default();
        steps.push(CaptureStep {
            piece: victim_role, color: victim_color,
            square: sq.to_string(), value_cp: victim_val,
        });
        board.discard_piece_at(sq);

        // Check interrupt: if initiator's capture delivers check, chain ends
        if self.delivers_check(&board, sq, initiator) {
            return (steps, net);
        }

        let mut att_sq = sq;
        let mut side_to_capture = initiator.other();

        loop {
            let occupied = board.occupied();
            let attackers = board.attacks_to(att_sq, side_to_capture, occupied);
            if attackers == Bitboard::EMPTY { break; }

            let mut best_sq = None;
            let mut best_role = Role::Pawn;
            for role in [Role::Pawn, Role::Knight, Role::Bishop, Role::Rook, Role::Queen, Role::King] {
                if let Some(recap_sq) = (attackers & board.by_role(role)).into_iter().next() {
                    best_sq = Some(recap_sq); best_role = role; break;
                }
            }
            if let Some(recap_sq) = best_sq {
                let val = Self::piece_value(best_role);
                let delta = val * if side_to_capture == initiator { 1 } else { -1 };
                net += delta;
                steps.push(CaptureStep {
                    piece: role_name(best_role),
                    color: Side::from(side_to_capture),
                    square: recap_sq.to_string(), value_cp: val,
                });
                board.discard_piece_at(recap_sq);

                // Check interrupt after recapture too
                if self.delivers_check(&board, recap_sq, side_to_capture) {
                    return (steps, net);
                }
                att_sq = recap_sq;
                side_to_capture = side_to_capture.other();
            } else { break; }
        }
        (steps, net)
    }

    /// Convenience: SEE net score only.
    pub fn see(&self, sq: Square, initiator: Color) -> i64 {
        self.see_chain(sq, initiator).1
    }

    /// Bucket a SEE net score (always initiator/attacker-perspective, same
    /// convention as `see`/`see_chain`) into a plain verdict. One threshold
    /// set shared by every concept that reports a `Consequence`, so
    /// "winning" means the same centipawn range everywhere in this file.
    fn consequence_of(see_cp: i64) -> Consequence {
        if see_cp > 150 { Consequence::Winning }
        else if see_cp > 0 { Consequence::Minor }
        else if see_cp < -50 { Consequence::Losing }
        else { Consequence::Even }
    }

    /// Find all forks: a piece attacks ≥2 enemy pieces.
    pub fn find_forks(&self, color: Color) -> Vec<EvaluatedFork> {
        let mut out = Vec::new();
        let enemy = color.other();
        for (sq, piece) in self.pieces_of(color) {
            let attacks = self.attacks_from[Self::idx(sq)];
            let attacked = attacks & self.board.by_color(enemy);
            if attacked.count() < 2 { continue; }

            let mut targets: Vec<PieceRef> = Vec::new();
            let mut total_val = 0i64;
            for t_sq in attacked {
                if let Some(tp) = self.pieces[Self::idx(t_sq)] {
                    let val = Self::piece_value(tp.role);
                    total_val += val;
                    targets.push(PieceRef {
                        role: role_name(tp.role),
                        color: Side::from(enemy),
                        square: t_sq.to_string(),
                    });
                }
            }
            if targets.len() >= 2 && total_val >= Self::piece_value(Role::Rook) {
                let attacker = PieceRef {
                    role: role_name(piece.role),
                    color: Side::from(color),
                    square: sq.to_string(),
                };
                // Which target is this fork's real point: the one that gives
                // the attacker the best SEE outcome, not just whichever
                // target happens to have zero defenders. A rook defended
                // once by a pawn is still very much worth capturing with a
                // knight (net +180) — restricting this to strictly-
                // undefended targets silently reported a fork's true
                // material swing as "Even" whenever every target happened to
                // have exactly one defender (see FINDINGS.md's Fruit-game
                // entry: this is exactly what happened to the Ne5 fork on a
                // rook and a queen that were each individually defended).
                let (hangs, chain, see_gain) = match self.best_fork_target(&targets, color) {
                    Some((piece, chain, gain)) => (Some(piece), chain, gain),
                    None => (None, Vec::new(), 0),
                };
                let consequence = Self::consequence_of(see_gain);

                out.push(EvaluatedFork {
                    attacker,
                    targets,
                    hangs,
                    see_cp: see_gain,
                    consequence,
                    chain,
                });
            }
        }
        out
    }

    /// Among fork targets, find the one that gives the attacking side
    /// (`attacker`) the best real SEE outcome — full static-exchange
    /// evaluation per target, not a defender-count shortcut. Zero-defender
    /// targets are still very much included (SEE on an undefended piece just
    /// returns its full value, same as before), but so are defended targets
    /// where capturing is still net-favorable once the recapture is played
    /// out.
    fn best_fork_target(&self, targets: &[PieceRef], attacker: Color) -> Option<(PieceRef, Vec<CaptureStep>, i64)> {
        let mut best: Option<(PieceRef, Vec<CaptureStep>, i64)> = None;
        for t in targets {
            let t_sq = match shakmaty::Square::from_ascii(t.square.as_bytes()) {
                Ok(sq) => sq, Err(_) => continue,
            };
            let (chain, see_gain) = self.see_chain(t_sq, attacker);
            match &best {
                None => best = Some((t.clone(), chain, see_gain)),
                Some((_, _, existing)) if see_gain > *existing => best = Some((t.clone(), chain, see_gain)),
                _ => {}
            }
        }
        best
    }

    /// Raw hanging-piece scan, `safe_to_capture` left as a placeholder —
    /// `collapse_criticality`'s own baseline computation calls this instead
    /// of `find_hanging`, since `find_hanging` calls `collapse_criticality`
    /// per candidate to fill that field in; going through the public
    /// `find_hanging` here would recurse forever.
    ///
    /// Kings excluded: a king attacked with no piece able to interpose or
    /// capture the attacker isn't "hanging" (a piece a player could simply
    /// take for free) — it's check, a different fact already reported
    /// explicitly (`is_in_check`/`checkers`) and more clearly (kings can't
    /// actually be captured; the game ends at checkmate first). Without
    /// this, a checked king reads as a "hanging piece worth 20000
    /// centipawns" — technically consistent with the raw attacker/defender
    /// count, but a confusing duplicate of a fact stated better elsewhere.
    fn find_hanging_raw(&self) -> Vec<HangingPiece> {
        let mut out = Vec::new();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
            if piece.role == Role::King { continue; }
            let attacker_count = (self.attackers_to[idx]
                & self.board.by_color(piece.color.other())).count();
            if attacker_count == 0 { continue; }
            let defenders = self.attackers_to[idx]
                & self.board.by_color(piece.color)
                & !Bitboard::from(sq);
            if defenders == Bitboard::EMPTY {
                out.push(HangingPiece {
                    piece: PieceRef {
                        role: role_name(piece.role),
                        color: Side::from(piece.color),
                        square: sq.to_string(),
                    },
                    attacker_count: attacker_count as u8,
                    value: Self::piece_value(piece.role),
                    safe_to_capture: true,
                });
            }
        }
        out
    }

    /// Find hanging pieces: attacked with 0 defenders — and, per candidate,
    /// whether any attacker could actually capture here without exposing
    /// its own king (`collapse_criticality`). "Brilliant move looks like a
    /// hanging piece" (user's phrase): a raw zero-defender count says this
    /// piece is lost, but if *every* attacker that could take it would walk
    /// into check, nobody safely can — `safe_to_capture: false` is that
    /// case, checked here so `extract_concepts` doesn't need graph access
    /// itself to act on it.
    pub fn find_hanging(&self) -> Vec<HangingPiece> {
        self.find_hanging_raw().into_iter().map(|mut h| {
            let Ok(sq) = Square::from_ascii(h.piece.square.as_bytes()) else { return h };
            let attacker_color = h.piece.color.other();
            h.safe_to_capture = self.collapse_criticality(sq).iter()
                .any(|r| r.piece.color == attacker_color && !r.own_king_in_check);
            h
        }).collect()
    }

    /// Find outnumbered pieces: real defenders exist (unlike `find_hanging`),
    /// but attackers still outnumber them. The most direct application of
    /// `control` to piece safety — deliberately checked before the fancier
    /// cross-references (`find_overloaded`, `find_false_defense`) even
    /// though it was built after them; see FINDINGS.md.
    ///
    /// Kings excluded, same reasoning as `find_hanging_raw`: a king in a
    /// double check, with a friendly piece that geometrically covers its
    /// square, would otherwise read as "outnumbered" — a confusing restated
    /// version of check/double-check, not a real material-safety fact.
    pub fn find_outnumbered(&self) -> Vec<Outnumbered> {
        let mut out = Vec::new();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
            if piece.role == Role::King { continue; }
            let attacker_count = (self.attackers_to[idx]
                & self.board.by_color(piece.color.other())).count();
            if attacker_count == 0 { continue; }
            let defender_count = (self.attackers_to[idx]
                & self.board.by_color(piece.color)
                & !Bitboard::from(sq)).count();
            if defender_count == 0 { continue; } // find_hanging's job, not this one
            if attacker_count > defender_count {
                // First-exchange-only pricing (victim minus the mover's
                // cheapest attacker), same direct-subtraction approach
                // `find_mover_favored` uses — deliberately NOT `self.see()`.
                // A live game caught this the hard way: a real 2-attacker/
                // 1-defender knight got labeled `consequence: Losing` (i.e.
                // "safe") by the old `see()`-backed computation here, while
                // the actual board showed the cheap attacker simply wins the
                // knight outright. `see()`/`see_chain` has a known sign-flip
                // bug from phantom captures on squares emptied earlier in
                // its own walk (see `find_mover_favored`'s doc comment,
                // FINDINGS.md's 2026-09-01 entries) — real defenders existing
                // here doesn't make that bug go away, so this detector must
                // not depend on it either.
                let attackers = self.attackers_to[idx] & self.board.by_color(piece.color.other());
                let cheapest_val = attackers.into_iter()
                    .filter_map(|a_sq| self.pieces[Self::idx(a_sq)])
                    .map(|p| Self::piece_value(p.role))
                    .min()
                    .unwrap_or(0);
                let see_cp = Self::piece_value(piece.role) - cheapest_val;
                out.push(Outnumbered {
                    piece: PieceRef {
                        role: role_name(piece.role),
                        color: Side::from(piece.color),
                        square: sq.to_string(),
                    },
                    attacker_count: attacker_count as u8,
                    defender_count: defender_count as u8,
                    value: Self::piece_value(piece.role),
                    see_cp,
                    consequence: Self::consequence_of(see_cp),
                });
            }
        }
        out
    }

    /// Find pieces where the raw attacker/defender count looks safe (real
    /// defenders exist — outside `find_hanging`'s territory — and attackers
    /// don't exceed defenders, so `find_outnumbered` doesn't fire either)
    /// but the *first* exchange on that square still favors the mover (the
    /// side that would initiate the capture): the cheapest attacking piece
    /// is worth less than the piece it's attacking.
    ///
    /// Computed *directly* — `victim value − cheapest attacker's own piece
    /// value` — never through `ThreatGraph::see`/`see_chain`. This is
    /// deliberately a **first-exchange-only** check: it only asks "if the
    /// opponent's cheapest attacker captures here and I recapture with my
    /// best response, was that first trade good for them" — not "what
    /// happens if the whole square gets fought over to the end" (that's
    /// exactly the question `see_chain` tries and fails to answer
    /// correctly, see below). Answering the first-exchange question this
    /// way doesn't actually require knowing the *defender* count at all
    /// beyond "at least one" (there has to be a real recapture for the
    /// formula's second half to mean anything) — a queen attacked by a
    /// pawn is a bad trade for the queen's side whether it has one defender
    /// or five, because the opponent only ever risks their cheapest piece
    /// to win it. This is a real generalization from an earlier, narrower
    /// version of this detector that required *exactly* one attacker and
    /// one defender — found too narrow in a live game where a queen with
    /// **two** defenders (a king and a knight, one of which was missed by
    /// eye at the board) was still lost outright to a single bishop, for
    /// exactly this reason (`FINDINGS.md`, 2026-09-01).
    ///
    /// **What "attacker"/"defender" access means here**: immediate
    /// geometric access to the square on the *current* board — the same
    /// snapshot `attackers_to` gives every other detector in this file —
    /// not a simulation of the capture actually being played out. It does
    /// not account for a piece that would only gain a line to the square
    /// once pieces in front of it move (an x-ray attacker/defender). That's
    /// a real, known scope limit, not an oversight.
    ///
    /// Why direct subtraction instead of the general chain-walker:
    /// `see_chain` was tried on exactly this shape of position (one real
    /// attacker, one real defender, nothing more) and found to give the
    /// **wrong sign**: it never actually places a capturing piece back on
    /// the contested square (only ever discards, never sets), so once both
    /// the original piece and the recapturing piece are gone from the
    /// board, any other piece that now happens to have a clear line to the
    /// (empty) square gets treated as a further, real attacker — even
    /// though there's nothing left there to capture. See `FINDINGS.md`'s
    /// 2026-09-01 entry for the exact reproduction (a bare queen-defended
    /// pawn attacked by an enemy queen down an open file: `see_chain`
    /// reports the trade as *favorable* for the attacker who is actually
    /// about to lose their queen for a pawn). Deeper questions — what a
    /// *second* attacker changes, whether a long chain is worth playing out
    /// to the end — are not attempted here at all until `see_chain` itself
    /// is fixed; reporting nothing for those is more honest than reporting
    /// a number already shown to have the wrong sign.
    pub fn find_mover_favored(&self) -> Vec<MoverFavored> {
        let mut out = Vec::new();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
            if piece.role == Role::King { continue; }
            let attackers = self.attackers_to[idx] & self.board.by_color(piece.color.other());
            let attacker_count = attackers.count();
            if attacker_count == 0 { continue; } // find_hanging's job, not this one
            let defenders = self.attackers_to[idx]
                & self.board.by_color(piece.color)
                & !Bitboard::from(sq);
            let defender_count = defenders.count();
            if defender_count == 0 { continue; } // find_hanging's job, not this one
            if attacker_count > defender_count { continue; } // find_outnumbered's job, not this one

            // Cheapest attacker: the opponent's rational first choice, and
            // the only one this first-exchange check needs to know about.
            let Some(cheapest_val) = attackers.into_iter()
                .filter_map(|a_sq| self.pieces[Self::idx(a_sq)])
                .map(|p| Self::piece_value(p.role))
                .min()
            else { continue };

            let victim_val = Self::piece_value(piece.role);
            let see_cp = victim_val - cheapest_val;
            match Self::consequence_of(see_cp) {
                Consequence::Winning | Consequence::Minor => {
                    out.push(MoverFavored {
                        piece: PieceRef {
                            role: role_name(piece.role),
                            color: Side::from(piece.color),
                            square: sq.to_string(),
                        },
                        attacker_count: attacker_count as u8,
                        defender_count: defender_count as u8,
                        see_cp,
                        consequence: Self::consequence_of(see_cp),
                    });
                }
                Consequence::Even | Consequence::Losing => {}
            }
        }
        out
    }

    /// Find overloaded pieces: the mirror image of a fork. One piece
    /// attacking 2+ enemy targets is a fork; one piece being the *sole*
    /// defender of 2+ of its own side's currently-attacked pieces is an
    /// overload. Pure `attackers_to` lookup — no search, no cross-reference
    /// with any other detector needed (contrast the pin/false-defense case,
    /// which does need one — see FINDINGS.md).
    ///
    /// Targets exclude the king, same reasoning as `find_hanging_raw`: being
    /// the sole piece "defending" a check isn't the same fact as being the
    /// sole defender of a capturable piece, and mixing the king's 20000
    /// piece-value into `critical_value` produces a number that isn't
    /// really comparable to a genuine material stakes figure.
    pub fn find_overloaded(&self, color: Color) -> Vec<Overloaded> {
        let mut out = Vec::new();
        let enemy = color.other();
        for (sq, piece) in self.pieces_of(color) {
            let mut critical_for: Vec<PieceRef> = Vec::new();
            let mut critical_value = 0i64;
            for (t_sq, t_piece) in self.pieces_of(color) {
                if t_sq == sq { continue; }
                if t_piece.role == Role::King { continue; }
                let t_idx = Self::idx(t_sq);
                let attacker_count = (self.attackers_to[t_idx] & self.board.by_color(enemy)).count();
                if attacker_count == 0 { continue; }
                let defenders = self.attackers_to[t_idx] & self.board.by_color(color);
                let is_sole_defender = defenders.count() == 1 && (defenders & Bitboard::from(sq)).any();
                if is_sole_defender {
                    critical_value += Self::piece_value(t_piece.role);
                    critical_for.push(PieceRef {
                        role: role_name(t_piece.role),
                        color: Side::from(color),
                        square: t_sq.to_string(),
                    });
                }
            }
            if critical_for.len() >= 2 {
                out.push(Overloaded {
                    piece: PieceRef {
                        role: role_name(piece.role),
                        color: Side::from(color),
                        square: sq.to_string(),
                    },
                    critical_for,
                    critical_value,
                });
            }
        }
        out
    }

    /// Find pieces whose raw defenders (nonzero, so `find_hanging` passes
    /// them) are all pinned *and* unable to legally recapture here, because
    /// this square isn't on their own pin's attacker–king line. Needs the
    /// already-detected `pins` as input — `ThreatGraph` doesn't detect pins
    /// itself (see this file's own module doc) — so this is a genuine
    /// cross-reference between two independently-computed facts, not a
    /// self-contained graph query the way `find_hanging`/`find_overloaded`
    /// are. Still no search, no exchange priced: just checking whether an
    /// already-known line covers an already-known square.
    pub fn find_false_defense(&self, color: Color, pins: &[Pin]) -> Vec<FalseDefense> {
        let mut out = Vec::new();
        let enemy = color.other();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
            if piece.color != color { continue; }
            if piece.role == Role::King { continue; } // check, not a "falsely defended" material fact
            let attacker_count = (self.attackers_to[idx] & self.board.by_color(enemy)).count();
            if attacker_count == 0 { continue; }
            let defenders = self.attackers_to[idx] & self.board.by_color(color) & !Bitboard::from(sq);
            if defenders == Bitboard::EMPTY { continue; } // find_hanging's job, not this one

            let mut pinned_defenders: Vec<PieceRef> = Vec::new();
            let mut all_ineffective = true;
            for d_sq in defenders {
                let Some(pin) = pins.iter().find(|p| p.pinned.square == d_sq.to_string() && p.pinned.color == Side::from(color)) else {
                    all_ineffective = false; break; // a genuinely unpinned defender — real defense exists
                };
                let (Ok(attacker_sq), Ok(king_sq)) = (
                    Square::from_ascii(pin.attacker.square.as_bytes()),
                    Square::from_ascii(pin.shielded.square.as_bytes()),
                ) else { all_ineffective = false; break; };
                // Legal squares for a pinned piece are strictly between the
                // attacker and its own king (plus capturing the attacker
                // itself) — NOT the full infinite ray::ray() line, which
                // extends past both endpoints. Moving to a square beyond the
                // king, on the far side from the attacker, would expose the
                // king exactly as much as moving off the line entirely.
                let legal_pin_squares = attacks::between(attacker_sq, king_sq) | Bitboard::from(attacker_sq);
                if (legal_pin_squares & Bitboard::from(sq)).any() {
                    all_ineffective = false; break; // sq is on the pin line — this defender can legally recapture
                }
                if let Some(d_piece) = self.pieces[Self::idx(d_sq)] {
                    pinned_defenders.push(PieceRef {
                        role: role_name(d_piece.role),
                        color: Side::from(color),
                        square: d_sq.to_string(),
                    });
                }
            }
            if all_ineffective && !pinned_defenders.is_empty() {
                out.push(FalseDefense {
                    piece: PieceRef { role: role_name(piece.role), color: Side::from(color), square: sq.to_string() },
                    attacker_count: attacker_count as u8,
                    pinned_defenders,
                    value: Self::piece_value(piece.role),
                });
            }
        }
        out
    }

    /// Clear the whole local contest at `sq`, then test each candidate
    /// individually against that clean board — not move-by-move simulation,
    /// no capture order, no recapture choice. For every piece currently
    /// attacking, defending, or occupying `sq`: discard the *entire*
    /// cluster first, then place just this one candidate back onto `sq`,
    /// rebuild the graph, and read what that final configuration looks
    /// like — as if this piece were the one left standing once everyone
    /// else contesting the square had traded off, regardless of what order
    /// that happened in. This identifies false defenders directly: a piece
    /// whose own king ends up in check (or checkmate) once *it's* the one
    /// occupying `sq` cannot actually go there safely, no matter what the
    /// raw attacker/defender count said.
    ///
    /// This is the general mechanism `find_overloaded`/`find_false_defense`
    /// are each one special case of: a pin is exactly "placing this piece
    /// here leaves its own king in check"; an overload is "placing this
    /// piece here swings control of some *other* square sharply." One
    /// clear-and-place operation, not two separately-built detectors. Feeds
    /// `HangingPiece.safe_to_capture` (wired into `extract_concepts`);
    /// otherwise still experimental.
    pub fn collapse_criticality(&self, sq: Square) -> Vec<PieceCriticality> {
        let idx = Self::idx(sq);
        let mut cluster: Vec<Square> = self.attackers_to[idx].into_iter().collect();
        if self.pieces[idx].is_some() && !cluster.contains(&sq) { cluster.push(sq); }
        if cluster.is_empty() { return Vec::new(); }

        let mut clean_slate = self.board.clone();
        for &p_sq in &cluster { clean_slate.discard_piece_at(p_sq); }

        let white_zone_before = self.zone_control(king_ring(&self.board, Color::White), Color::White);
        let black_zone_before = self.zone_control(king_ring(&self.board, Color::Black), Color::Black);
        // Baseline for "more pieces threatened": whatever's already hanging
        // in the real position doesn't count as a new consequence of this
        // particular collapse.
        let baseline_hanging = self.find_hanging_raw();

        cluster.iter().filter_map(|&p_sq| {
            let piece = self.pieces[Self::idx(p_sq)]?;
            let mut hypo = clean_slate.clone();
            hypo.set_piece_at(sq, piece);
            let graph = ThreatGraph::build_from_board(hypo.clone(), self.turn);

            let square_control_delta = graph.control(sq, piece.color) - self.control(sq, piece.color);
            let white_zone_after = graph.zone_control(king_ring(&graph.board, Color::White), Color::White);
            let black_zone_after = graph.zone_control(king_ring(&graph.board, Color::Black), Color::Black);

            // Check and its identity come straight off the graph, no
            // shakmaty round-trip needed (ThreatGraph::checkers). Only
            // checkmate genuinely needs shakmaty's legal move generation.
            let own_king_checked_by = graph.checkers(piece.color);
            let delivers_check_via = graph.checkers(piece.color.other());
            let own_king_checkmated = is_checkmate_via_shakmaty(&hypo, piece.color);
            let delivers_checkmate = is_checkmate_via_shakmaty(&hypo, piece.color.other());

            // Reuse find_hanging (already-built, already-tested) on the
            // hypothetical itself instead of a bespoke "is this piece now
            // undefended" check — anything it reports that wasn't already
            // hanging in the real position is a fresh consequence of this
            // specific candidate ending up on `sq`. find_hanging_raw, not
            // find_hanging: only the piece list is used below, not
            // safe_to_capture, so there's no reason to pay for (and recurse
            // one level deeper into) the full collapse_criticality
            // enrichment on this hypothetical too. No king filter needed
            // here any more — find_hanging_raw itself excludes kings now.
            let newly_hanging: Vec<PieceRef> = graph.find_hanging_raw().into_iter()
                .filter(|h| !baseline_hanging.iter().any(|b| b.piece.square == h.piece.square && b.piece.color == h.piece.color))
                .map(|h| h.piece)
                .collect();

            Some(PieceCriticality {
                piece: PieceRef { role: role_name(piece.role), color: Side::from(piece.color), square: sq.to_string() },
                square_control_delta,
                white_king_zone_delta: white_zone_after - white_zone_before,
                black_king_zone_delta: black_zone_after - black_zone_before,
                own_king_in_check: !own_king_checked_by.is_empty(),
                own_king_checked_by,
                own_king_checkmated,
                delivers_check: !delivers_check_via.is_empty(),
                delivers_check_via,
                delivers_checkmate,
                newly_hanging,
            })
        }).collect()
    }

    /// Find pieces where the raw count looks safe
    /// (`raw_defender_count >= attacker_count`, so neither `find_hanging` nor
    /// `find_outnumbered` flag them) but isn't once defenders already spoken
    /// for elsewhere are discounted: pinned off the recapture line
    /// (`find_false_defense`'s per-defender fact, generalized here to a
    /// partial count instead of requiring *every* defender compromised) or
    /// the sole defender of another piece (`find_overloaded`'s fact). This is
    /// the rung above both: each already reports its own fact standalone (a
    /// specific pin, a specific overload); this asks whether combining them
    /// with the raw count changes the verdict a bare-count read would give.
    /// Needs both `pins` and `overloaded` as input — a genuine
    /// cross-reference of two already-computed facts, same shape as
    /// `find_false_defense`, just widened to partial discounting instead of
    /// all-or-nothing.
    pub fn find_false_safety(&self, color: Color, pins: &[Pin], overloaded: &[Overloaded]) -> Vec<FalseSafety> {
        let mut out = Vec::new();
        let enemy = color.other();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
            if piece.color != color { continue; }
            if piece.role == Role::King { continue; } // check, not a material safety fact
            let attacker_count = (self.attackers_to[idx] & self.board.by_color(enemy)).count();
            if attacker_count == 0 { continue; }
            let defenders = self.attackers_to[idx] & self.board.by_color(color) & !Bitboard::from(sq);
            let raw_defender_count = defenders.count();
            // find_hanging's job (0 defenders) or find_outnumbered's job
            // (already outnumbered on the raw count) — this rung only fires
            // when the raw count alone would have said "safe."
            if raw_defender_count == 0 || attacker_count > raw_defender_count { continue; }

            let mut compromised: Vec<PieceRef> = Vec::new();
            for d_sq in defenders {
                let pinned_off_line = pins.iter()
                    .find(|p| p.pinned.square == d_sq.to_string() && p.pinned.color == Side::from(color))
                    .is_some_and(|pin| {
                        let (Ok(attacker_sq), Ok(king_sq)) = (
                            Square::from_ascii(pin.attacker.square.as_bytes()),
                            Square::from_ascii(pin.shielded.square.as_bytes()),
                        ) else { return false; };
                        let legal_pin_squares = attacks::between(attacker_sq, king_sq) | Bitboard::from(attacker_sq);
                        !(legal_pin_squares & Bitboard::from(sq)).any()
                    });
                let is_overloaded = overloaded.iter()
                    .any(|ov| ov.piece.square == d_sq.to_string() && ov.piece.color == Side::from(color));
                if pinned_off_line || is_overloaded {
                    if let Some(d_piece) = self.pieces[Self::idx(d_sq)] {
                        compromised.push(PieceRef { role: role_name(d_piece.role), color: Side::from(color), square: d_sq.to_string() });
                    }
                }
            }
            if compromised.is_empty() { continue; }
            let effective_defender_count = raw_defender_count - compromised.len();
            if effective_defender_count < attacker_count {
                out.push(FalseSafety {
                    piece: PieceRef { role: role_name(piece.role), color: Side::from(color), square: sq.to_string() },
                    attacker_count: attacker_count as u8,
                    raw_defender_count: raw_defender_count as u8,
                    effective_defender_count: effective_defender_count as u8,
                    compromised_defenders: compromised,
                    value: Self::piece_value(piece.role),
                });
            }
        }
        out
    }

    /// Find exchange chains: captures on the same square across consecutive moves.
    /// Returns chains with ≥3 captures (a collapse).
    pub fn find_exchange_chain(&self, sq: Square, initiator: Color) -> Option<ExchangeChain> {
        let mut captures = Vec::new();
        let mut side = initiator;
        let mut current_sq = sq;
        let mut net = 0i64;

        loop {
            let attackers = self.attackers_to[Self::idx(current_sq)]
                & self.board.by_color(side);
            if attackers == Bitboard::EMPTY { break; }

            // Find lowest-value attacker
            let mut best: Option<(Square, Role, i64)> = None;
            for r in [Role::Pawn, Role::Knight, Role::Bishop, Role::Rook, Role::Queen, Role::King] {
                if let Some(s) = (attackers & self.board.by_role(r)).into_iter().next() {
                    let v = Self::piece_value(r);
                    if best.as_ref().is_none_or(|b| v < b.2) {
                        best = Some((s, r, v));
                    }
                }
            }
            if let Some((s, r, v)) = best {
                let delta = if side == initiator { v } else { -v };
                net += delta;
                captures.push(CaptureStep {
                    piece: role_name(r),
                    color: Side::from(side),
                    square: s.to_string(),
                    value_cp: v,
                });
                current_sq = s;
                side = side.other();
            } else { break; }
        }

        if captures.len() >= 3 {
            Some(ExchangeChain {
                square: sq.to_string(),
                steps: captures,
                net_cp: net,
                winner: if net > 0 {
                    (if initiator.is_white() { "white" } else { "black" }).to_string()
                } else if net < 0 {
                    (if initiator.is_white() { "black" } else { "white" }).to_string()
                } else { "even".to_string() },
            })
        } else {
            None
        }
    }
}

// ── Output types ──

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Consequence { Winning, Minor, Losing, Even }

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvaluatedFork {
    pub attacker: PieceRef,
    pub targets: Vec<PieceRef>,
    pub hangs: Option<PieceRef>,
    pub see_cp: i64,
    pub consequence: Consequence,
    /// Optimal SEE recapture chain (for step-by-step comparison)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<CaptureStep>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptureStep {
    pub piece: String,
    pub color: Side,
    pub square: String,
    pub value_cp: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExchangeChain {
    pub square: String,
    pub steps: Vec<CaptureStep>,
    pub net_cp: i64,
    pub winner: String,
}

// ── Name helpers ──

pub fn role_name(r: Role) -> String {
    match r { Role::Pawn => "Pawn", Role::Knight => "Knight", Role::Bishop => "Bishop",
              Role::Rook => "Rook", Role::Queen => "Queen", Role::King => "King" }.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{attacks, fen::Fen, CastlingMode};

    fn pos(fen: &str) -> Chess {
        Fen::from_ascii(fen.as_bytes()).unwrap().into_position(CastlingMode::Standard).unwrap()
    }

    #[test]
    fn control_and_attackers_agree_with_a_direct_recount() {
        // White queen d4, kings only otherwise. Queen attacks the whole
        // d-file and 4th rank plus its diagonals; black king defends its own
        // 8 neighbors. Verify control()/attackers() against an independent
        // recount via board.attacks_to directly, not by reusing the graph's
        // own logic.
        let chess = pos("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let board = chess.board();
        let occ = board.occupied();

        for sq in Square::ALL {
            let expect_white = (board.attacks_to(sq, Color::White, occ) & board.by_color(Color::White)).count() as i32;
            let expect_black = (board.attacks_to(sq, Color::Black, occ) & board.by_color(Color::Black)).count() as i32;
            assert_eq!(graph.attackers(sq, Color::White).count() as i32, expect_white, "attackers(White) mismatch at {sq}");
            assert_eq!(graph.attackers(sq, Color::Black).count() as i32, expect_black, "attackers(Black) mismatch at {sq}");
            assert_eq!(graph.control(sq, Color::White), expect_white - expect_black, "control(White) mismatch at {sq}");
        }
    }

    #[test]
    fn control_is_one_shared_map_not_two_independent_ones() {
        // control(sq, White) == -control(sq, Black) for every square, on
        // every position, by construction (see control's own body: swapping
        // `color` swaps which count is `ours` vs `theirs`). There's one real
        // per-square quantity (white_attackers - black_attackers); querying
        // either color's control is just that one number read with a sign
        // flip, not two separately-computed maps that happen to agree.
        for fen in [
            "4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1",
            "r1bq1rk1/ppp2ppp/2n2n2/2bpp3/4P3/2PP1N2/PP1N1PPP/R1BQKB1R w KQ - 0 7",
            "6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1",
        ] {
            let chess = pos(fen);
            let graph = ThreatGraph::build(&chess);
            for sq in Square::ALL {
                assert_eq!(
                    graph.control(sq, Color::White), -graph.control(sq, Color::Black),
                    "control(White)/control(Black) should be exact negatives at {sq} for {fen}"
                );
            }
        }
    }

    #[test]
    fn zone_control_sums_control_over_every_square_in_the_zone() {
        let chess = pos("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let king_sq = graph.kings.1.unwrap(); // black king, e8
        let zone = attacks::king_attacks(king_sq) | Bitboard::from(king_sq);

        let independent_sum: i32 = zone.into_iter().map(|sq| graph.control(sq, Color::Black)).sum();
        assert_eq!(graph.zone_control(zone, Color::Black), independent_sum);
        // Ring is {d7,d8,e7,e8,f7,f8}. The queen reaches d7/d8 (same file),
        // exactly canceled there by the king's own defense of its neighbors
        // (control 0 on each); e7/f7/f8 are king-defended and queen-unreached
        // (control +1 each); e8 itself is unattacked by anyone (control 0).
        // Net: 0 + 0 + 1 + 0 + 1 + 1 = 3.
        assert_eq!(graph.zone_control(zone, Color::Black), 3);
    }

    #[test]
    fn is_in_check_matches_shakmatys_own_is_check() {
        let in_check = pos("rnb1kbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1qPP/RNBQKB1R w KQkq - 0 1");
        assert!(in_check.is_check());
        assert!(ThreatGraph::build(&in_check).is_in_check(Color::White));

        let not_in_check = pos("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1");
        assert!(!not_in_check.is_check());
        assert!(!ThreatGraph::build(&not_in_check).is_in_check(Color::White));
    }

    // Regression for the corrected collapse experiment: a black knight on
    // h4 blocks White's Rh1 from the h-file, and is the sole attacker of a
    // white pawn on f5. Clear both (pawn + knight), then test each
    // candidate individually against that clean board. If the *pawn* is
    // the one left standing, the knight is simply gone from the position --
    // and the h-file is wide open regardless, so black is already in
    // check. If the *knight* is the one left standing (as if it had
    // captured on f5), its own king is now in check too: the knight cannot
    // actually recapture here without exposing its own king, even though a
    // naive count would just call the pawn "hanging."
    #[test]
    fn collapse_criticality_finds_the_false_defender_via_check() {
        let chess = pos("7k/8/8/5P2/7n/8/8/4K2R w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let f5 = Square::from_ascii(b"f5").unwrap();

        let results = graph.collapse_criticality(f5);
        assert_eq!(results.len(), 2, "cluster is {{pawn f5, knight h4}}, one entry per candidate, got {results:?}");

        let pawn_candidate = results.iter().find(|r| r.piece.role == "Pawn").expect("pawn candidate should be present");
        assert_eq!(pawn_candidate.square_control_delta, 1, "with the knight gone entirely, nothing attacks f5 any more");
        assert_eq!(pawn_candidate.white_king_zone_delta, 0);
        assert_eq!(pawn_candidate.black_king_zone_delta, -2, "the knight's total absence still reopens the h-file");
        assert!(!pawn_candidate.own_king_in_check, "White's own king is untouched by any of this");
        assert!(pawn_candidate.delivers_check, "Rh1 checks Black with the knight simply absent");
        assert_eq!(pawn_candidate.delivers_check_via.len(), 1);
        assert_eq!(pawn_candidate.delivers_check_via[0].square, "h1", "the checking piece should be identified, not just a bare bool");
        assert_eq!(pawn_candidate.delivers_check_via[0].role, "Rook");

        let knight_candidate = results.iter().find(|r| r.piece.role == "Knight").expect("knight candidate should be present");
        assert_eq!(knight_candidate.square_control_delta, -1, "black loses its only attacker of f5 once it's the knight standing there instead of attacking from h4");
        assert_eq!(knight_candidate.white_king_zone_delta, 0, "the knight never touches White's king ring from f5 either");
        assert_eq!(knight_candidate.black_king_zone_delta, -1);
        assert!(knight_candidate.own_king_in_check, "the knight vacating h4 to stand on f5 leaves its own king in check -- a false defender");
        assert_eq!(knight_candidate.own_king_checked_by.len(), 1);
        assert_eq!(knight_candidate.own_king_checked_by[0].square, "h1", "same rook, now checking via the reopened file instead of being blocked");
        assert_eq!(knight_candidate.own_king_checked_by[0].role, "Rook");
        assert!(!knight_candidate.delivers_check, "a knight on f5 doesn't attack e1");
        assert!(knight_candidate.delivers_check_via.is_empty());

        // Neither case is actually checkmate here -- Black's king still has
        // g7/g8 to run to -- so the best-effort checkmate check should
        // agree it's "just" check, not propagate an error or false-positive.
        assert!(!pawn_candidate.own_king_checkmated);
        assert!(!pawn_candidate.delivers_checkmate);
        assert!(!knight_candidate.own_king_checkmated);
        assert!(!knight_candidate.delivers_checkmate);

        // Sanity: attackers()/control() on the real graph agree with what
        // the hand-derived deltas above assume about the starting position.
        assert_eq!(graph.control(f5, Color::Black), 1, "knight is f5's sole attacker before any removal");
        assert!(!graph.is_in_check(Color::Black), "Rh1 is blocked by the knight before any removal");
    }

    // Regression: when a king itself is part of the local cluster (here,
    // Black's own king on h8 defends its knight on g7, attacked by White's
    // queen on g1 -- cluster is {queen g1, king h8, knight g7}), testing
    // any candidate *other than the king* leaves that king missing from the
    // board entirely. Check itself (ThreatGraph::checkers) handles this
    // fine -- king_sq is just None, checkers() returns empty -- but
    // checkmate genuinely needs Chess::from_setup, which must reject a
    // missing king cleanly (reads as "not checkmate") rather than panic.
    #[test]
    fn collapse_criticality_checkmate_check_degrades_gracefully_when_a_king_is_in_the_cluster() {
        let chess = pos("7k/6n1/8/8/8/8/8/6QK w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let g7 = Square::from_ascii(b"g7").unwrap();
        let results = graph.collapse_criticality(g7);
        assert_eq!(results.len(), 3, "cluster is {{queen g1, king h8, knight g7}}, got {results:?}");
        for r in &results {
            // Whichever candidate is tested, at least one other cluster
            // member is simply absent; when that member is a king,
            // from_setup must fail cleanly rather than panic.
            let _ = r.own_king_checkmated;
            let _ = r.delivers_checkmate;
        }
    }

    // Regression for "more pieces threatened, not just the king": Black's
    // rook on g2 is currently defended once (Nh4, a knight move away) and
    // attacked once (Bc6, a clear diagonal) -- 1v1, not hanging in the real
    // position. Nh4 is *also* the pawn f5's sole attacker, so it's part of
    // that square's cluster. In every collapse_criticality(f5) candidate,
    // h4 ends up empty (either the knight is absent entirely, or it's been
    // relocated to f5) -- so g2's rook loses its only defender regardless
    // of which candidate is tested, and becomes newly hanging as a side
    // effect of a collapse happening on a completely different square.
    #[test]
    fn collapse_criticality_surfaces_other_pieces_newly_hanging_as_a_side_effect() {
        let chess = pos("7k/8/2B5/5P2/7n/8/6r1/4K2R w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let f5 = Square::from_ascii(b"f5").unwrap();
        let g2 = Square::from_ascii(b"g2").unwrap();

        assert!(graph.find_hanging().iter().all(|h| h.piece.square != "g2"), "g2's rook has a real defender in the actual position, must not be hanging yet");
        assert_eq!(graph.control(g2, Color::Black), 0, "one attacker (Bc6), one defender (Nh4) -- net zero control");

        let results = graph.collapse_criticality(f5);
        assert_eq!(results.len(), 2, "cluster is {{pawn f5, knight h4}}, got {results:?}");
        for r in &results {
            assert_eq!(r.newly_hanging.len(), 1, "g2's rook should newly hang once h4 is empty, for candidate {:?}", r.piece);
            assert_eq!(r.newly_hanging[0].square, "g2");
            assert_eq!(r.newly_hanging[0].color, Side::Black);
            assert_eq!(r.newly_hanging[0].role, "Rook");
        }
    }

    #[test]
    fn collapse_criticality_is_empty_off_an_unattacked_undefended_square() {
        let chess = pos("4k3/8/8/8/8/8/8/4K2R w - - 0 1");
        let graph = ThreatGraph::build(&chess);
        let a1 = Square::from_ascii(b"a1").unwrap();
        assert!(graph.collapse_criticality(a1).is_empty(), "empty, unattacked square has no cluster to collapse");
    }

    // Regression: a checked king must not read as a "hanging piece" (or as
    // "outnumbered") -- that's check, a different and better-stated fact
    // (is_in_check/checkers), not a material-safety one; kings can't
    // actually be captured. Real position, verified independently via the
    // live nu-plugin apply-uci pipeline (not hand-constructed): the Fried
    // Liver main line through 7.Qf3+ -- Black's king on f7 is in check from
    // White's queen on f3, with no black piece able to interpose or capture
    // it, which is exactly the shape find_hanging's raw attacker/defender
    // count would otherwise flag.
    #[test]
    fn checked_king_is_not_reported_as_hanging_or_outnumbered() {
        let chess = pos("r1bq1b1r/ppp2kpp/2n5/3np3/2B5/5Q2/PPPP1PPP/RNB1K2R b KQ - 1 7");
        let graph = ThreatGraph::build(&chess);

        assert!(graph.is_in_check(Color::Black), "the king really is in check here");
        assert_eq!(graph.checkers(Color::Black)[0].square, "f3", "Qf3 delivers the check");

        assert!(graph.find_hanging().iter().all(|h| h.piece.square != "f7"),
            "the checked king must not appear as a hanging piece, got {:?}", graph.find_hanging());
        assert!(graph.find_outnumbered().iter().all(|o| o.piece.square != "f7"),
            "the checked king must not appear as outnumbered either, got {:?}", graph.find_outnumbered());
    }

    // Regression for the same king-inclusion issue surfacing one rung
    // deeper: real position from the Noah's Ark Trap (Steiner-Capablanca,
    // Budapest 1929), position after 10.Qc6+ -- verified independently via
    // the live nu-plugin apply-uci pipeline, not hand-constructed. Qc6
    // checks the king on e8, which Qd8 and Bf8 both nominally "defend" by
    // the raw count (2 defenders vs 1 attacker -- false_safety's own
    // precondition for firing). Before this fix, find_false_safety and
    // find_overloaded both treated the king like any other capturable
    // target, producing a false_safety entry on e8 (severity = king's
    // 20000 piece value) and an overloaded entry crediting a king-sized
    // "critical_value" to whichever piece nominally guards it. Neither is a
    // real material-safety fact -- it's check, already reported elsewhere.
    #[test]
    fn checked_king_is_excluded_from_overloaded_false_defense_and_false_safety_too() {
        let chess = pos("r2qkbnr/5ppp/p1Qpb3/1pp5/4P3/1B6/PPP2PPP/RNB1K2R b KQkq - 3 10");
        let graph = ThreatGraph::build(&chess);

        assert!(graph.is_in_check(Color::Black), "the king really is in check here");

        let pins: Vec<Pin> = Vec::new();
        assert!(graph.find_false_defense(Color::Black, &pins).iter().all(|f| f.piece.square != "e8"),
            "the checked king must not appear in false_defense either");

        let overloaded = graph.find_overloaded(Color::Black);
        assert!(overloaded.iter().all(|o| !o.critical_for.iter().any(|c| c.square == "e8")),
            "no piece should be credited with 'defending' the king as a critical_for target, got {overloaded:?}");

        let false_safety = graph.find_false_safety(Color::Black, &pins, &overloaded);
        assert!(false_safety.iter().all(|f| f.piece.square != "e8"),
            "the checked king must not appear as a false_safety piece either, got {false_safety:?}");
    }
}



