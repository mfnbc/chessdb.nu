use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;
use crate::stockfish::StockfishEngine;

pub struct StockfishEval;

impl PluginCommand for StockfishEval {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb stockfish-eval"
    }

    fn description(&self) -> &str {
        "Evaluate chess positions via Stockfish (external oracle, not HUGM). Accepts a FEN string or list of FEN strings."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![
                (Type::String, Type::Record(vec![].into())),
                (
                    Type::List(Box::new(Type::String)),
                    Type::List(Box::new(Type::Record(vec![].into()))),
                ),
            ])
            .category(Category::Custom(PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &nu_plugin::EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let input_value = input.into_value(span)?;

        // `single` tracks the input's own shape (String vs. List), not the
        // element count — a one-element list must still come back as a
        // one-element list, matching the declared List-in/List-out
        // signature and every sibling command's convention, not collapse
        // to a bare record the way a single-FEN string does.
        let (fens, single): (Vec<String>, bool) = match input_value {
            Value::String { val, .. } => (vec![val], true),
            Value::List { vals, .. } => {
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    out.push(v.as_str()?.to_string());
                }
                (out, false)
            }
            _ => {
                return Err(LabeledError::new("Expected a FEN string or list of FEN strings")
                    .with_label("invalid input type", span))
            }
        };

        if fens.is_empty() {
            return Ok(PipelineData::Value(Value::list(vec![], span), None));
        }

        let stockfish_bin =
            std::env::var("STOCKFISH_BIN").unwrap_or_else(|_| "/usr/sbin/stockfish".to_string());
        let mut engine = StockfishEngine::spawn(&stockfish_bin).map_err(LabeledError::new)?;

        let mut results: Vec<Value> = Vec::with_capacity(fens.len());
        for fen in &fens {
            let score = engine.eval_fen(fen.trim()).map_err(LabeledError::new)?.unwrap_or(0);
            let record = nu_protocol::record! {
                "fen" => Value::string(fen, span),
                "stockfish_score" => Value::int(score, span),
            };
            results.push(Value::record(record, span));
        }

        if single {
            Ok(PipelineData::Value(results.remove(0), None))
        } else {
            Ok(PipelineData::Value(Value::list(results, span), None))
        }
    }
}
