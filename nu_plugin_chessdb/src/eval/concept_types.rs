use serde::Serialize;

// Re-exported (not just used) since `Fork`/`Outnumbered` below expose this
// as part of their own public field surface — callers doing the established
// `use crate::eval::concept_types::*;` glob import get it too, without
// needing a second import from `threat_graph` just for the enum.
pub use crate::eval::threat_graph::Consequence;

/// White or black, for every color/side field in this module's typed output
/// structs. Serializes to exactly the same `"white"`/`"black"` strings the
/// fields here used to hold as bare `String`s (`#[serde(rename_all =
/// "lowercase")]`), so this is a pure internal refactor — no JSON/Nu/SQL
/// consumer downstream can tell the difference.
///
/// Deliberately its own type rather than reusing `shakmaty::Color`: that's a
/// foreign type, so this crate can't implement `Serialize` for it directly
/// (orphan rule). `other()` and `From<shakmaty::Color>` are the two things
/// every call site actually needed — see `canonical::unflip_color` (now
/// deleted, superseded by `other()`) and the ~22 duplicated `if
/// color.is_white() { "white" } else { "black" }` conversions this replaced
/// across `position.rs`/`threat_graph.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }
}

impl From<shakmaty::Color> for Side {
    fn from(c: shakmaty::Color) -> Side {
        if c.is_white() { Side::White } else { Side::Black }
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Side::White => "white",
            Side::Black => "black",
        })
    }
}

/// Which side of a position a `Concept`/`GatedIssue` concerns: the side
/// actually to move (`Us`) or their opponent (`Them`) — **never** a real
/// color, and deliberately never converted to one anywhere in this crate.
///
/// This exists because `Concept`/`GatedIssue` used to carry a `Side` field
/// for this (`Side::White` standing in for "the mover," `Side::Black` for
/// "the opponent") — reusing `Side` here worked only because
/// `extract_concepts` runs entirely inside the internal normalized frame
/// (`normalize_to_white_to_move`, `canonical.rs`) where the mover is always
/// literally `White`. It shipped that way, was then quietly un-flipped back
/// to real color for every consumer (`GatedIssue.side = GatedIssue.side.other()`
/// plus a blanket find/replace of the words "White"/"Black" inside already-
/// rendered phrase text, `unflip_phrase`, both now deleted) — a second,
/// fragile flip layer sitting on top of the first, whose only job was to
/// undo a labeling choice this module never needed to make. An audit
/// (FINDINGS.md, 2026-09-01) found this was the single most fragile part of
/// the whole scoring pipeline: any future concept phrase that didn't route
/// its color word through `us_color`/`them_color`, or that legitimately
/// needed the word "white"/"black" for an unrelated reason (e.g. a
/// "light-squared bishop" mention), would have silently corrupted text sent
/// straight into the `chess-coach` LLM prompt (`ai/mod.nu`).
///
/// The fix: never claim a real color for `Concept`/`GatedIssue` in the first
/// place. `Mover::Us`/`Mover::Them` are computed once, directly, and never
/// need correcting — `Us` always means "whoever `side_to_move` says is to
/// move" by definition, in every frame, with no flip required. A caller that
/// wants real White/Black has everything needed to compute it in one line:
/// `if mover == Mover::Us { side_to_move } else { side_to_move.other() }` —
/// this is the "let the client keep track of which color is the mover"
/// convention already used by `Fork`/`Outnumbered`/`MoverFavored`'s
/// `see_cp`/`consequence` (no color field at all — the client derives mover
/// from `piece.color`), extended to the two structs that couldn't avoid
/// carrying an explicit field because they aren't anchored to one piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mover {
    Us,
    Them,
}

impl std::fmt::Display for Mover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Mover::Us => "the mover",
            Mover::Them => "the opponent",
        })
    }
}

/// Reference to a piece on the board — human-readable, no bitboards.
#[derive(Debug, Clone, Serialize)]
pub struct PieceRef {
    pub role: String,     // "Knight", "Bishop", "Rook", "Queen", "Pawn", "King"
    pub color: Side,
    pub square: String,   // "d5", "e4", "a1"
}

impl PieceRef {
    pub fn notation(&self) -> String {
        let role_char = match self.role.as_str() {
            "Knight" => "N", "Bishop" => "B", "Rook" => "R",
            "Queen" => "Q", "King" => "K", _ => "",
        };
        format!("{}{}", role_char, self.square)
    }
}

