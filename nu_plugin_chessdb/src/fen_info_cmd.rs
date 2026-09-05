use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type};

use crate::core::fen_info;
use crate::utils::{fen_from_input, to_pipeline_data};
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::fen_info` — material, check/checkmate/
/// stalemate, en passant, half/full move counters, legal move count. Was
/// previously reachable only by paying for a full `hugm-eval` call; this is
/// the cheap version for a plain "what's the state of this position" check.
pub struct FenInfo;

impl PluginCommand for FenInfo {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb fen-info"
    }

    fn description(&self) -> &str {
        "Material, check/checkmate/stalemate, en passant square, move counters, and legal move count for a FEN (pipeline input). Cheaper than chessdb hugm-eval when you only need these facts."
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
        let result = fen_info(&fen, span)?;
        to_pipeline_data(&result, span)
    }
}
