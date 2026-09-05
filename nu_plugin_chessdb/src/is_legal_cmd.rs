use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use crate::core::{gives_check, is_legal};
use crate::utils::fen_from_input;
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
        let fen = fen_from_input(input, span)?;
        let result = is_legal(&fen, &move_str, span)?;
        Ok(PipelineData::Value(Value::bool(result, span), None))
    }
}

/// Nu-facing exposure of `core::gives_check` — would this candidate UCI
/// move give check, directly, without applying it first (previously only
/// derivable indirectly: `apply-uci` then check `in_check`). Same
/// FEN-via-pipeline + one-flag + bool-output shape as `IsLegal` above.
pub struct GivesCheckCmd;

impl PluginCommand for GivesCheckCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb gives-check"
    }

    fn description(&self) -> &str {
        "Would --uci (a UCI move, must be legal) give check in this FEN (pipeline input)? Composed from clone/play_unchecked/is_check -- shakmaty's own Chess::gives_check is private and feature-gated, see core::gives_check."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("uci", SyntaxShape::String, "The move to check, UCI, e.g. 'g1f3'", Some('u'))
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
        let uci_str: String = call
            .get_flag("uci")?
            .ok_or_else(|| LabeledError::new("--uci is required").with_label("missing move", span))?;
        let fen = fen_from_input(input, span)?;
        let result = gives_check(&fen, &uci_str, span)?;
        Ok(PipelineData::Value(Value::bool(result, span), None))
    }
}
