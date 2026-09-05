use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type};

use crate::core::attack_summary;
use crate::utils::{fen_from_input, to_pipeline_data};
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
        let fen = fen_from_input(input, span)?;
        let result = attack_summary(&fen, span)?;
        to_pipeline_data(&result, span)
    }
}
