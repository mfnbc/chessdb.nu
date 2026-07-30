use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value, record};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

use crate::ChessdbPlugin;
use crate::PLUGIN_CATEGORY;
use crate::core::pgn_to_fens;
use crate::eval::analyze_fen_with_engine_score;
use crate::game_parse::parse_game;

struct PendingPos {
    zobrist: String,
    fen: String,
    board_pieces: String,
    hugm_score: i64,
    hugm_eval_arr: String,
    state_id: u16,
    mate_in_1: i64,
    is_checkmate: i64,
}

/// Lightweight FEN entry collected during game parsing.
/// Evaluated in batch after all games are parsed.
struct FenToEval {
    zobrist: String,
    fen: String,
    board_pieces: String,
}

pub struct ProcessCorpus;

impl PluginCommand for ProcessCorpus {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str {
        "chessdb process-corpus"
    }

    fn description(&self) -> &str {
        "Takes a JSON array of games, parses PGNs into structured DataFrames (games, positions, moves)."
    }

    fn signature(&self) -> Signature {
        Signature::build(PluginCommand::name(self))
            .input_output_type(Type::String, Type::Any)
            .named(
                "username",
                nu_protocol::SyntaxShape::String,
                "The username of the player to determine result relative to them",
                Some('u')
            )
            .category(Category::Custom(PLUGIN_CATEGORY.to_string()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &nu_plugin::EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let input_str = input.into_value(span)?.into_string()?;

        let json_data: JsonValue = serde_json::from_str(&input_str).map_err(|e| {
            LabeledError::new(format!("Failed to parse JSON: {}", e))
        })?;

        let games_array = match json_data.as_array() {
            Some(arr) => arr,
            None => return Err(LabeledError::new("Input must be a JSON array")),
        };

        let mut out_games = Vec::new();
        let mut out_moves = Vec::new();

        // Phase 1: collect FENs during game parsing (no evaluation yet)
        let mut fens_to_eval: Vec<FenToEval> = Vec::new();
        let mut unique_positions = HashSet::new();

        let username: Option<String> = call.get_flag("username")?;
        for g in games_array {
            let parsed = parse_game(g, username.as_deref());

            if let Some(pgn) = &parsed.pgn {
                if let Ok(move_rows) = pgn_to_fens(pgn, span) {
                    let initial_zobrist = "463b96181691fc9c".to_string();
                    if unique_positions.insert(initial_zobrist.clone()) {
                        fens_to_eval.push(FenToEval {
                            zobrist: initial_zobrist.clone(),
                            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
                            board_pieces: "rnbqkbnrppppppppPPPPPPPPRNBQKBNR".to_string(),
                        });
                    }
                    let mut prev_zobrist: Option<String> = Some(initial_zobrist);

                    for m_row in move_rows {
                        let z_hex = m_row.zobrist.clone();

                        if unique_positions.insert(z_hex.clone()) {
                            let board_pieces: String = m_row.fen.chars().take_while(|c| *c != ' ').filter(|c| c.is_alphabetic()).collect();
                            fens_to_eval.push(FenToEval {
                                zobrist: z_hex.clone(),
                                fen: m_row.fen.clone(),
                                board_pieces,
                            });
                        }

                        if let Some(ref prev_z) = prev_zobrist {
                            let move_record = record! {
                                "game_id" => Value::int(parsed.game_id, span),
                                "position_id" => Value::string(prev_z, span),
                                "next_position_id" => Value::string(&z_hex, span),
                                "ply" => Value::int(m_row.ply as i64, span),
                                "move_number" => Value::int(m_row.move_number as i64, span),
                                "color" => Value::string(&m_row.color, span),
                                "san" => Value::string(&m_row.san, span),
                                "canonical_san" => Value::string(&m_row.canonical_san, span),
                                "uci" => Value::string(&m_row.uci, span),
                            };
                            out_moves.push(Value::record(move_record, span));
                        }

                        prev_zobrist = Some(z_hex);
                    }
                }
            }

            let game_record = record! {
                "game_id" => Value::int(parsed.game_id, span),
                "source" => Value::string(parsed.source, span),
                "source_game_id" => Value::string(parsed.source_game_id, span),
                "white" => Value::string(parsed.white, span),
                "black" => Value::string(parsed.black, span),
                "white_elo" => Value::int(parsed.white_elo, span),
                "black_elo" => Value::int(parsed.black_elo, span),
                "result" => Value::string(parsed.result, span),
                "played_at" => Value::string(parsed.played_at, span),
                "time_control" => Value::string(parsed.time_control, span),
                "eco" => Value::string(parsed.eco, span),
                "opening" => Value::string(parsed.opening, span),
            };
            out_games.push(Value::record(game_record, span));
        }


        // Phase 2: batch-evaluate all unique FENs in parallel (Rayon)
        use rayon::prelude::*;
        let eval_results: Vec<PendingPos> = fens_to_eval
            .par_iter()
            .map(|fe| {
                let (hugm_score, hugm_eval_arr, state_id, mate_in_1, is_checkmate) =
                    match analyze_fen_with_engine_score(&fe.fen, None, None) {
                        Ok(rec) => {
                            let sid = rec.sensor_report.state_id;
                            let mi1 = if rec.sensor_report.mate_in_1_exists { 1 } else { 0 };
                            let cm = if rec.legal.is_checkmate { 1 } else { 0 };
                            let arr = vec![
                                rec.groups.material.blended,
                                rec.groups.pawn_structure.blended,
                                rec.groups.piece_activity.blended,
                                rec.groups.king_safety.blended,
                                rec.groups.passed_pawns.blended,
                                rec.groups.development.blended,
                                rec.groups.vector_features.blended,
                                rec.groups.strategic.blended,
                                rec.groups.scaling.value,
                                rec.groups.drawishness.value,
                                rec.groups.override_.value,
                            ];
                            let json_str =
                                serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
                            (rec.final_score, json_str, sid, mi1, cm)
                        }
                        Err(_) => (0, "[]".to_string(), 0u16, 0, 0),
                    };
                PendingPos {
                    zobrist: fe.zobrist.clone(),
                    fen: fe.fen.clone(),
                    board_pieces: fe.board_pieces.clone(),
                    hugm_score,
                    hugm_eval_arr,
                    state_id,
                    mate_in_1,
                    is_checkmate,
                }
            })
            .collect();

        // Phase 3: materialize out_positions
        let mut out_positions = Vec::new();
        for p in eval_results.into_iter() {
            let pos_record = record! {
                "zobrist" => Value::string(&p.zobrist, span),
                "fen" => Value::string(&p.fen, span),
                "board_pieces" => Value::string(p.board_pieces, span),
                "hugm_score" => Value::int(p.hugm_score, span),
                "hugm_eval_arr" => Value::string(&p.hugm_eval_arr, span),
                "state_id" => Value::int(p.state_id as i64, span),
                "mate_in_1" => Value::int(p.mate_in_1, span),
                "is_checkmate" => Value::int(p.is_checkmate, span),
            };
            out_positions.push(Value::record(pos_record, span));
        }

        let final_record = record! {
            "games" => Value::list(out_games, span),
            "positions" => Value::list(out_positions, span),
            "moves" => Value::list(out_moves, span),
        };

        Ok(PipelineData::Value(Value::record(final_record, span), None))
    }
}