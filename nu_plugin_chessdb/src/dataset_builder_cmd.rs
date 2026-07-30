use nu_plugin::{EvaluatedCall, PluginCommand};
use nu_protocol::{Category, LabeledError, PipelineData, Signature, Type, Value};

use bulletformat::{ChessBoard, BulletFormat};
use std::io::BufWriter;
use std::fs::File;
use std::path::Path;
use std::io::Write;
use std::collections::HashSet;

use crate::ChessdbPlugin;
use shakmaty::zobrist::ZobristHash;

pub struct DatasetBuilder;

impl PluginCommand for DatasetBuilder {
    type Plugin = ChessdbPlugin;

    fn name(&self) -> &str { "chessdb bullet-build" }
    fn description(&self) -> &str { "Build bulletformat .bin shards for bullet training from a Nushell table of positions" }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![(Type::List(Box::new(Type::Record(vec![].into()))), Type::Nothing)])
            .named("out-dir", nu_protocol::SyntaxShape::String, "output directory for shards", Some('o'))
            .named("shard-size", nu_protocol::SyntaxShape::Int, "samples per shard", Some('s'))
            .named("max-unique-bytes", nu_protocol::SyntaxShape::Int, "memory budget for unique zobrist set (bytes)", None)
            .named("bytes-per-entry", nu_protocol::SyntaxShape::Int, "estimated bytes per zobrist entry", None)
            .named("min-elo", nu_protocol::SyntaxShape::Int, "minimum elo for inclusion (min white/black)", None)
            .category(Category::Custom(crate::PLUGIN_CATEGORY.into()))
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &nu_plugin::EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;

        let out_dir: Option<String> = call.get_flag("out-dir").map_err(|e| LabeledError::new(e.to_string()))?;
        let shard_size: i64 = call.get_flag("shard-size").unwrap_or(Some(50000)).unwrap_or(50000);
        let max_unique_bytes: i64 = call.get_flag("max-unique-bytes").unwrap_or(Some(100_000_000)).unwrap_or(100_000_000);
        let bytes_per_entry: i64 = call.get_flag("bytes-per-entry").unwrap_or(Some(48)).unwrap_or(48);
        let min_elo: Option<i64> = call.get_flag("min-elo").unwrap_or(None);

        let out_dir = out_dir.ok_or_else(|| LabeledError::new("--out-dir is required"))?;
        std::fs::create_dir_all(&out_dir).map_err(|e| LabeledError::new(format!("could not create out dir: {}", e)))?;

        let max_unique_count: usize = if bytes_per_entry>0 { (max_unique_bytes as usize) / (bytes_per_entry as usize) } else { 5_000_000 };

        // Read pipeline input as list of records
        let input_value = input.into_value(span).map_err(|e| LabeledError::new(format!("input error: {}", e)))?;
        let rows = match input_value {
            Value::List { vals, .. } => vals,
            _ => return Err(LabeledError::new("Expected a list of records as input")),
        };

        let mut meta = ShardMeta::with_capacity(shard_size as usize);

        let mut shard_idx = 0usize;
        let mut count_in_shard = 0usize;
        let mut zobrist_set: HashSet<u64> = HashSet::new();
        let mut total_positions_seen: usize = 0usize;

        for val in rows.into_iter() {
            // expect a record value and convert
            let rec = match val.into_record() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("skipping non-record input: {}", e);
                    continue;
                }
            };

            let fen: String = rec.get("fen").and_then(|v| v.as_str().ok()).unwrap_or("").to_string();
            let zob: String = rec.get("zobrist").and_then(|v| v.as_str().ok()).unwrap_or("").to_string();
            let source_game_id: String = rec.get("source_game_id").and_then(|v| v.as_str().ok()).unwrap_or("").to_string();
            let ply: i64 = rec.get("ply").and_then(|v| v.as_int().ok()).unwrap_or(0) as i64;
            let white_elo: i64 = rec.get("white_elo").and_then(|v| v.as_int().ok()).unwrap_or(0) as i64;
            let black_elo: i64 = rec.get("black_elo").and_then(|v| v.as_int().ok()).unwrap_or(0) as i64;
            let result: String = rec.get("result").and_then(|v| v.as_str().ok()).unwrap_or("").to_string();

            if let Some(min) = min_elo {
                if white_elo < min || black_elo < min { continue; }
            }

            // parse fen
            let parsed = match shakmaty::fen::Fen::from_ascii(fen.as_bytes()) { Ok(f)=>f, Err(_) => { eprintln!("bad fen: {}", fen); continue } };
            let chess: shakmaty::Chess = match parsed.into_position(shakmaty::CastlingMode::Standard) { Ok(c)=>c, Err(_) => { eprintln!("failed to into_position: {}", fen); continue } };

            // compute zob u64
            let zob_u64 = if !zob.is_empty() {
                if let Ok(zv) = u64::from_str_radix(zob.trim_start_matches("0x"), 16) { zv }
                else if let Ok(zv) = u64::from_str_radix(&zob, 16) { zv }
                else { chess.zobrist_hash::<shakmaty::zobrist::Zobrist64>(shakmaty::EnPassantMode::Legal).0 }
            } else {
                chess.zobrist_hash::<shakmaty::zobrist::Zobrist64>(shakmaty::EnPassantMode::Legal).0
            };

            total_positions_seen += 1;
            if !zobrist_set.contains(&zob_u64) {
                if zobrist_set.len() >= max_unique_count {
                    // flush
                    if count_in_shard>0 {
                        write_shard(&out_dir, shard_idx, &meta).map_err(|e| LabeledError::new(format!("write shard error: {}", e)))?;
                    }
                    let sentinel = serde_json::json!({"reason":"unique_limit_reached","unique_count":zobrist_set.len(),"total_positions_seen":total_positions_seen,"shard_idx":shard_idx});
                    let sfile = Path::new(&out_dir).join("unique_limit_reached.json");
                    let mut sf = File::create(&sfile).map_err(|e| LabeledError::new(format!("could not write sentinel: {}", e)))?;
                    sf.write_all(serde_json::to_string_pretty(&sentinel).unwrap().as_bytes()).map_err(|e| LabeledError::new(format!("could not write sentinel: {}", e)))?;
                    return Ok(PipelineData::empty());
                } else {
                    zobrist_set.insert(zob_u64);
                }
            }

            // Only recognized result strings produce a usable label downstream
            // (write_shard's win/draw/loss score); skip anything else here,
            // at the source, rather than writing an unlabeled row.
            match result.as_str() {
                "1-0" | "0-1" | "1/2-1/2" | "1/2" => {}
                _ => continue,
            }

            meta.zobrist.push(format!("{:016x}", zob_u64));
            meta.game_id.push(source_game_id);
            meta.ply.push(ply);
            meta.white_elo.push(white_elo);
            meta.black_elo.push(black_elo);
            meta.result.push(result);
            meta.fen.push(fen);

            count_in_shard += 1;
            if count_in_shard as i64 >= shard_size {
                write_shard(&out_dir, shard_idx, &meta).map_err(|e| LabeledError::new(format!("write shard error: {}", e)))?;
                shard_idx +=1; count_in_shard=0;
                meta.clear();
            }
        }

        if count_in_shard>0 {
            write_shard(&out_dir, shard_idx, &meta).map_err(|e| LabeledError::new(format!("write shard error: {}", e)))?;
        }

        Ok(PipelineData::empty())
    }
}

