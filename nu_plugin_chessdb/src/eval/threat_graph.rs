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
//!   `detect_outposts`) and `find_hanging` read directly, instead of each
//!   independently re-deriving the same attack facts via its own
//!   `board.attacks_to`/`attacks_from` call. See PLAN.md's "continuity map"
//!   thread for the full migration history — and which call sites still
//!   don't use it (`detect_skewers`, the legacy `detect_forks`).
//! - **Pricing — `see`/`see_chain`, consumed by `find_forks`.** A genuinely
//!   different question: not "who's here" but "what would this be worth."
//!   Runs an actual capture/recapture simulation. Has a known, unfixed
//!   correctness bug in its multi-step chain math (PLAN.md) — deliberately
//!   not depended on by anything in the geometry layer above, so that bug
//!   doesn't reach `hanging_piece`/`detect_outposts`/`king_safety_score`/etc.
//!   `find_hanging`'s recorded piece `value` happens to equal what `see`
//!   would compute for that specific case (zero defenders means no
//!   recapture to walk), but it's read directly from `piece_value`, not
//!   routed through the buggy chain logic.
//!
//! **Not built from this graph at all**: pins, skewers, and discovered
//! attacks are detected independently in `position.rs` (`detect_pins`,
//! `detect_skewers`, `detect_discovered`) using shakmaty's own
//! occupancy-aware sliding-attack primitives directly — `detect_pins` calls
//! `attacks::rook_attacks`/`bishop_attacks` itself; `detect_skewers` instead
//! hand-rolls a ray walk that doesn't yet reuse them (a known, deferred gap,
//! PLAN.md). Forks *are* graph-derived (`find_forks` below), but through a
//! second, independent `detect_forks` also exists in `position.rs` purely
//! for the legacy scoring engine, with a threshold that can disagree with
//! this one (PLAN.md).

use shakmaty::{Bitboard, Board, Chess, Color, Piece, Position, Role, Square};
use crate::eval::concept_types::*;

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
        let board = chess.board().clone();
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
        let turn = chess.turn();
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

    /// Whether `color`'s king is currently attacked — mathematically the same
    /// fact shakmaty's own `Position::checkers().any()` computes
    /// (`king_attackers` there is just `board().attacks_to(...)`, the same
    /// primitive `attackers_to` is built from — see PLAN.md), read here from
    /// the graph already built for this position instead of a second,
    /// separate shakmaty call.
    pub fn is_in_check(&self, color: Color) -> bool {
        let king_sq = if color.is_white() { self.kings.0 } else { self.kings.1 };
        king_sq.map(|sq| self.attackers(sq, color.other()).any()).unwrap_or(false)
    }

    /// `control`, summed over every square in `zone` — the zone-level
    /// generalization of the same continuity primitive, for questions like
    /// "who controls the king ring" rather than "who controls this one
    /// square." Not yet used anywhere: king_safety_score currently checks
    /// only the king's own square (its own `board.attacks_to` call, not this
    /// graph), and piece_activity_score's `king_ring` usage is a per-piece
    /// existence check ("does this piece's attack reach the ring"), not a
    /// control sum — see PLAN.md for why neither was migrated onto this in
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
                // Find which target hangs (lowest-value undefended)
                let hangs = self.undefended_target(&targets, enemy);
                // SEE: optimal recapture chain + net score
                let (chain, see_gain) = if let Some(ref h) = hangs {
                    let h_sq = match shakmaty::Square::from_ascii(h.square.as_bytes()) {
                        Ok(sq) => sq, Err(_) => continue,
                    };
                    self.see_chain(h_sq, color)
                } else { (Vec::new(), 0) };
                let consequence = if see_gain > 150 { Consequence::Winning }
                    else if see_gain > 0 { Consequence::Minor }
                    else if see_gain < -50 { Consequence::Losing }
                    else { Consequence::Even };

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

    /// Among fork targets, find the lowest-value undefended one.
    fn undefended_target(&self, targets: &[PieceRef], color: Color) -> Option<PieceRef> {
        let mut best: Option<(PieceRef, i64)> = None;
        for t in targets {
            let t_sq = match shakmaty::Square::from_ascii(t.square.as_bytes()) {
                Ok(sq) => sq, Err(_) => continue,
            };
            let defenders = self.attackers_to[Self::idx(t_sq)]
                & self.board.by_color(color)
                & !Bitboard::from(t_sq);
            if defenders == Bitboard::EMPTY {
                let val = match t.role.as_str() {
                    "Queen" => 900, "Rook" => 500, "Bishop" => 330,
                    "Knight" => 320, "Pawn" => 100, _ => 0,
                };
                match best {
                    None => best = Some((t.clone(), val)),
                    Some((_, existing)) if val < existing => best = Some((t.clone(), val)),
                    _ => {}
                }
            }
        }
        best.map(|(p, _)| p)
    }

    /// Find hanging pieces: attacked with 0 defenders.
    pub fn find_hanging(&self) -> Vec<HangingPiece> {
        let mut out = Vec::new();
        for sq in Square::ALL {
            let idx = Self::idx(sq);
            let Some(piece) = self.pieces[idx] else { continue };
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

#[derive(Debug, Clone, serde::Serialize)]
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
}

