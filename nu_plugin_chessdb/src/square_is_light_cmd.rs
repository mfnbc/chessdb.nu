use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};

use crate::core::square_is_light;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `Square::is_light` — a pure geometric fact about
/// the square itself, no board or position dependency at all.
pub struct SquareIsLightCmd;

impl PluginCommand for SquareIsLightCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb square-is-light"
    }

    fn description(&self) -> &str {
        "Square::is_light(--square) -- true for a light square (a1, h8, ...), false for dark. No FEN, no pipeline input."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("square", SyntaxShape::String, "The square to inspect, e.g. 'e4'", Some('s'))
            .input_output_types(vec![(Type::Nothing, Type::Bool)])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let square: String = call.get_flag("square")?.ok_or_else(|| LabeledError::new("--square is required").with_label("missing square", span))?;
        let result = square_is_light(&square, span)?;
        Ok(PipelineData::Value(nu_protocol::Value::bool(result, span), None))
    }
}
