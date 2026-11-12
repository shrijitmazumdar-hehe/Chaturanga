use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Color, Move, Piece, Position, Role};
use std::io::{self, BufRead, Write};

const INF: i32 = 32_000;
const MATE: i32 = 30_000;
const MAX_PLY: i32 = 64;
const DEFAULT_DEPTH: i32 = 4;

// ---------------------------------------------------------------- evaluation

// Order: pawn, knight, bishop, rook, queen, king
const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];

// Tables are written from White's point of view, rank 8 first.
#[rustfmt::skip]
const PST: [[i32; 64]; 6] = [
    // pawn
    [  0,  0,  0,  0,  0,  0,  0,  0,
      50, 50, 50, 50, 50, 50, 50, 50,
      10, 10, 20, 30, 30, 20, 10, 10,
       5,  5, 10, 25, 25, 10,  5,  5,
       0,  0,  0, 20, 20,  0,  0,  0,
       5, -5,-10,  0,  0,-10, -5,  5,
       5, 10, 10,-20,-20, 10, 10,  5,
       0,  0,  0,  0,  0,  0,  0,  0],
    // knight
    [-50,-40,-30,-30,-30,-30,-40,-50,
     -40,-20,  0,  0,  0,  0,-20,-40,
     -30,  0, 10, 15, 15, 10,  0,-30,
     -30,  5, 15, 20, 20, 15,  5,-30,
     -30,  0, 15, 20, 20, 15,  0,-30,
     -30,  5, 10, 15, 15, 10,  5,-30,
     -40,-20,  0,  5,  5,  0,-20,-40,
     -50,-40,-30,-30,-30,-30,-40,-50],
    // bishop
    [-20,-10,-10,-10,-10,-10,-10,-20,
     -10,  0,  0,  0,  0,  0,  0,-10,
     -10,  0,  5, 10, 10,  5,  0,-10,
     -10,  5,  5, 10, 10,  5,  5,-10,
     -10,  0, 10, 10, 10, 10,  0,-10,
     -10, 10, 10, 10, 10, 10, 10,-10,
     -10,  5,  0,  0,  0,  0,  5,-10,
     -20,-10,-10,-10,-10,-10,-10,-20],
    // rook
    [  0,  0,  0,  0,  0,  0,  0,  0,
       5, 10, 10, 10, 10, 10, 10,  5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
       0,  0,  0,  5,  5,  0,  0,  0],
    // queen
    [-20,-10,-10, -5, -5,-10,-10,-20,
     -10,  0,  0,  0,  0,  0,  0,-10,
     -10,  0,  5,  5,  5,  5,  0,-10,
      -5,  0,  5,  5,  5,  5,  0, -5,
       0,  0,  5,  5,  5,  5,  0, -5,
     -10,  5,  5,  5,  5,  5,  0,-10,
     -10,  0,  5,  0,  0,  0,  0,-10,
     -20,-10,-10, -5, -5,-10,-10,-20],
    // king (middlegame)
    [-30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -20,-30,-30,-40,-40,-30,-30,-20,
     -10,-20,-20,-20,-20,-20,-20,-10,
      20, 20,  0,  0,  0,  0, 20, 20,
      20, 30, 10,  0,  0, 10, 30, 20],
];

fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

/// Score in centipawns from the point of view of the side to move.
fn evaluate(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut score = 0; // from White's point of view
    for color in Color::ALL {
        for role in Role::ALL {
            let r = role_index(role);
            for sq in board.by_piece(Piece { color, role }) {
                let s = sq as usize; // a1 = 0 ... h8 = 63
                let (idx, sign) = match color {
                    Color::White => (s ^ 56, 1), // flip rank to match table layout
                    Color::Black => (s, -1),
                };
                score += sign * (VALUE[r] + PST[r][idx]);
            }
        }
    }
    match pos.turn() {
        Color::White => score,
        Color::Black => -score,
    }
}

// -------------------------------------------------------------------- search