// ── Tactical concepts ──

#[derive(Debug, Clone, Serialize)]
pub struct Fork {
    pub attacker: PieceRef,
    pub targets: Vec<PieceRef>,
    /// Which target this fork's `see_cp`/`consequence` are anchored on —
    /// the target that gives the attacker the best real SEE outcome, not
    /// necessarily the (or an) undefended one. A rook defended once by a
    /// pawn can still be this fork's real point (net +180 to capture it
    /// with a knight) even though it technically "has a defender."
    pub hangs: Option<PieceRef>,
    /// Net material result (centipawns) of playing out the best capture
    /// sequence on `hangs`'s square — a real static-exchange evaluation
    /// (`ThreatGraph::see`), not a face-value guess. **Always from the
    /// mover's perspective** (the side that owns `attacker` — the piece
    /// that would actually make the capture), regardless of whose turn it
    /// is: positive means the fork actually wins material for the mover,
    /// not merely that a fork-shaped pattern exists. Same perspective rule
    /// as `Outnumbered.see_cp`/`MoverFavored.see_cp`, just already explicit
    /// here since `attacker` names the mover directly instead of leaving it
    /// as "`piece.color`'s opponent."
    ///
    /// **Known limitation, not yet fixed**: `see_chain` prices the initial
    /// capture correctly (that part is exact — it equals `HangingPiece`'s
    /// own zero-defender-case math), but every recapture from the first one
    /// onward is currently mispriced (it charges the *recapturing* piece's
    /// own value instead of the value of whatever it's actually capturing),
    /// and the contested square silently drifts to the wrong square past
    /// the first recapture too — see `FINDINGS.md`'s "see_chain gives wrong
    /// answers for 2+ step exchanges" and the 2026-08-31 follow-up entry.
    /// In practice this means: exact only when `hangs` has zero defenders
    /// (no recapture happens at all); approximate — sometimes by a lot —
    /// whenever `hangs` has at least one defender. The bucketed
    /// `consequence` below has still landed correctly on every case checked
    /// so far (the direction of the error hasn't flipped a verdict yet),
    /// but that's not a guarantee for every position.
    pub see_cp: i64,
    /// `see_cp` bucketed into a plain verdict, same attacker-perspective
    /// convention as `see_cp` — same known limitation for exchanges longer
    /// than one capture-and-recapture.
    pub consequence: Consequence,
}

#[derive(Debug, Clone, Serialize)]
pub enum PinType { Absolute, Relative }

