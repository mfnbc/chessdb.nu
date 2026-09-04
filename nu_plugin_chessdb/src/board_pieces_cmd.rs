use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use crate::core::{board_piece_at, board_pieces};
use crate::utils::json_to_nu_value;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

fn fen_from_input(input: PipelineData, span: nu_protocol::Span) -> Result<String, LabeledError> {
    match input.into_value(span)? {
        Value::String { val, .. } => Ok(val),
        _ => Err(LabeledError::new("expected a FEN string").with_label("invalid input type", span)),
    }
}

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
        let json = serde_json::to_value(&result).map_err(|e| LabeledError::new(e.to_string()).with_label("serialization error", span))?;
        Ok(PipelineData::Value(json_to_nu_value(json, span), None))
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
        let json = serde_json::to_value(&result).map_err(|e| LabeledError::new(e.to_string()).with_label("serialization error", span))?;
        Ok(PipelineData::Value(json_to_nu_value(json, span), None))
    }
}
