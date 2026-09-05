use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, SyntaxShape, Type};

use crate::core::{geom_aligned, geom_attacks, geom_between, geom_ray, square_distance};
use crate::utils::to_pipeline_data;
use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;

/// Nu-facing exposure of `shakmaty::attacks::attacks` — pure geometry, no
/// board or position, `occupied` always an explicit input (never "the
/// board's current occupancy"). One dispatcher for every piece role.
pub struct GeomAttacksCmd;

impl PluginCommand for GeomAttacksCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb geom-attacks"
    }

    fn description(&self) -> &str {
        "shakmaty::attacks::attacks(square, piece, occupied) -- pure geometry: which squares a --role of --color on --square would attack, given --occupied (a nuon list of squares blocking sliding pieces; ignored for pawn/knight/king). No FEN, no pipeline input -- entirely position-independent."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("square", SyntaxShape::String, "The square the piece stands on, e.g. 'e4'", Some('s'))
            .named("color", SyntaxShape::String, "'white' or 'black'", Some('c'))
            .named("role", SyntaxShape::String, "pawn/knight/bishop/rook/queen/king", Some('r'))
            .named("occupied", SyntaxShape::List(Box::new(SyntaxShape::String)), "Squares occupied by any piece (blocks sliding roles); [] if none", Some('o'))
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
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
        let color: String = call.get_flag("color")?.ok_or_else(|| LabeledError::new("--color is required").with_label("missing color", span))?;
        let role: String = call.get_flag("role")?.ok_or_else(|| LabeledError::new("--role is required").with_label("missing role", span))?;
        let occupied: Vec<String> = call.get_flag("occupied")?.unwrap_or_default();
        let result = geom_attacks(&square, &color, &role, &occupied, span)?;
        to_pipeline_data(&result, span)
    }
}

fn two_square_flags(call: &EvaluatedCall, span: nu_protocol::Span, a_name: &str, b_name: &str) -> Result<(String, String), LabeledError> {
    let a: String = call.get_flag(a_name)?.ok_or_else(|| LabeledError::new(format!("--{a_name} is required")).with_label("missing square", span))?;
    let b: String = call.get_flag(b_name)?.ok_or_else(|| LabeledError::new(format!("--{b_name} is required")).with_label("missing square", span))?;
    Ok((a, b))
}

/// Nu-facing exposure of `shakmaty::attacks::ray`.
pub struct GeomRayCmd;

impl PluginCommand for GeomRayCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb geom-ray"
    }

    fn description(&self) -> &str {
        "shakmaty::attacks::ray(a, b) -- every square on the rank/file/diagonal through both --a and --b (the whole line, both directions), empty if they don't share one. No FEN, no pipeline input."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("a", SyntaxShape::String, "first square", None)
            .named("b", SyntaxShape::String, "second square", None)
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
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
        let (a, b) = two_square_flags(call, span, "a", "b")?;
        let result = geom_ray(&a, &b, span)?;
        to_pipeline_data(&result, span)
    }
}

/// Nu-facing exposure of `shakmaty::attacks::between`.
pub struct GeomBetweenCmd;

impl PluginCommand for GeomBetweenCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb geom-between"
    }

    fn description(&self) -> &str {
        "shakmaty::attacks::between(a, b) -- squares strictly between --a and --b on a shared rank/file/diagonal (endpoints excluded), empty if not aligned. No FEN, no pipeline input."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("a", SyntaxShape::String, "first square", None)
            .named("b", SyntaxShape::String, "second square", None)
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
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
        let (a, b) = two_square_flags(call, span, "a", "b")?;
        let result = geom_between(&a, &b, span)?;
        to_pipeline_data(&result, span)
    }
}

/// Nu-facing exposure of `shakmaty::attacks::aligned`.
pub struct GeomAlignedCmd;

impl PluginCommand for GeomAlignedCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb geom-aligned"
    }

    fn description(&self) -> &str {
        "shakmaty::attacks::aligned(a, b, c) -- true if all three squares share a rank/file/diagonal. No FEN, no pipeline input."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("a", SyntaxShape::String, "first square", None)
            .named("b", SyntaxShape::String, "second square", None)
            .named("c", SyntaxShape::String, "third square", None)
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
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
        let (a, b) = two_square_flags(call, span, "a", "b")?;
        let c: String = call.get_flag("c")?.ok_or_else(|| LabeledError::new("--c is required").with_label("missing square", span))?;
        let result = geom_aligned(&a, &b, &c, span)?;
        to_pipeline_data(&result, span)
    }
}

/// Nu-facing exposure of `Square::distance` -- Chebyshev distance
/// (`max(file_dist, rank_dist)`), the exact primitive `CLAUDE.md`'s own
/// "chessdb defers to shakmaty" section cites (`chebyshev_distance`), never
/// previously exposed as an independent fact. No FEN, no pipeline input.
pub struct SquareDistanceCmd;

impl PluginCommand for SquareDistanceCmd {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb square-distance"
    }

    fn description(&self) -> &str {
        "Square::distance(a, b) -- Chebyshev distance (max of file/rank distance, not Euclidean). No FEN, no pipeline input -- entirely position-independent."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .named("a", SyntaxShape::String, "first square", None)
            .named("b", SyntaxShape::String, "second square", None)
            .input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
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
        let (a, b) = two_square_flags(call, span, "a", "b")?;
        let result = square_distance(&a, &b, span)?;
        to_pipeline_data(&result, span)
    }
}
