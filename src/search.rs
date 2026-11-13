use crate::eval::{VALUE, evaluate, role_index};
use crate::tt::{Bound, Tt};
use shakmaty::zobrist::Zobrist64;
use shakmaty::{Chess, Color, EnPassantMode, Move, Position};
use std::time::{Duration, Instant};

pub const INF: i32 = 32_000;
pub const MATE: i32 = 30_000;
pub const MAX_DEPTH: i32 = 32;
const MAX_PLY: i32 = 64;
const HISTORY_MAX: i32 = 6_000;

struct Search<'a> {
    nodes: u64,
    start: Instant,
    limit: Option<Duration>,
    stopped: bool,
    tt: &'a mut Tt,
    tt_cutoffs: u64,
    repetition_draws: u64,
    killers: Vec<[Option<Move>; 2]>, // two killer moves per ply
    history: Vec<i32>,               // move-ordering history, [color][from][to]
    path: Vec<u64>,                  // hashes of earlier positions: game plus search line
}

/// Index into the history table, or None for moves without a from-square.
fn history_index(color: Color, m: &Move) -> Option<usize> {
    let from = m.from()?;
    let c = if color == Color::White { 1 } else { 0 };
    Some(c * 4096 + (from as usize) * 64 + (m.to() as usize))
}

impl Search<'_> {
    /// Called once per node; checks the clock every 2048 nodes.
    fn tick(&mut self) {
        self.nodes += 1;
        if self.nodes & 2047 == 0
            && let Some(limit) = self.limit
            && self.start.elapsed() >= limit
        {
            self.stopped = true;
        }
    }

    /// Remember a quiet move that caused a beta cutoff.
    fn record_cutoff(&mut self, m: &Move, color: Color, depth: i32, ply: i32) {
        let p = ply as usize;
        if p < self.killers.len() && self.killers[p][0].as_ref() != Some(m) {
            self.killers[p][1] = self.killers[p][0].take();
            self.killers[p][0] = Some(*m);
        }
        if let Some(i) = history_index(color, m) {
            self.history[i] = (self.history[i] + depth * depth).min(HISTORY_MAX);
        }
    }

    fn move_score(&self, m: &Move, color: Color, killers: Option<&[Option<Move>; 2]>) -> i32 {
        // Captures: most valuable victim, least valuable attacker.
        // Promotions come right after.
        if m.is_capture() || m.is_promotion() {
            let mut s = 0;
            if let Some(victim) = m.capture() {
                s += 10_000 + 10 * VALUE[role_index(victim)] - VALUE[role_index(m.role())];
            }
            if let Some(promo) = m.promotion() {
                s += 9_000 + VALUE[role_index(promo)];
            }
            return s;
        }
        // Quiet moves: killers first, then by history score.
        if let Some(k) = killers {
            if k[0].as_ref() == Some(m) {
                return 8_000;
            }
            if k[1].as_ref() == Some(m) {
                return 7_000;
            }
        }
        history_index(color, m).map_or(0, |i| self.history[i])
    }

    /// Legal moves, best-looking first. A move suggested by the transposition
    /// table, if given, is tried before everything else.
    fn ordered_moves(&self, pos: &Chess, first: Option<&Move>, ply: i32) -> Vec<Move> {
        let color = pos.turn();
        let killers = self.killers.get(ply as usize);
        let mut scored: Vec<(i32, Move)> = pos
            .legal_moves()
            .into_iter()
            .map(|m| (self.move_score(&m, color, killers), m))
            .collect();
        scored.sort_by_key(|(sc, _)| std::cmp::Reverse(*sc));
        let mut moves: Vec<Move> = scored.into_iter().map(|(_, m)| m).collect();
        if let Some(fm) = first
            && let Some(i) = moves.iter().position(|m| m == fm)
        {
            let mv = moves.remove(i);
            moves.insert(0, mv);
        }
        moves
    }
}

pub fn hash(pos: &Chess) -> u64 {
    let h = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal);
    h.0
}

