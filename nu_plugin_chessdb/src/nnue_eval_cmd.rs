use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;
use crate::stockfish::StockfishEngine;

pub struct NnueEval;

impl PluginCommand for NnueEval {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb nnue-eval"
    }

    fn description(&self) -> &str {
        "Evaluate chess positions using Stockfish NNUE. Accepts a FEN string or list of FEN strings."
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

        let fens: Vec<String> = match input_value {
            Value::String { val, .. } => vec![val],
            Value::List { vals, .. } => vals
                .iter()
                .filter_map(|v| v.as_str().ok().map(|s| s.to_string()))
                .collect(),
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
                "nnue_score" => Value::int(score, span),
            };
            results.push(Value::record(record, span));
        }

        if results.len() == 1 {
            Ok(PipelineData::Value(results.remove(0), None))
        } else {
            Ok(PipelineData::Value(Value::list(results, span), None))
        }
    }
}
