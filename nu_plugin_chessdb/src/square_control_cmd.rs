use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use crate::core::square_control;
use crate::utils::json_to_nu_value;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `core::square_control` — every square one piece
/// geometrically controls (occupancy-aware, turn/pin-independent), so a
/// caller can render a spatial "what does this piece see" view instead of
/// deriving it by hand.
pub struct SquareControlCmd;

impl PluginCommand for SquareControlCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb square-control"
    }

    fn description(&self) -> &str {
        "Every square the piece on --square geometrically controls (occupancy-aware, independent of whose turn it is), for a FEN (pipeline input)."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("square", SyntaxShape::String, "The square to inspect, e.g. 'e4'", Some('s'))
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
        let square: String = call
            .get_flag("square")?
            .ok_or_else(|| LabeledError::new("--square is required").with_label("missing square", span))?;
        let fen = match input.into_value(span)? {
            Value::String { val, .. } => val,
            _ => return Err(LabeledError::new("expected a FEN string").with_label("invalid input type", span)),
        };
        let result = square_control(&fen, &square, span)?;
        let json = serde_json::to_value(&result)
            .map_err(|e| LabeledError::new(e.to_string()).with_label("serialization error", span))?;
        Ok(PipelineData::Value(json_to_nu_value(json, span), None))
    }
}