/// Simple move ordering: captures first (most valuable victim, least valuable
/// attacker), then promotions, then quiet moves.
fn move_order_score(m: &Move) -> i32 {
    let mut s = 0;
    if let Some(victim) = m.capture() {
        s += 10_000 + 10 * VALUE[role_index(victim)] - VALUE[role_index(m.role())];
    }
    if let Some(promo) = m.promotion() {
        s += 9_000 + VALUE[role_index(promo)];
    }
    s
}

fn ordered_moves(pos: &Chess) -> Vec<Move> {
    let mut moves: Vec<Move> = pos.legal_moves().into_iter().collect();
    moves.sort_by_key(|m| std::cmp::Reverse(move_order_score(m)));
    moves
}

fn quiescence(pos: &Chess, mut alpha: i32, beta: i32, ply: i32, nodes: &mut u64) -> i32 {
    *nodes += 1;
    if ply >= MAX_PLY {
        return evaluate(pos);
    }

    let moves: Vec<Move> = if pos.is_check() {
        // In check there is no "stand pat": every legal move is an evasion.
        let moves = ordered_moves(pos);
        if moves.is_empty() {
            return -MATE + ply; // checkmated
        }
        moves
    } else {
        let stand_pat = evaluate(pos);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        ordered_moves(pos)
            .into_iter()
            .filter(|m| m.is_capture() || m.is_promotion())
            .collect()
    };

    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(m);
        let score = -quiescence(&next, -beta, -alpha, ply + 1, nodes);
        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    alpha
}

fn negamax(pos: &Chess, depth: i32, mut alpha: i32, beta: i32, ply: i32, nodes: &mut u64) -> i32 {
    if depth <= 0 {
        return quiescence(pos, alpha, beta, ply, nodes);
    }
    *nodes += 1;

    if pos.is_insufficient_material() {
        return 0;
    }
    let moves = ordered_moves(pos);
    if moves.is_empty() {
        return if pos.is_check() { -MATE + ply } else { 0 }; // mate or stalemate
    }

    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(m);
        let score = -negamax(&next, depth - 1, -beta, -alpha, ply + 1, nodes);
        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    alpha
}

/// Returns (best move, score, nodes searched).
fn search_root(pos: &Chess, depth: i32) -> (Option<Move>, i32, u64) {
    let mut nodes = 0u64;
    let mut alpha = -INF;
    let mut best: Option<Move> = None;
    if pos.legal_moves().is_empty() {
        return (None, if pos.is_check() { -MATE } else { 0 }, 0);
    }

    for m in ordered_moves(pos) {
        let mv = m.clone();
        let mut next = pos.clone();
        next.play_unchecked(m);
        let score = -negamax(&next, depth - 1, -INF, -alpha, 1, &mut nodes);
        if best.is_none() || score > alpha {
            alpha = score;
            best = Some(mv);
        }
    }
    (best, alpha, nodes)
}

// ----------------------------------------------------------------------- UCI

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

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut pos = Chess::default();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let tokens: Vec<&str> = line.split_whitespace().collect();

        match tokens.first().copied() {
            Some("uci") => {
                println!("id name chess-engine 0.1");
                println!("id author shrijitm");
                println!("uciok");
            }
            Some("isready") => println!("readyok"),
            Some("ucinewgame") => pos = Chess::default(),
            Some("position") => match parse_position(&tokens[1..]) {
                Some(p) => pos = p,
                None => eprintln!("error: could not parse position"),
            },
            Some("go") => {
                let mut depth: i32 = DEFAULT_DEPTH;
                if let Some(i) = tokens.iter().position(|&t| t == "depth") {
                    if let Some(d) = tokens.get(i + 1).and_then(|t| t.parse().ok()) {
                        depth = d;
                    }
                }
                let (best, score, nodes) = search_root(&pos, depth);
                println!("info depth {depth} score cp {score} nodes {nodes}");
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
            Some("quit") => break,
            _ => {} // unknown commands are ignored, as UCI requires
        }

        stdout.flush().unwrap();
    }
}