/// Per-position metadata collected for one shard. `write_shard` derives the
/// actual bullet-format training label (score/result) from `fen`/`result` at
/// write time — nothing else here is a training feature, just provenance
/// for the `.meta.json` sidecar.
#[derive(Default)]
struct ShardMeta {
    zobrist: Vec<String>,
    game_id: Vec<String>,
    ply: Vec<i64>,
    white_elo: Vec<i64>,
    black_elo: Vec<i64>,
    result: Vec<String>,
    fen: Vec<String>,
}

impl ShardMeta {
    fn with_capacity(n: usize) -> Self {
        Self {
            zobrist: Vec::with_capacity(n),
            game_id: Vec::with_capacity(n),
            ply: Vec::with_capacity(n),
            white_elo: Vec::with_capacity(n),
            black_elo: Vec::with_capacity(n),
            result: Vec::with_capacity(n),
            fen: Vec::with_capacity(n),
        }
    }

    fn clear(&mut self) {
        self.zobrist.clear();
        self.game_id.clear();
        self.ply.clear();
        self.white_elo.clear();
        self.black_elo.clear();
        self.result.clear();
        self.fen.clear();
    }
}

fn write_shard(out_dir: &str, shard_idx: usize, meta: &ShardMeta) -> anyhow::Result<()> {
    let n = meta.result.len();

    // bulletformat's `ChessBoard: FromStr` expects "fen | score | result" with
    // score/result relative to White — it applies its own side-to-move flip
    // internally (see FromStr::from_str: parses these at face value, then
    // negates score and does `2 - result` when the FEN's stm is black).
    // Passing already-side-relative values here would get double-flipped.
    let mut boards: Vec<ChessBoard> = Vec::with_capacity(n);
    for i in 0..n {
        let fen = &meta.fen[i];
        let result = meta.result[i].as_str();
        let result_float = match result {
            "1-0" => 1.0_f32,
            "0-1" => 0.0_f32,
            _ => 0.5_f32,
        };
        let score: i16 = match result {
            "1-0" => 10000,
            "0-1" => -10000,
            _ => 0,
        };
        let line = format!("{} | {} | {}", fen, score, result_float);
        let cb: ChessBoard = line.parse()
            .map_err(|e| anyhow::anyhow!("could not parse chessboard from fen: {}", e))?;
        boards.push(cb);
    }

    let shard_name = format!("shard-{:05}.bin", shard_idx);
    let shard_path = Path::new(out_dir).join(&shard_name);
    let f = File::create(&shard_path)?;
    let mut writer = BufWriter::new(f);
    ChessBoard::write_to_bin(&mut writer, &boards)?;

    // Write metadata JSON
    let meta_name = format!("shard-{:05}.meta.json", shard_idx);
    let meta_path = Path::new(out_dir).join(&meta_name);
    let mut meta_f = File::create(&meta_path)?;

    let meta_json = serde_json::json!({
        "n": n,
        "shard_file": shard_name,
        "zobrist": meta.zobrist,
        "source_game_id": meta.game_id,
        "ply": meta.ply,
        "white_elo": meta.white_elo,
        "black_elo": meta.black_elo,
        "result": meta.result
    });
    meta_f.write_all(serde_json::to_string_pretty(&meta_json)?.as_bytes())?;

    println!("wrote shard {} ({} samples)", shard_name, n);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds the exact "fen | score | result" line write_shard constructs,
    /// and confirms bulletformat::ChessBoard's own side-to-move flip produces
    /// the label a training pipeline would actually expect: positive/winning
    /// from the perspective of whoever is to move in that FEN, not always
    /// from White's.
    fn label_for(fen: &str, result: &str) -> (i16, u8) {
        let result_float: f32 = match result {
            "1-0" => 1.0,
            "0-1" => 0.0,
            _ => 0.5,
        };
        let score: i16 = match result {
            "1-0" => 10000,
            "0-1" => -10000,
            _ => 0,
        };
        let line = format!("{fen} | {score} | {result_float}");
        let cb: ChessBoard = line.parse().expect("valid bulletformat line");
        (cb.score, cb.result)
    }

    const WHITE_TO_MOVE: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const BLACK_TO_MOVE: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";

    #[test]
    fn white_win_white_to_move_is_positive() {
        let (score, result) = label_for(WHITE_TO_MOVE, "1-0");
        assert_eq!(score, 10000);
        assert_eq!(result, 2); // win, from the mover's (White's) perspective
    }

    #[test]
    fn white_win_black_to_move_is_negative() {
        // White ultimately won, but it's Black's move in this FEN — the
        // label must be negative/losing from Black's perspective, not a
        // blanket "White win" score copied in unchanged.
        let (score, result) = label_for(BLACK_TO_MOVE, "1-0");
        assert_eq!(score, -10000);
        assert_eq!(result, 0); // loss, from the mover's (Black's) perspective
    }

    #[test]
    fn draw_is_symmetric_regardless_of_side_to_move() {
        assert_eq!(label_for(WHITE_TO_MOVE, "1/2-1/2"), (0, 1));
        assert_eq!(label_for(BLACK_TO_MOVE, "1/2-1/2"), (0, 1));
    }
}
