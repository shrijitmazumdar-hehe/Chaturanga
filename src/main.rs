mod eval;
mod search;
mod tt;

use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Color, Position};
use std::io::{self, BufRead, Write};
use std::time::Duration;

/// Parses the arguments of a UCI `position` command:
///   startpos [moves m1 m2 ...]
///   fen <6 FEN fields> [moves m1 m2 ...]
fn parse_position(args: &[&str]) -> Option<Chess> {
    let (mut pos, next) = match args.first() {
        Some(&"startpos") => (Chess::default(), 1),
        Some(&"fen") => {
            let end = args.iter().position(|&t| t == "moves").unwrap_or(args.len());
            let fen: Fen = args[1..end].join(" ").parse().ok()?;
            let pos: Chess = fen.into_position(CastlingMode::Standard).ok()?;
            (pos, end)
        }
        _ => return None,
    };

    if args.get(next) == Some(&"moves") {
        for token in &args[next + 1..] {
            let uci: UciMove = token.parse().ok()?;
            let mv = uci.to_move(&pos).ok()?; // fails if the move is illegal here
            pos.play_unchecked(mv);
        }
    }
    Some(pos)
}

/// Turns the arguments of `go` into (max depth, time limit).
fn go_limits(tokens: &[&str], white_to_move: bool) -> (i32, Option<Duration>) {
    let get = |name: &str| -> Option<u64> {
        let i = tokens.iter().position(|&t| t == name)?;
        tokens.get(i + 1)?.parse().ok()
    };

    if let Some(d) = get("depth") {
        return ((d as i32).min(search::MAX_DEPTH), None);
    }
    if let Some(ms) = get("movetime") {
        let ms = ms.saturating_sub(30); // safety margin
        return (search::MAX_DEPTH, Some(Duration::from_millis(ms)));
    }
    let (time, inc) = if white_to_move {
        (get("wtime"), get("winc"))
    } else {
        (get("btime"), get("binc"))
    };
    if let Some(t) = time {
        let moves_left = get("movestogo").unwrap_or(30).max(1);
        let budget = (t / moves_left + inc.unwrap_or(0) / 2).min(t / 2);
        return (search::MAX_DEPTH, Some(Duration::from_millis(budget.max(10))));
    }
    (search::MAX_DEPTH, Some(Duration::from_millis(1000)))
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut pos = Chess::default();
    let mut tt = tt::Tt::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let tokens: Vec<&str> = line.split_whitespace().collect();

        match tokens.first().copied() {
            Some("uci") => {
                println!("id name Chaturanga 0.2");
                println!("id author shrijitm");
                println!("uciok");
            }
            Some("isready") => println!("readyok"),
            Some("ucinewgame") => {
                pos = Chess::default();
                tt.clear();
            }
            Some("position") => match parse_position(&tokens[1..]) {
                Some(p) => pos = p,
                None => eprintln!("error: could not parse position"),
            },
            Some("go") => {
                let (depth, limit) = go_limits(&tokens, pos.turn() == Color::White);
                let (best, _score) = search::search(&pos, depth, limit, &mut tt);
                match best {
                    Some(m) => println!("bestmove {}", m.to_uci(CastlingMode::Standard)),
                    None => println!("bestmove 0000"),
                }
            }
            // Custom debug command (not part of UCI)
            Some("legalmoves") => {
                let moves: Vec<String> = pos
                    .legal_moves()
                    .iter()
                    .map(|m| m.to_uci(CastlingMode::Standard).to_string())
                    .collect();
                println!("{} legal moves: {}", moves.len(), moves.join(" "));
            }
            Some("perft") => {
                let d: u32 = tokens.get(1).and_then(|t| t.parse().ok()).unwrap_or(1);
                println!("nodes {}", perft(&pos, d));
            }
            Some("quit") => break,
            _ => {} // unknown commands are ignored, as UCI requires
        }

        stdout.flush().unwrap();
    }
}

/// Counts all legal move sequences of the given length (debug/testing only).
fn perft(pos: &Chess, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(m);
        nodes += perft(&next, depth - 1);
    }
    nodes
}
