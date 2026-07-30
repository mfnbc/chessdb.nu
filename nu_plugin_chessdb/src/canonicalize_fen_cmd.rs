use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::core::canonicalize_fen;
use crate::utils::map_string_or_list;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

pub struct CanonicalizeFen;

impl PluginCommand for CanonicalizeFen {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb canonicalize-fen"
    }

    fn description(&self) -> &str {
        "Normalize a FEN (or list of FENs) to the canonical White-always-to-move frame positions.zobrist/.fen use."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![
                (Type::String, Type::String),
                (
                    Type::List(Box::new(Type::String)),
                    Type::List(Box::new(Type::String)),
                ),
            ])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        map_string_or_list(input, call.head, |fen, span| {
            Ok(Value::string(canonicalize_fen(fen, span)?, span))
        })
    }
}