fn tt_hash(pos: &Chess) -> u64 {
    let key = hash(pos);
    let halfmoves = pos.halfmoves();

    // Reuse entries across clocks when the search horizon cannot reach 100.
    if halfmoves.saturating_add(MAX_PLY as u32) < 100 {
        key
    } else {
        key ^ (halfmoves as u64).wrapping_mul(0x9E3779B97F4A7C15)
    }
}

// Mate scores depend on the distance from the root, so they are stored in the
// table relative to the position itself and converted back on retrieval.
fn score_to_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE - MAX_PLY {
        score + ply
    } else if score <= -MATE + MAX_PLY {
        score - ply
    } else {
        score
    }
}

fn score_from_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE - MAX_PLY {
        score - ply
    } else if score <= -MATE + MAX_PLY {
        score + ply
    } else {
        score
    }
}

fn quiescence(pos: &Chess, mut alpha: i32, beta: i32, ply: i32, s: &mut Search<'_>) -> i32 {
    s.tick();
    if s.stopped {
        return 0;
    }

    let in_check = pos.is_check();
    let mut moves = s.ordered_moves(pos, None, ply);

    // Detect mate and stalemate before evaluating the position.
    if moves.is_empty() {
        return if in_check { -MATE + ply } else { 0 };
    }

    if pos.is_insufficient_material() || pos.halfmoves() >= 100 {
        return 0;
    }

    // The current position plus two earlier occurrences is a threefold draw.
    let key = hash(pos);
    let window = pos.halfmoves() as usize;
    let previous_matches = s
        .path
        .iter()
        .rev()
        .take(window)
        .filter(|&&h| h == key)
        .count();
    if previous_matches >= 2 {
        s.repetition_draws += 1;
        return 0;
    }

    if ply >= MAX_PLY {
        return evaluate(pos);
    }

    if !in_check {
        let stand_pat = evaluate(pos);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        moves.retain(|m| m.is_capture() || m.is_promotion());
    }

    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(m);
        s.path.push(key);
        let score = -quiescence(&next, -beta, -alpha, ply + 1, s);
        s.path.pop();

        if s.stopped {
            return 0;
        }
        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }
    alpha
}

fn negamax(
    pos: &Chess,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    ply: i32,
    s: &mut Search<'_>,
) -> i32 {
    if depth <= 0 {
        return quiescence(pos, alpha, beta, ply, s);
    }
    s.tick();
    if s.stopped {
        return 0;
    }

    if pos.is_insufficient_material() {
        return 0;
    }

    // A fifty-move draw takes priority over cached scores, except checkmate.
    if pos.halfmoves() >= 100 {
        if pos.is_check() && pos.legal_moves().is_empty() {
            return -MATE + ply;
        }
        return 0;
    }

    let key = hash(pos);
    let tt_key = tt_hash(pos);

    // Repetition: this position already occurred earlier in the game or on the
    // current search line since the last pawn move or capture. Only the last
    // `halfmoves` positions can possibly match.
    let window = pos.halfmoves() as usize;
    let previous_matches = s
        .path
        .iter()
        .rev()
        .take(window)
        .filter(|&&h| h == key)
        .count();
    if previous_matches >= 2 {
        s.repetition_draws += 1;
        return 0;
    }

    // Transposition table lookup.
    let mut tt_move: Option<Move> = None;
    if let Some(e) = s.tt.probe(tt_key) {
        tt_move = e.best;
        if e.depth >= depth {
            let sc = score_from_tt(e.score, ply);
            let usable = match e.bound {
                Bound::Exact => true,
                Bound::Lower => sc >= beta,
                Bound::Upper => sc <= alpha,
            };
            if usable {
                s.tt_cutoffs += 1;
                return sc;
            }
        }
    }

    let moves = s.ordered_moves(pos, tt_move.as_ref(), ply);
    if moves.is_empty() {
        return if pos.is_check() { -MATE + ply } else { 0 }; // mate or stalemate
    }

    let alpha_orig = alpha;
    let mut best_move: Option<Move> = None;

    let mut first_move = true;
    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(m);
        s.path.push(key);

        let mut score = if first_move {
            -negamax(&next, depth - 1, -beta, -alpha, ply + 1, s)
        } else {
            -negamax(&next, depth - 1, -alpha - 1, -alpha, ply + 1, s)
        };
        if !first_move && !s.stopped && score > alpha && score < beta {
            score = -negamax(&next, depth - 1, -beta, -alpha, ply + 1, s);
        }

        s.path.pop();
        if s.stopped {
            return 0;
        }
        first_move = false;
        if score >= beta {
            if !m.is_capture() && !m.is_promotion() {
                s.record_cutoff(&m, pos.turn(), depth, ply);
            }
            s.tt.store(
                tt_key,
                depth,
                score_to_tt(score, ply),
                Bound::Lower,
                Some(m),
            );
            return score;
        }
        if score > alpha {
            alpha = score;
            best_move = Some(m);
        }
    }

    let bound = if alpha > alpha_orig {
        Bound::Exact
    } else {
        Bound::Upper
    };
    s.tt.store(
        tt_key,
        depth,
        score_to_tt(alpha, ply),
        bound,
        best_move.or(tt_move),
    );
    alpha
}

