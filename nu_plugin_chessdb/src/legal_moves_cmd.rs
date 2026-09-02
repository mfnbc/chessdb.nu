use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::core::mobility_summary;
use crate::utils::json_to_nu_value;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::mobility_summary` — side to move, legal move
/// count, and the full legal move list in SAN. Named for what it answers
/// ("what can I play right now") rather than the internal function name.
pub struct LegalMoves;

impl PluginCommand for LegalMoves {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb legal-moves"
    }

    fn description(&self) -> &str {
        "Side to move, legal move count, and the full legal move list in SAN for a FEN (pipeline input)."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
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
        let fen = match input.into_value(span)? {
            Value::String { val, .. } => val,
            _ => return Err(LabeledError::new("expected a FEN string").with_label("invalid input type", span)),
        };
        let result = mobility_summary(&fen, span)?;
        let json = serde_json::to_value(&result)
            .map_err(|e| LabeledError::new(e.to_string()).with_label("serialization error", span))?;
        Ok(PipelineData::Value(json_to_nu_value(json, span), None))
    }
}
