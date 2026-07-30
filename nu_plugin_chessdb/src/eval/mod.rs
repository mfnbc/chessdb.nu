//! HUGM evaluation pipeline — data flow and naming conventions.
//!
//! ```text
//! board
//!   │
//!   ▼
//! compute_groups (position.rs)  ──►  EvalGroups
//!   │                                  internal, legacy scoring engine.
//!   │                                  GroupValue.{mg,eg,blended} are real
//!   │                                  typed fields — fine to read. But
//!   │                                  GroupValue.terms: Map<String, Value>
//!   │                                  is an untyped scratch bag, private
//!   │                                  to position.rs's own bookkeeping.
//!   │                                  Nothing outside position.rs's one
//!   │                                  conversion boundary below reads it.
//!   ▼
//! build_sensor_report (position.rs)  ──►  SensorReport
//!   │                                       the one typed boundary. Every
//!   │                                       downstream consumer (concepts,
//!   │                                       coach_derive_cmd, explanations)
//!   │                                       reads this, never EvalGroups.
//!   ▼
//! extract_concepts (concepts.rs)  ──►  Vec<Concept>
//!   ▼
//! rank_issues_for_position /
//! rank_issues_for_player (concepts.rs)  ──►  Vec<GatedIssue>
//!                                              the actual coaching output.
//! ```
//!
//! ## Two naming families feed `build_sensor_report`, deliberately
//!
//! - `detect_X(board, color) -> (count, raw_examples)` + `X_to_typed(board,
//!   raw_examples) -> Vec<TypedConcept>` — used for tactical concepts (pins,
//!   forks, skewers, discovered attacks). Two steps because detection is
//!   comparatively expensive and its raw `Square`-tuple output is cached in
//!   `TacticalRaw` by `tactical_score` so it's computed once and converted
//!   to typed form only when needed, not re-run.
//! - `extract_X(board)` or `extract_X(groups, ...) -> Vec<TypedConcept>` —
//!   used for positional/material concepts (outposts, passed pawns, pawn
//!   majority, center control, ...) that are cheap enough to detect and
//!   type in one step, either scanning `board` directly or reading the
//!   handful of already-computed `EvalGroups` scalar/synthesized fields
//!   needed to derive them.
//!
//! This is a real structural difference (cached-then-converted vs.
//! direct-scan-and-build), not inconsistent naming — read it that way.

pub mod concept_types;
pub mod concepts;
pub mod position;
pub mod sensor;
pub mod threat_graph;

pub use position::{analyze_fen, analyze_fen_with_engine_score, compute_phase, compute_groups, build_sensor_report, PositionRecord, render_explanations, render_structured_explanations, set_weights_from_file};
pub use concepts::{encode_state, decode_state_id, SensorTier, attenuation, extract_concepts, rank_issues_for_position, StateVector};
pub use concept_types::{GatedIssue, Side};
