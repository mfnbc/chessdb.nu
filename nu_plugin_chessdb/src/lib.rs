pub mod apply_uci_cmd;
pub mod attack_summary_cmd;
pub mod board_pieces_cmd;
pub mod canonical;
pub mod canonicalize_fen_cmd;
pub mod checker_summary_cmd;
pub mod chess;
pub mod collapse_criticality_cmd;
pub mod core;
// critter_eval_cmd removed — undifferentiated alias for hugm-eval (BUG-7)
pub mod hugm_eval_cmd;
pub mod eval;
pub mod fen_info_cmd;
pub mod is_legal_cmd;
pub mod legal_moves_cmd;
pub mod pgn_to_fens;
pub mod process_corpus;
pub mod game_parse;
pub mod geometry_cmd;
pub mod stockfish_eval_cmd;
pub mod coach_derive_cmd;
pub mod square_is_light_cmd;
pub mod stockfish;
pub mod utils;
pub mod zobrist;
// square_control_cmd / square_attackers_cmd / square_swap_list_cmd /
// board_probe_cmd removed 2026-09-03 — rust-side composition over shakmaty
// replaced by the geom-attacks/board-pieces/board-piece-at/square-is-light
// leaf commands above, composed in nushell instead
// (scripts/play/shakmaty_compose.nu). Every nu-composed replacement was
// A/B-verified byte-identical against these before removal — see
// FINDINGS.md and the `chessdb_shakmaty_1to1` memory.

use nu_plugin::Plugin;

/// Shared help-category string for all `chessdb *` plugin commands.
pub const PLUGIN_CATEGORY: &str = "chess";

pub struct ChessdbPlugin;

impl ChessdbPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChessdbPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ChessdbPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // Small, focused command surface including the evaluations
    fn commands(&self) -> Vec<Box<dyn nu_plugin::PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(hugm_eval_cmd::HugmEval),
            // critter-eval removed — use hugm-eval instead
            Box::new(pgn_to_fens::PgnToBatch),
            Box::new(pgn_to_fens::PgnToFens),
            Box::new(process_corpus::ProcessCorpus),
            Box::new(stockfish_eval_cmd::StockfishEval),
            Box::new(coach_derive_cmd::DeriveCoachSignals),
            Box::new(zobrist::Zobrist),
            Box::new(canonicalize_fen_cmd::CanonicalizeFen),
            Box::new(collapse_criticality_cmd::CollapseCriticalityCmd),
            Box::new(apply_uci_cmd::ApplyUci),
            Box::new(fen_info_cmd::FenInfo),
            Box::new(legal_moves_cmd::LegalMoves),
            Box::new(attack_summary_cmd::AttackSummaryCmd),
            Box::new(checker_summary_cmd::CheckerSummaryCmd),
            Box::new(is_legal_cmd::IsLegal),
            Box::new(geometry_cmd::GeomAttacksCmd),
            Box::new(geometry_cmd::GeomRayCmd),
            Box::new(geometry_cmd::GeomBetweenCmd),
            Box::new(geometry_cmd::GeomAlignedCmd),
            Box::new(board_pieces_cmd::BoardPiecesCmd),
            Box::new(board_pieces_cmd::BoardPieceAtCmd),
            Box::new(square_is_light_cmd::SquareIsLightCmd),
        ]
    }
}
