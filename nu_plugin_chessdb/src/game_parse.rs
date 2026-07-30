//! Parses one chess.com/lichess game JSON object (as found in the array
//! passed to `chessdb process-corpus`) into typed fields. Extracted out of
//! `process_corpus.rs`'s `PluginCommand::run` — none of this needs Nu's
//! `Value`/`LabeledError` types, so it can be tested directly against JSON
//! fixtures instead of only through the full plugin protocol.

use chrono::DateTime;
use serde_json::Value as JsonValue;

pub struct ParsedGame {
    pub game_id: i64,
    pub source: String,
    pub source_game_id: String,
    pub white: String,
    pub black: String,
    pub white_elo: i64,
    pub black_elo: i64,
    pub result: String,
    pub played_at: String,
    pub time_control: String,
    pub eco: String,
    pub opening: String,
    pub pgn: Option<String>,
}

/// `username`, if given, determines which side's `result` field becomes the
/// game's overall result (see `result_relative_to`).
pub fn parse_game(g: &JsonValue, username: Option<&str>) -> ParsedGame {
    let url = g.get("url").and_then(|v| v.as_str()).unwrap_or("unknown");
    let (game_id, mut source_game_id) = game_identity(url);
    // Prefer an explicit id field over whatever was derived from the URL.
    if let Some(id_str) = g.get("id").and_then(|v| v.as_str()) {
        if !id_str.is_empty() { source_game_id = id_str.to_string(); }
    }

    let source = if url.contains("chess.com") { "chesscom" } else { "lichess" }.to_string();
    let white = g.get("white").and_then(|w| w.get("username")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let black = g.get("black").and_then(|b| b.get("username")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let white_elo = g.get("white").and_then(|w| w.get("rating")).and_then(|v| v.as_i64()).unwrap_or(0);
    let black_elo = g.get("black").and_then(|b| b.get("rating")).and_then(|v| v.as_i64()).unwrap_or(0);
    let time_control = g.get("time_control").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

    let played_at = parse_played_at(g);
    let result = result_relative_to(g, username, &white, &black);
    let (eco, opening) = extract_eco_opening(g);
    let pgn = g.get("pgn").and_then(|v| v.as_str()).map(|s| s.to_string());

    ParsedGame {
        game_id, source, source_game_id, white, black, white_elo, black_elo,
        result, played_at, time_control, eco, opening, pgn,
    }
}

/// `(game_id_int, source_game_id)` from the trailing path segment of a game
/// URL — numeric if it parses as one, kept as the raw segment either way.
fn game_identity(url: &str) -> (i64, String) {
    let mut game_id = 0i64;
    let mut source_game_id = url.to_string();
    if let Some(last_slash) = url.rfind('/') {
        let tail = &url[last_slash + 1..];
        source_game_id = tail.to_string();
        if let Ok(parsed_id) = tail.parse::<i64>() {
            game_id = parsed_id;
        }
    }
    (game_id, source_game_id)
}

/// chess.com uses `end_time` (unix seconds); lichess uses `lastMoveAt`
/// (unix ms) or `createdAt` (ms or seconds — disambiguated by magnitude).
/// Each field is checked in order; once a field is *present* the chain
/// commits to that branch and returns "unknown" if it turns out invalid,
/// rather than falling through to the next field (matches original
/// behavior — not obviously correct, but not this extraction's job to fix).
fn parse_played_at(g: &JsonValue) -> String {
    if let Some(end_time) = g.get("end_time").and_then(|v| v.as_i64()) {
        if end_time > 0 {
            if let Some(dt) = DateTime::from_timestamp(end_time, 0) {
                return dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            }
        }
        return "unknown".to_string();
    }
    if let Some(last_move_at) = g.get("lastMoveAt").and_then(|v| v.as_i64()) {
        if last_move_at > 0 {
            let secs = last_move_at / 1000;
            if let Some(dt) = DateTime::from_timestamp(secs, 0) {
                return dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            }
        }
        return "unknown".to_string();
    }
    if let Some(created_at) = g.get("createdAt").and_then(|v| v.as_i64()) {
        let secs = if created_at > 1_000_000_000_000 { created_at / 1000 } else { created_at };
        if let Some(dt) = DateTime::from_timestamp(secs, 0) {
            return dt.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        }
        return "unknown".to_string();
    }
    if let Some(played_str) = g.get("played_at").and_then(|v| v.as_str()) {
        return played_str.to_string();
    }
    "unknown".to_string()
}

/// Without a username, the game's own top-level `result` field is used.
/// With one, the result is taken from whichever side (`white`/`black`)
/// matches — "unknown" if neither does.
fn result_relative_to(g: &JsonValue, username: Option<&str>, white_name: &str, black_name: &str) -> String {
    match username {
        Some(uname) if white_name.eq_ignore_ascii_case(uname) => {
            g.get("white").and_then(|w| w.get("result")).and_then(|v| v.as_str()).unwrap_or("unknown").to_string()
        }
        Some(uname) if black_name.eq_ignore_ascii_case(uname) => {
            g.get("black").and_then(|b| b.get("result")).and_then(|v| v.as_str()).unwrap_or("unknown").to_string()
        }
        Some(_) => "unknown".to_string(),
        None => g.get("result").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
    }
}

/// Confusingly, in the source data the PGN's embedded `[ECO "..."]` tag is
/// this game's eco *code*, while the separate top-level `eco` JSON field is
/// actually a URL whose last path segment is the opening *name* — preserved
/// as-is (that's the field mapping the rest of the pipeline expects), not
/// renamed here.
fn extract_eco_opening(g: &JsonValue) -> (String, String) {
    let mut eco = "unknown".to_string();
    let mut opening = "unknown".to_string();
    if let Some(pgn) = g.get("pgn").and_then(|v| v.as_str()) {
        if let Some(eco_start) = pgn.find("[ECO \"") {
            let eco_sub = &pgn[eco_start + 6..];
            if let Some(eco_end) = eco_sub.find("\"]") {
                eco = eco_sub[..eco_end].to_string();
            }
        }
        if let Some(eco_url) = g.get("eco").and_then(|v| v.as_str()) {
            if let Some(last_slash) = eco_url.rfind('/') {
                opening = eco_url[last_slash + 1..].to_string();
            }
        }
    }
    (eco, opening)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn game_identity_numeric_url() {
        assert_eq!(game_identity("https://www.chess.com/game/live/123456789"), (123456789, "123456789".to_string()));
    }

    #[test]
    fn game_identity_non_numeric_url() {
        assert_eq!(game_identity("https://lichess.org/abcd1234"), (0, "abcd1234".to_string()));
    }

    #[test]
    fn played_at_prefers_end_time_over_others() {
        let g = json!({"end_time": 1700000000, "lastMoveAt": 1, "createdAt": 1});
        assert_eq!(parse_played_at(&g), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn played_at_invalid_end_time_does_not_fall_through() {
        // end_time present but 0 -> commits to "unknown", doesn't try lastMoveAt
        let g = json!({"end_time": 0, "lastMoveAt": 1700000000000i64});
        assert_eq!(parse_played_at(&g), "unknown");
    }

    #[test]
    fn played_at_lichess_last_move_at_ms() {
        let g = json!({"lastMoveAt": 1700000000000i64});
        assert_eq!(parse_played_at(&g), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn played_at_created_at_disambiguates_ms_vs_seconds() {
        let g_secs = json!({"createdAt": 1700000000});
        let g_ms = json!({"createdAt": 1700000000000i64});
        assert_eq!(parse_played_at(&g_secs), "2023-11-14T22:13:20Z");
        assert_eq!(parse_played_at(&g_ms), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn played_at_falls_back_to_string_field() {
        let g = json!({"played_at": "some-string-value"});
        assert_eq!(parse_played_at(&g), "some-string-value");
    }

    #[test]
    fn played_at_defaults_unknown() {
        assert_eq!(parse_played_at(&json!({})), "unknown");
    }

    #[test]
    fn result_without_username_uses_top_level_field() {
        let g = json!({"result": "win"});
        assert_eq!(result_relative_to(&g, None, "alice", "bob"), "win");
    }

    #[test]
    fn result_with_username_matching_white() {
        let g = json!({"white": {"result": "win"}, "black": {"result": "checkmated"}});
        assert_eq!(result_relative_to(&g, Some("Alice"), "alice", "bob"), "win");
    }

    #[test]
    fn result_with_username_matching_black() {
        let g = json!({"white": {"result": "win"}, "black": {"result": "resigned"}});
        assert_eq!(result_relative_to(&g, Some("bob"), "alice", "bob"), "resigned");
    }

    #[test]
    fn result_with_username_matching_neither() {
        let g = json!({"white": {"result": "win"}, "black": {"result": "resigned"}});
        assert_eq!(result_relative_to(&g, Some("carol"), "alice", "bob"), "unknown");
    }

    #[test]
    fn eco_opening_extracted_from_pgn_and_url() {
        let g = json!({
            "pgn": "[ECO \"C50\"]\n1. e4 e5",
            "eco": "https://www.chess.com/openings/Italian-Game",
        });
        assert_eq!(extract_eco_opening(&g), ("C50".to_string(), "Italian-Game".to_string()));
    }

    #[test]
    fn eco_opening_defaults_without_pgn() {
        assert_eq!(extract_eco_opening(&json!({})), ("unknown".to_string(), "unknown".to_string()));
    }
}
