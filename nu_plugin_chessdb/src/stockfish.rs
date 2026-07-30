//! Minimal UCI client for talking to a Stockfish subprocess. Extracted out of
//! `nnue_eval_cmd.rs`'s `PluginCommand::run` — spawning a process and
//! hand-rolling a UCI handshake is engine-communication logic, not Nu plugin
//! wiring, and it's unit-testable in isolation once it's not tangled up with
//! `Value`/`LabeledError` construction.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// A running Stockfish UCI subprocess with NNUE enabled. The handshake
/// (`uci` → `uciok`, enable NNUE, `isready` → `readyok`) happens once at
/// construction; `eval_fen` sends one position at a time and blocks for its
/// "Final evaluation" line. Sends `quit` and waits for the child on drop.
pub struct StockfishEngine {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl StockfishEngine {
    pub fn spawn(bin: &str) -> Result<Self, String> {
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot spawn Stockfish ({bin}): {e}"))?;

        let mut stdin = child.stdin.take().ok_or("no stdin for stockfish")?;
        let stdout = child.stdout.take().ok_or("no stdout for stockfish")?;
        let mut reader = BufReader::new(stdout);

        writeln!(stdin, "uci").map_err(|e| format!("write error: {e}"))?;
        stdin.flush().map_err(|e| format!("flush error: {e}"))?;
        Self::read_until(&mut reader, "uciok")?;

        writeln!(stdin, "setoption name Use NNUE value true").map_err(|e| format!("write error: {e}"))?;
        writeln!(stdin, "isready").map_err(|e| format!("write error: {e}"))?;
        stdin.flush().map_err(|e| format!("flush error: {e}"))?;
        Self::read_until(&mut reader, "readyok")?;

        Ok(Self { child, stdin, reader })
    }

    fn read_until(reader: &mut BufReader<ChildStdout>, marker: &str) -> Result<(), String> {
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| format!("read error: {e}"))?;
            if line.trim() == marker { return Ok(()); }
        }
    }

    /// Evaluate one FEN. Returns `Ok(None)` if Stockfish printed no "Final
    /// evaluation" line within 100 lines of output (matches prior behavior:
    /// the caller falls back to a 0 score rather than treating this as fatal).
    pub fn eval_fen(&mut self, fen: &str) -> Result<Option<i64>, String> {
        writeln!(self.stdin, "position fen {fen}").map_err(|e| format!("write error: {e}"))?;
        writeln!(self.stdin, "eval").map_err(|e| format!("write error: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush error: {e}"))?;

        for _ in 0..100 {
            let mut line = String::new();
            self.reader.read_line(&mut line).map_err(|e| format!("read error: {e}"))?;
            if line.starts_with("Final evaluation") {
                return Ok(parse_eval_line(&line));
            }
        }
        eprintln!("Warning: no Final eval for FEN after 100 lines (fen={}...)", &fen[..fen.len().min(40)]);
        Ok(None)
    }
}

impl Drop for StockfishEngine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.wait();
    }
}

fn parse_eval_line(line: &str) -> Option<i64> {
    let after = line.split("Final evaluation").nth(1)?;
    // after: "       +6.60 (white side) [with scaled NNUE, ...]"
    let value_str = after.split_whitespace().next()?;
    match value_str {
        "none" => None,
        s if s.starts_with('+') || s.starts_with('-') => {
            let f: f64 = s.parse().ok()?;
            Some((f * 100.0).round() as i64)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positive_eval() {
        let line = "Final evaluation       +6.60 (white side) [with scaled NNUE, ...]\n";
        assert_eq!(parse_eval_line(line), Some(660));
    }

    #[test]
    fn parses_negative_eval() {
        let line = "Final evaluation       -1.25 (white side)\n";
        assert_eq!(parse_eval_line(line), Some(-125));
    }

    #[test]
    fn parses_none_eval() {
        let line = "Final evaluation       none (in check)\n";
        assert_eq!(parse_eval_line(line), None);
    }

    #[test]
    fn ignores_line_without_marker() {
        assert_eq!(parse_eval_line("bestmove e2e4\n"), None);
    }
}