#[derive(Debug, Clone, Serialize)]
pub struct Pin {
    pub attacker: PieceRef,
    pub pinned: PieceRef,
    pub shielded: PieceRef,
    pub pin_type: PinType,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skewer {
    pub attacker: PieceRef,
    pub front: PieceRef,
    pub behind: PieceRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredAttack {
    pub mover: PieceRef,
    pub attacker: PieceRef,
    pub target: PieceRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct HangingPiece {
    pub piece: PieceRef,
    pub attacker_count: u8,
    /// Material at stake if the attacker simply takes it — exact, not an
    /// estimate: with zero defenders (this struct's whole definition) there's
    /// no recapture, so this equals the full SEE result for the single
    /// capture on this square, not just a face-value approximation of it.
    pub value: i64,
    /// Whether at least one attacker can actually capture here without its
    /// own king ending up in check (`ThreatGraph::collapse_criticality`) —
    /// false is the "brilliant move looks like a hanging piece" case: a raw
    /// zero-defender count says this piece is lost, but every attacker that
    /// could take it would expose its own king, so nobody safely can. Not a
    /// search or a value judgment about whether the sacrifice is *good* —
    /// just whether taking it is even safe for the taker.
    pub safe_to_capture: bool,
}

/// The plainest form of miscalculation, and the most direct application of
/// `ThreatGraph::control`: a piece with real defenders (so `find_hanging`
/// doesn't touch it) where attackers still outnumber them. `attacker_count`/
/// `defender_count`/`value` are the raw count fact alone — the defending
/// side runs out of recapture material before the attacking side does —
/// independent of whether the actual trade is favorable; `see_cp`/
/// `consequence` below are the (currently imprecise, see their own doc
/// comments) attempt at that pricing question.
#[derive(Debug, Clone, Serialize)]
pub struct Outnumbered {
    pub piece: PieceRef,
    pub attacker_count: u8,
    pub defender_count: u8,
    pub value: i64,
    /// Net material result (centipawns) of the mover — the side that would
    /// actually initiate a capture here, always `piece.color`'s opponent —
    /// playing out the capture sequence on this square (`ThreatGraph::see`).
    /// Real defenders exist here (unlike `HangingPiece`), so unlike
    /// `HangingPiece.value` this is not automatically the full piece value.
    /// **Always from the mover's perspective, never `piece`'s own side's**
    /// — same convention as `Fork.see_cp`/`MoverFavored.see_cp`, one
    /// perspective rule for all three concepts. No color/side field is
    /// carried alongside it: this crate's positions are always analyzed in
    /// the canonical White-to-move frame (see `CLAUDE.md`), so a `Side`
    /// value here would be uselessly constant — "the mover" is the only
    /// perspective that means anything at this layer, and it's fixed by
    /// definition (the opponent of whoever owns `piece`), not a fact that
    /// needs its own field.
    ///
    /// **Same known limitation as `Fork.see_cp`** (see its doc comment):
    /// every recapture is currently mispriced, and since `Outnumbered` by
    /// definition always has at least one defender, this value should be
    /// treated as approximate, not exact.
    pub see_cp: i64,
    /// `see_cp` bucketed into a plain verdict, same convention as `Fork` —
    /// same known limitation on the underlying `see_cp`.
    pub consequence: Consequence,
}

/// The gap between `HangingPiece` (zero defenders) and `Outnumbered`
/// (attackers exceed defenders): a piece with real defenders, at least as
/// many as it has attackers — the raw count says it's covered — where the
/// *first* exchange still favors the mover because the cheapest attacking
/// piece is worth less than the piece it's attacking (a pawn attacking a
/// knight that only a rook can recapture, say — 1 attacker, 1 defender; or
/// a queen attacked by a single bishop but "defended" by both a king and a
/// knight — 1 attacker, 2 defenders, still lost outright). It isn't
/// automatically safe just because the count matches or the defender count
/// is generous; neither `find_hanging` nor `find_outnumbered` can see this
/// at all, since both are pure count comparisons, blind to piece values.
///
/// This started life trying to catch a real miscalculation — playing
/// `dxc5` in a session game left a knight with 2 attackers and 2 defenders
/// that a (buggy) chain-walk claimed still favored the mover — and was
/// first shipped restricted to *exactly* 1 attacker/1 defender once
/// `ThreatGraph::see`/`see_chain` was found to give the wrong sign even on
/// that simplest case (see `see_cp`'s doc comment). That 1-vs-1 restriction
/// then turned out to be too narrow in its own right: a later live game
/// lost a queen with *two* real defenders (a king and a knight, the second
/// missed by eye at the board) to a single bishop, for exactly the same
/// underlying reason a 1-vs-1 case would be lost — a bad first exchange
/// doesn't stop being bad just because more defenders exist. What ships now
/// is the generalization that's still verified correct: any real
/// attacker/defender counts, as long as attackers don't outnumber
/// defenders, computed from just the cheapest attacker's value. The
/// original `dxc5` 2-attacker case that motivated this remains a genuinely
/// open gap (its answer depends on which of *two* attackers is cheapest
/// AND on the deeper multi-step exchange `see_chain` still can't be
/// trusted for) — see `FINDINGS.md`'s "MoverFavored" entries (2026-08-31
/// and 2026-09-01, both dated).
#[derive(Debug, Clone, Serialize)]
pub struct MoverFavored {
    pub piece: PieceRef,
    pub attacker_count: u8,
    pub defender_count: u8,
    /// Net material result (centipawns): the piece's own value minus the
    /// *cheapest* attacker's value, for the mover (`piece.color`'s
    /// opponent) — same perspective rule as `Outnumbered.see_cp`, see its
    /// doc comment for why no separate color field accompanies it. Computed
    /// directly, **not** through `ThreatGraph::see`/`see_chain` — that was
    /// tried first and found to give the wrong sign on exactly this shape
    /// of position (see `find_mover_favored`'s doc comment for the full
    /// reproduction and root cause). This is a deliberately
    /// **first-exchange-only** number: it doesn't account for what a
    /// *second* attacker or a longer recapture chain would change — see
    /// `find_mover_favored`'s doc comment for exactly what is and isn't
    /// covered. Only ever populated when this actually favors the mover
    /// enough to clear the same `Consequence::Winning`/`Minor` bar the
    /// other two concepts use — a `MoverFavored` entry always means the
    /// mover really does come out ahead on the first exchange, not "here's
    /// a number, judge for yourself."
    pub see_cp: i64,
    pub consequence: Consequence,
}

/// The mirror image of a fork: one piece is the *sole* defender of two or
/// more of its own side's currently-attacked pieces. Not a search result —
/// a pure `attackers_to` lookup, same substrate `HangingPiece` reads. If
/// this piece is captured, distracted, or pinned, everything it alone
/// covers becomes as undefended as a zero-defender hanging piece.
#[derive(Debug, Clone, Serialize)]
pub struct Overloaded {
    pub piece: PieceRef,
    pub critical_for: Vec<PieceRef>,
    /// Sum of `critical_for`'s piece values — what's jointly at risk if this
    /// piece's coordination breaks down. Computed once here, where the real
    /// `Role` enum is on hand, not re-derived from `PieceRef.role` strings
    /// downstream (same reasoning as `HangingPiece.value`).
    pub critical_value: i64,
}

/// A piece with a nonzero raw defender count (so `find_hanging` doesn't flag
/// it) where every defender is pinned *and* none of them could legally
/// recapture here anyway — the pin restricts moving off the attacker–king
/// line, not moving along it, so a defender is only truly neutralized when
/// this square isn't on that line too. Cross-references two already-
/// detected facts (`attackers_to` and the separately-detected `pins` list);
/// no capture is simulated, no exchange priced.
#[derive(Debug, Clone, Serialize)]
pub struct FalseDefense {
    pub piece: PieceRef,
    pub attacker_count: u8,
    pub pinned_defenders: Vec<PieceRef>,
    /// Piece value of the falsely-defended piece itself — what's really at
    /// stake once its defenders are recognized as unable to help.
    pub value: i64,
}

/// The rung above `Outnumbered`, `Overloaded`, and `FalseDefense`: the raw
/// count says this piece is adequately defended
/// (`raw_defender_count >= attacker_count`, so neither `find_hanging` nor
/// `find_outnumbered` touch it) — but it isn't, once defenders already
/// spoken for elsewhere are discounted: pinned off the recapture line
/// (`FalseDefense`'s per-defender fact, generalized here to a partial count
/// instead of requiring *every* defender compromised) or the sole defender
/// of another piece (`Overloaded`'s fact). This is the fact a player who
/// "did the count right" can still miss — the count was right, the
/// commitments weren't seen. Both counts are carried so a report can show
/// the gap (what the bare numbers said vs. what's actually true), not just
/// the conclusion.
#[derive(Debug, Clone, Serialize)]
pub struct FalseSafety {
    pub piece: PieceRef,
    pub attacker_count: u8,
    pub raw_defender_count: u8,
    pub effective_defender_count: u8,
    pub compromised_defenders: Vec<PieceRef>,
    /// Piece value of the falsely-safe piece itself.
    pub value: i64,
}

/// `ThreatGraph::collapse_criticality` — feeds `HangingPiece.safe_to_capture`
/// (wired into `extract_concepts`), otherwise still experimental beyond
/// that: what the position looks like if every piece contesting a square
/// traded off except this one — the whole local cluster (every attacker,
/// defender, and occupant) cleared, then just this candidate placed back.
/// Not a capture simulation: no order, no recapture choice, just "if this is
/// the piece left standing here, is that actually safe for it." The general
/// mechanism `Overloaded` and `FalseDefense` are each one special case of: a
/// pin is "placing this piece here leaves its own king in check"; an
/// overload is "placing this piece here swings control of some *other*
/// square." Deltas and exposure — with names, not just booleans — not a
/// verdict; the caller decides what they mean.
#[derive(Debug, Clone, Serialize)]
pub struct PieceCriticality {
    pub piece: PieceRef,
    /// control(sq, piece's color) on the clean-slate-plus-this-piece board
    /// minus the same reading on the real position.
    pub square_control_delta: i32,
    pub white_king_zone_delta: i32,
    pub black_king_zone_delta: i32,
    /// This piece's own king is in check once it's the one standing on the
    /// square — the false-defender signal: this piece can't actually go
    /// there safely, whatever the raw count said.
    pub own_king_in_check: bool,
    /// Which piece(s) deliver that check (`ThreatGraph::checkers`) — not
    /// just *that* this candidate can't safely stand here, but *because of
    /// what*, named.
    pub own_king_checked_by: Vec<PieceRef>,
    /// Stronger version of the same signal: no legal escape at all
    /// (best-effort — see `is_checkmate_via_shakmaty`).
    pub own_king_checkmated: bool,
    /// This piece, once it's the one standing on the square, delivers
    /// check to the opponent — true whenever `delivers_check_via` is
    /// non-empty; note the checking piece need not be this candidate itself
    /// (a discovered check the candidate's placement reveals is exactly as
    /// identifiable here as a direct one).
    pub delivers_check: bool,
    pub delivers_check_via: Vec<PieceRef>,
    pub delivers_checkmate: bool,
    /// Pieces `find_hanging` reports on this hypothetical board that
    /// weren't already hanging in the real position — a fresh consequence
    /// of this specific candidate ending up on the contested square, not
    /// something that was already true regardless.
    pub newly_hanging: Vec<PieceRef>,
}

// ── Positional concepts ──

#[derive(Debug, Clone, Serialize)]
pub struct Outpost {
    pub piece: PieceRef,
    pub supported_by: PieceRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenFile {
    pub file: String,
    pub rook_count: u8,
    pub color: Side,
}

#[derive(Debug, Clone, Serialize)]
pub struct PassedPawn {
    pub square: String,
    pub rank: u8,
    pub color: Side,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PawnIsland {
    pub files: Vec<String>,
    pub count: u8,
    pub color: Side,
}

#[derive(Debug, Clone, Serialize)]
pub struct KingExposure {
    pub color: Side,
    pub shelter_files: u8,
    pub attacker_count: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoubledPawn {
    pub file: String,
    pub count: u8,
    pub color: Side,
}

#[derive(Debug, Clone, Serialize)]
pub struct IsolatedPawn {
    pub square: String,
    pub color: Side,
}

// ── Material concepts ──

#[derive(Debug, Clone, Serialize, Default)]
pub struct PieceCounts {
    pub queens: u8,
    pub rooks: u8,
    pub bishops: u8,
    pub knights: u8,
    pub pawns: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterialBalance {
    pub white: PieceCounts,
    pub black: PieceCounts,
    pub centipawns: i64,
    pub bishop_pair_white: bool,
    pub bishop_pair_black: bool,
}

// ── Development concepts ──

#[derive(Debug, Clone, Serialize)]
pub struct DevelopmentInfo {
    pub color: Side,
    pub undeveloped_pieces: Vec<PieceRef>,
    pub space_advantage: i64,
}

// ── Other concepts ──

#[derive(Debug, Clone, Serialize)]
pub struct PawnBreak {
    pub square: String,
    pub color: Side,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinorityAttack {
    pub color: Side,
    pub strength: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PawnMajority {
    pub color: Side,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RookOnSeventh {
    pub color: Side,
    pub count: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct CenterControl {
    pub color: Side,
    pub strength: i64,
}

/// A gated issue scored by magnitude × severity × elo_relevance × confidence.
/// Lives here (not in `concepts.rs`, where `rank_issues_for_position`/
/// `rank_issues_for_player` build it) because `SensorReport` (`sensor.rs`)
/// carries a `Vec<GatedIssue>` field — putting the type in the shared
/// foundational types module `sensor.rs` already depends on avoids a
/// `sensor.rs` <-> `concepts.rs` mutual dependency.
#[derive(Debug, Clone, Serialize)]
pub struct GatedIssue {
    pub name: String,
    pub severity: i64,
    pub elo_min: i32,
    pub magnitude: f64,
    pub elo_relevance: f64,
    pub confidence: f64,
    pub score: f64,
    pub phrase: String,
    /// See `Mover`'s doc comment: `Us`/`Them`, never a real color — the
    /// client already has `PositionRecord.side_to_move` and can translate.
    pub mover: Mover,
    /// Which rung of the piece-safety ladder this issue represents
    /// (`threat_graph.rs`'s module doc): 1 = `hanging_piece` (no defenders
    /// at all), 2 = `outnumbered` (raw count insufficient), 3 =
    /// `overloaded`/`false_defense` (a defender's own commitment elsewhere),
    /// 4 = `false_safety` (the raw count alone looked fine but wasn't). 0
    /// for every concept outside this ladder. Not a severity or ELO gate —
    /// those already exist — this exists so the coach can say *how deep* a
    /// correct calculation needed to go, not just that a mistake happened.
    pub stage: u8,
}
