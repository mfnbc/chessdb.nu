use serde::Serialize;

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
    pub side: Side,
}
