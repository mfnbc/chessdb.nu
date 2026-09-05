use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};
use shakmaty::Square;

use crate::chess::fen_to_chess;
use crate::eval::threat_graph::ThreatGraph;
use crate::utils::{fen_from_input, to_pipeline_data};
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `ThreatGraph::collapse_criticality` — otherwise a
/// Rust-only method with no way for the Nu side (`chess-review` and friends)
/// to read its per-candidate, fully-identifiable facts (named checkers,
/// king-zone deltas, newly-hanging pieces). One record per cluster
/// candidate, no aggregation, no narrative — the structured facts only; see
/// FINDINGS.md's "what can be described vs. what can be quantified" entry for
/// why this command deliberately doesn't try to also produce a phrase.
pub struct CollapseCriticalityCmd;

impl PluginCommand for CollapseCriticalityCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb collapse-criticality"
    }

    fn description(&self) -> &str {
        "Clear the local cluster contesting a square (every attacker, defender, occupant) and test each candidate individually — see ThreatGraph::collapse_criticality. Returns one record per candidate: king-zone deltas, named checkers, newly-hanging pieces. FEN via pipeline, square via --square."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("square", SyntaxShape::String, "The contested square, e.g. 'f5'", Some('s'))
            .input_output_types(vec![(
                Type::String,
                Type::List(Box::new(Type::Record(vec![].into()))),
            )])
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
        let square_str: String = call
            .get_flag("square")?
            .ok_or_else(|| LabeledError::new("--square is required").with_label("missing square", span))?;
        let sq = Square::from_ascii(square_str.as_bytes())
            .map_err(|_| LabeledError::new(format!("invalid square: {square_str}")).with_label("expected a square like 'f5'", span))?;

        let fen = fen_from_input(input, span)?;
        let chess = fen_to_chess(&fen, span)?;

        let graph = ThreatGraph::build(&chess);
        let results = graph.collapse_criticality(sq);
        to_pipeline_data(&results, span)
    }
}
