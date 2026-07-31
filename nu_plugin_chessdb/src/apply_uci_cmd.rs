use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use crate::core::apply_uci;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Play one UCI move on a FEN and return the resulting FEN — in whatever
/// frame the input FEN was in (real or canonical), no normalization either
/// way. Exists specifically so callers can replay a game's *real* moves
/// (`moves.uci`, always stored in real terms) from the real starting
/// position to reconstruct real per-ply FENs — `positions.fen` is stored
/// canonical (White-always-to-move) and is *not* a substitute for this: for
/// any ply where the true side to move is Black, it's a color-swapped,
/// rank-mirrored version of the real position, not the real position
/// itself. See PLAN.md's "tactical_events fed canonical FENs" entry.
pub struct ApplyUci;

impl PluginCommand for ApplyUci {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb apply-uci"
    }

    fn description(&self) -> &str {
        "Play one UCI move on a FEN (pipeline input) and return the resulting FEN, same frame in as out — no canonicalization. Use to replay moves.uci from the real starting position and reconstruct real per-ply FENs."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("uci", SyntaxShape::String, "The move in UCI form, e.g. 'e2e4'", Some('u'))
            .input_output_types(vec![(Type::String, Type::String)])
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
        let uci: String = call
            .get_flag("uci")?
            .ok_or_else(|| LabeledError::new("--uci is required").with_label("missing move", span))?;
        let fen = match input.into_value(span)? {
            Value::String { val, .. } => val,
            _ => return Err(LabeledError::new("expected a FEN string").with_label("invalid input type", span)),
        };
        let result = apply_uci(&fen, &uci, span)?;
        Ok(PipelineData::Value(Value::string(result, span), None))
    }
}
