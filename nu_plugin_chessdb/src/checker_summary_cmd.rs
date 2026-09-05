use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type};

use crate::core::checker_summary;
use crate::utils::{fen_from_input, to_pipeline_data};
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::checker_summary` — is_check/is_checkmate and,
/// distinct from `chessdb fen-info`, the actual checking square(s) (useful
/// to tell a single check from a double check).
pub struct CheckerSummaryCmd;

impl PluginCommand for CheckerSummaryCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb checker-summary"
    }

    fn description(&self) -> &str {
        "is_check/is_checkmate and the named checking square(s) for a FEN (pipeline input)."
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
        let result = checker_summary(&fen, span)?;
        to_pipeline_data(&result, span)
    }
}
