use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type};

use crate::core::mobility_summary;
use crate::utils::{fen_from_input, to_pipeline_data};
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
        let fen = fen_from_input(input, span)?;
        let result = mobility_summary(&fen, span)?;
        to_pipeline_data(&result, span)
    }
}
