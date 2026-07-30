use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::core::canonicalize_fen;
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
        let input_value = input.into_value(call.head)?;

        match input_value {
            Value::String { val, .. } => {
                let result = canonicalize_fen(&val, call.head)?;
                Ok(PipelineData::Value(Value::string(result, call.head), None))
            }
            Value::List { vals, .. } => {
                let mut results = Vec::with_capacity(vals.len());
                for val in vals {
                    let fen_str = val.as_str()?;
                    let canonical = canonicalize_fen(fen_str, call.head)?;
                    results.push(Value::string(canonical, call.head));
                }
                Ok(PipelineData::Value(Value::list(results, call.head), None))
            }
            _ => Err(LabeledError::new("Expected string or list of strings")
                .with_label("invalid input type", call.head)),
        }
    }
}
