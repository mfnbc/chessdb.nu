use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};

use crate::core::{bitboard_mask, board_piece_at, board_pieces, board_pieces_ascii};
use crate::utils::{fen_from_input, to_pipeline_data};
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `Board::occupied`/`by_color`/`by_role`/`by_piece`
/// — the board's own piece-placement bitboards, filtered by whichever of
/// `--color`/`--role` is given.
pub struct BoardPiecesCmd;

impl PluginCommand for BoardPiecesCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb board-pieces"
    }

    fn description(&self) -> &str {
        "Board::occupied/by_color/by_role/by_piece for a FEN (pipeline input) -- every square holding a piece matching --color and/or --role (both omitted = every occupied square, both given = exactly by_piece)."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("color", SyntaxShape::String, "'white' or 'black' (optional)", Some('c'))
            .named("role", SyntaxShape::String, "pawn/knight/bishop/rook/queen/king (optional)", Some('r'))
            .input_output_types(vec![(Type::String, Type::Record(vec![].into()))])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let fen = fen_from_input(input, span)?;
        let color: Option<String> = call.get_flag("color")?;
        let role: Option<String> = call.get_flag("role")?;
        let result = board_pieces(&fen, color.as_deref(), role.as_deref(), span)?;
        to_pipeline_data(&result, span)
    }
}

/// A `Board::occupied`/`by_color`/`by_role`/`by_piece` bitboard, rendered
/// to text entirely in Rust (an ASCII grid and a FEN-piece-placement-shaped
/// string) — 2026-09-04, explicit user direction: bitwise/rendering work
/// stays in Rust, Nu only ever consumes the finished text.
pub struct BoardPiecesAsciiCmd;

impl PluginCommand for BoardPiecesAsciiCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb board-pieces-ascii"
    }

    fn description(&self) -> &str {
        "Same selection as board-pieces (--color/--role, both optional), rendered as an 8-line ASCII grid (rank 8 at top) and a compact FEN-piece-placement-shaped string, computed entirely in Rust from the real Bitboard."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("color", SyntaxShape::String, "'white' or 'black' (optional)", Some('c'))
            .named("role", SyntaxShape::String, "pawn/knight/bishop/rook/queen/king (optional)", Some('r'))
            .input_output_types(vec![(Type::String, Type::Record(vec![].into()))])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let fen = fen_from_input(input, span)?;
        let color: Option<String> = call.get_flag("color")?;
        let role: Option<String> = call.get_flag("role")?;
        let result = board_pieces_ascii(&fen, color.as_deref(), role.as_deref(), span)?;
        to_pipeline_data(&result, span)
    }
}

/// A named `Bitboard` associated constant (`Bitboard::CENTER`, `::CORNERS`,
/// ...), rendered the same way `board-pieces-ascii` renders any other
/// bitboard. Position-independent — no FEN, matching `geometry_cmd.rs`'s
/// no-pipeline-input pattern.
pub struct BitboardMaskCmd;

impl PluginCommand for BitboardMaskCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb bitboard-mask"
    }

    fn description(&self) -> &str {
        "A named Bitboard associated constant (dark-squares/light-squares/center/edges/corners/backranks/north/south/west/east), rendered as an ASCII grid and FEN-piece-placement-shaped string. Position-independent -- no FEN, pure geometry."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("name", SyntaxShape::String, "dark-squares/light-squares/center/edges/corners/backranks/north/south/west/east", Some('n'))
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let name: String = call.get_flag("name")?.ok_or_else(|| LabeledError::new("--name is required").with_label("missing name", span))?;
        let result = bitboard_mask(&name, span)?;
        to_pipeline_data(&result, span)
    }
}

/// Nu-facing exposure of `Board::piece_at` — the single piece on one
/// square, or null if empty.
pub struct BoardPieceAtCmd;

impl PluginCommand for BoardPieceAtCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb board-piece-at"
    }

    fn description(&self) -> &str {
        "Board::piece_at(--square) for a FEN (pipeline input) -- {color, role} or null if empty."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("square", SyntaxShape::String, "The square to inspect, e.g. 'e4'", Some('s'))
            .input_output_types(vec![(Type::String, Type::Any)])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let fen = fen_from_input(input, span)?;
        let square: String = call.get_flag("square")?.ok_or_else(|| LabeledError::new("--square is required").with_label("missing square", span))?;
        let result = board_piece_at(&fen, &square, span)?;
        to_pipeline_data(&result, span)
    }
}
