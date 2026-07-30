use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::core::zobrist;
use crate::utils::map_string_or_list;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

pub struct Zobrist;

impl PluginCommand for Zobrist {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb zobrist"
    }

    fn description(&self) -> &str {
        "Compute Zobrist hash for a FEN or list of FENs."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .switch("int", "Output as integer instead of hex", Some('i'))
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
        let as_int = call.has_flag("int")?;
        map_string_or_list(input, call.head, |fen, span| {
            Ok(Value::string(zobrist(fen, as_int, span)?, span))
        })
    }
}
