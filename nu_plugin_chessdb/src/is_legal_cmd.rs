use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use crate::core::is_legal;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::is_legal` — a cheap yes/no legality check
/// (accepts SAN or UCI) without the try/catch scaffolding `chessdb apply-uci`
/// needs for the same question, since apply-uci signals illegality via error.
pub struct IsLegal;

impl PluginCommand for IsLegal {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb is-legal"
    }

    fn description(&self) -> &str {
        "Is this move (SAN or UCI) legal in this FEN (pipeline input)? --move is required."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("move", SyntaxShape::String, "The move to check, SAN or UCI, e.g. 'Nf3' or 'g1f3'", Some('m'))
            .input_output_types(vec![(Type::String, Type::Bool)])
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
        let move_str: String = call
            .get_flag("move")?
            .ok_or_else(|| LabeledError::new("--move is required").with_label("missing move", span))?;
        let fen = match input.into_value(span)? {
            Value::String { val, .. } => val,
            _ => return Err(LabeledError::new("expected a FEN string").with_label("invalid input type", span)),
        };
        let result = is_legal(&fen, &move_str, span)?;
        Ok(PipelineData::Value(Value::bool(result, span), None))
    }
}