/// Iterative deepening. Returns (best move, score). A depth that is cut off
/// by the clock is discarded; the last completed depth decides the move.
/// `game_history` holds the hashes of the positions played before `pos`.
pub fn search(
    pos: &Chess,
    max_depth: i32,
    limit: Option<Duration>,
    tt: &mut Tt,
    game_history: &[u64],
) -> (Option<Move>, i32) {
    let mut s = Search {
        nodes: 0,
        start: Instant::now(),
        limit: None,
        stopped: false,
        tt,
        tt_cutoffs: 0,
        repetition_draws: 0,
        killers: vec![[None, None]; MAX_PLY as usize],
        history: vec![0; 2 * 64 * 64],
        path: game_history.to_vec(),
    };
    // The root position stays on the path for the whole search.
    s.path.push(hash(pos));

    let mut root_moves = s.ordered_moves(pos, None, 0);
    if root_moves.is_empty() {
        return (None, if pos.is_check() { -MATE } else { 0 });
    }

    let mut best_move: Option<Move> = None;
    let mut best_score = 0;

    for depth in 1..=max_depth.min(MAX_DEPTH) {
        // Always finish depth 1 so there is a move to play.
        s.limit = if depth == 1 { None } else { limit };

        let mut alpha = -INF;
        let mut iter_best: Option<Move> = None;
        let mut first_move = true;
        for m in &root_moves {
            let mut next = pos.clone();
            next.play_unchecked(*m);

            let mut score = if first_move {
                -negamax(&next, depth - 1, -INF, -alpha, 1, &mut s)
            } else {
                -negamax(&next, depth - 1, -alpha - 1, -alpha, 1, &mut s)
            };
            if !first_move && !s.stopped && score > alpha {
                score = -negamax(&next, depth - 1, -INF, -alpha, 1, &mut s);
            }

            if s.stopped {
                break;
            }
            first_move = false;
            if iter_best.is_none() || score > alpha {
                alpha = score;
                iter_best = Some(*m);
            }
        }
        if s.stopped {
            break;
        }

        best_move = iter_best;
        best_score = alpha;

        let ms = s.start.elapsed().as_millis() as u64;
        let nps = s.nodes.saturating_mul(1000).checked_div(ms).unwrap_or(0);
        println!(
            "info depth {depth} score cp {best_score} nodes {} time {ms} nps {nps}",
            s.nodes
        );

        // Search the best move first at the next depth.
        if let Some(bm) = &best_move
            && let Some(i) = root_moves.iter().position(|m| m == bm)
        {
            let mv = root_moves.remove(i);
            root_moves.insert(0, mv);
        }

        // A forced mate has been found; deeper search won't change the result.
        if best_score.abs() >= MATE - MAX_PLY {
            break;
        }
    }
    println!(
        "info string tt cutoffs {} repetition draws {}",
        s.tt_cutoffs, s.repetition_draws
    );
    (best_move, best_score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::CastlingMode;
    use shakmaty::fen::Fen;

    fn position(fen_text: &str) -> Chess {
        let fen: Fen = fen_text.parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        pos
    }

    fn test_search<'a>(tt: &'a mut Tt, path: Vec<u64>) -> Search<'a> {
        Search {
            nodes: 0,
            start: Instant::now(),
            limit: None,
            stopped: false,
            tt,
            tt_cutoffs: 0,
            repetition_draws: 0,
            killers: vec![[None, None]; MAX_PLY as usize],
            history: vec![0; 2 * 64 * 64],
            path,
        }
    }

    #[test]
    fn quiescence_scores_stalemate_as_draw() {
        let pos = position("7k/5K2/6Q1/8/8/8/8/8 b - - 0 1");
        assert!(!pos.is_check());
        assert!(pos.legal_moves().is_empty());

        let mut tt = Tt::with_bits(8);
        let mut s = test_search(&mut tt, Vec::new());
        assert_eq!(quiescence(&pos, -INF, INF, 0, &mut s), 0);
    }

    #[test]
    fn quiescence_scores_insufficient_material_as_draw() {
        let pos = position("7k/8/5K2/8/8/8/8/6B1 w - - 0 1");
        assert!(pos.is_insufficient_material());

        let mut tt = Tt::with_bits(8);
        let mut s = test_search(&mut tt, Vec::new());
        assert_eq!(quiescence(&pos, -INF, INF, 0, &mut s), 0);
    }

    #[test]
    fn quiescence_scores_fifty_move_position_as_draw() {
        let pos = position("7k/8/5K2/8/8/8/8/6R1 w - - 100 1");
        assert!(!pos.legal_moves().is_empty());

        let mut tt = Tt::with_bits(8);
        let mut s = test_search(&mut tt, Vec::new());
        assert_eq!(quiescence(&pos, -INF, INF, 0, &mut s), 0);
    }

    #[test]
    fn one_previous_occurrence_is_not_threefold() {
        let pos = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 10 6");
        let key = hash(&pos);

        let mut tt = Tt::with_bits(8);
        let mut s = test_search(&mut tt, vec![key]);
        quiescence(&pos, -INF, INF, 0, &mut s);
        assert_eq!(s.repetition_draws, 0);
    }

    #[test]
    fn two_previous_occurrences_are_threefold() {
        let pos = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 10 6");
        let key = hash(&pos);

        let mut tt = Tt::with_bits(8);
        let mut s = test_search(&mut tt, vec![key, key]);
        assert_eq!(quiescence(&pos, -INF, INF, 0, &mut s), 0);
        assert_eq!(s.repetition_draws, 1);
    }

    #[test]
    fn transposition_keys_respect_the_fifty_move_horizon() {
        let early = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 10 6");
        let safe = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 35 18");
        let near_draw = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 36 19");
        let late = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 99 50");

        assert_eq!(hash(&early), hash(&safe));
        assert_eq!(hash(&early), hash(&near_draw));
        assert_eq!(tt_hash(&early), tt_hash(&safe));
        assert_ne!(tt_hash(&early), tt_hash(&near_draw));
        assert_ne!(tt_hash(&near_draw), tt_hash(&late));
    }
    #[test]
    fn fifty_move_draw_overrides_cached_score() {
        let pos = position("7k/8/5K2/8/8/8/8/6R1 w - - 100 1");
        let mut tt = Tt::with_bits(8);
        tt.store(tt_hash(&pos), 8, 777, Bound::Exact, None);

        let mut s = test_search(&mut tt, Vec::new());
        assert_eq!(negamax(&pos, 1, -INF, INF, 0, &mut s), 0);
    }
}
