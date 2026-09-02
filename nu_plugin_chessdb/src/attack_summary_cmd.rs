use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::core::attack_summary;
use crate::utils::json_to_nu_value;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::attack_summary` — every square attacked by
/// each side and the per-side attack counts.
pub struct AttackSummaryCmd;

impl PluginCommand for AttackSummaryCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb attack-summary"
    }

    fn description(&self) -> &str {
        "Every square attacked by White and by Black, plus per-side attack counts, for a FEN (pipeline input)."
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
        let result = attack_summary(&fen, span)?;
        let json = serde_json::to_value(&result)
            .map_err(|e| LabeledError::new(e.to_string()).with_label("serialization error", span))?;
        Ok(PipelineData::Value(json_to_nu_value(json, span), None))
    }
}
