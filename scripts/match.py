#!/usr/bin/env python3
"""Play two UCI engines against each other and report the result.

usage: match.py ENGINE_A ENGINE_B [--openings N] [--movetime SECONDS]
                [--workers K] [--openings-file FILE [FILE ...]] [--book-plies N]
                [--random-plies N] [--seed S] [--a-opt NAME=VALUE]
                [--b-opt NAME=VALUE] [--pgn FILE]

Every opening is played twice with the colors swapped, so --openings 200 means
400 games. Results are from engine A's point of view.
"""
import argparse
import csv
import math
import os
import random
from collections import Counter
from concurrent.futures import ProcessPoolExecutor

import chess
import chess.engine
import chess.pgn

BUILTIN = [
    "e4 e5 Nf3 Nc6 Bb5",
    "e4 e5 Nf3 Nc6 Bc4",
    "e4 c5 Nf3 d6 d4",
    "e4 e6 d4 d5",
    "e4 c6 d4 d5",
    "d4 d5 c4 e6 Nc3",
    "d4 Nf6 c4 g6 Nc3 Bg7",
    "d4 Nf6 c4 e6 Nf3 b6",
    "c4 e5 Nc3 Nf6",
    "Nf3 d5 g3 Nf6 Bg2",
    "e4 d5 exd5 Qxd5 Nc3",
    "d4 f5 g3 Nf6 Bg2",
]

MAX_PLIES = 240  # games that reach this length are scored as draws


def parse_opt(text):
    name, _, value = text.partition("=")
    if value.lower() in ("true", "false"):
        return name, value.lower() == "true"
    try:
        return name, int(value)
    except ValueError:
        return name, value


def line_to_moves(san_line, max_plies):
    """Opening line in SAN -> list of UCI moves, or None if it is unusable."""
    board = chess.Board()
    moves = []
    for token in san_line.split():
        if "." in token:  # move numbers such as "1." or "1..."
            continue
        if len(moves) >= max_plies:
            break
        try:
            move = board.push_san(token)
        except ValueError:
            return None
        moves.append(move.uci())
    if not moves or board.is_game_over():
        return None
    return moves


def load_openings(paths, max_plies):
    seen, result = set(), []
    for path in paths:
        with open(path, newline="") as f:
            reader = csv.DictReader(f, delimiter="\t")
            if "pgn" not in (reader.fieldnames or []):
                raise SystemExit(f"{path}: no 'pgn' column (found {reader.fieldnames})")
            for row in reader:
                moves = line_to_moves(row["pgn"], max_plies)
                if not moves:
                    continue
                board = chess.Board()
                for uci in moves:
                    board.push_uci(uci)
                key = board.epd()
                if key in seen:
                    continue
                seen.add(key)
                result.append(moves)
    return result


def random_opening(rng, plies):
    while True:
        board = chess.Board()
        moves = []
        for _ in range(plies):
            move = rng.choice(list(board.legal_moves))
            board.push(move)
            moves.append(move.uci())
            if board.is_game_over():
                break
        if not board.is_game_over():
            return moves


def make_openings(args):
    rng = random.Random(args.seed)
    if args.openings_file:
        pool = load_openings(args.openings_file, args.book_plies)
        rng.shuffle(pool)
        print(f"loaded {len(pool)} distinct openings from file(s)")
    else:
        pool = [line_to_moves(line, 99) for line in BUILTIN]
    if len(pool) < args.openings:
        extra = args.openings - len(pool)
        print(f"adding {extra} random-move openings ({args.random_plies} plies each)")
        while len(pool) < args.openings:
            pool.append(random_opening(rng, args.random_plies))
    return pool[: args.openings]


def play_game(white, black, opening, movetime):
    """Returns (result, board, note); result is '1-0', '0-1' or '1/2-1/2'."""
    board = chess.Board()
    for uci in opening:
        board.push_uci(uci)
    engines = {chess.WHITE: white, chess.BLACK: black}

    while not board.is_game_over(claim_draw=True):
        if board.ply() >= MAX_PLIES:
            return "1/2-1/2", board, "move limit"
        mover = board.turn
        move, note = None, "no move returned"
        try:
            res = engines[mover].play(board, chess.engine.Limit(time=movetime))
            move = res.move
        except (chess.engine.EngineError, chess.engine.EngineTerminatedError) as e:
            note = f"engine error: {e}"
        if move is None:
            return ("0-1" if mover == chess.WHITE else "1-0"), board, note
        board.push(move)
    outcome = board.outcome(claim_draw=True)
    note = outcome.termination.name.lower() if outcome else ""
    return board.result(claim_draw=True), board, note


def one_game(task):
    """Runs in a worker process: starts two fresh engines and plays one game."""
    (path_w, opts_w, path_b, opts_b, name_w, name_b, opening, movetime, round_no) = task
    white = chess.engine.SimpleEngine.popen_uci(path_w)
    black = chess.engine.SimpleEngine.popen_uci(path_b)
    try:
        if opts_w:
            white.configure(opts_w)
        if opts_b:
            black.configure(opts_b)
        result, board, note = play_game(white, black, opening, movetime)
    finally:
        for e in (white, black):
            try:
                e.quit()
            except Exception:
                pass
    game = chess.pgn.Game.from_board(board)
    game.headers["White"] = name_w
    game.headers["Black"] = name_b
    game.headers["Round"] = str(round_no)
    game.headers["Result"] = result
    return result, board.ply(), note, str(game)


def elo(score):
    return -400 * math.log10(1 / score - 1)


def report(w, d, l, endings):
    n = w + d + l
    score = (w + 0.5 * d) / n
    print(f"\nA: +{w} ={d} -{l}  ({n} games)  score {score * 100:.1f}%")
    if 0 < score < 1:
        var = (w * (1 - score) ** 2 + d * (0.5 - score) ** 2 + l * score**2) / n
        se = math.sqrt(var / n)
        lo = min(max(score - 1.96 * se, 1e-6), 1 - 1e-6)
        hi = min(max(score + 1.96 * se, 1e-6), 1 - 1e-6)
        print(f"Elo difference (A minus B): {elo(score):+.0f}  "
              f"(95% range {elo(lo):+.0f} to {elo(hi):+.0f})")
    else:
        print("One-sided result: too lopsided for an Elo estimate.")
    print("How games ended: " + ", ".join(f"{k} {v}" for k, v in endings.most_common()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("engine_a")
    ap.add_argument("engine_b")
    ap.add_argument("--openings", type=int, default=12)
    ap.add_argument("--movetime", type=float, default=0.2)
    ap.add_argument("--workers", type=int, default=1)
    ap.add_argument("--openings-file", nargs="+", default=[])
    ap.add_argument("--book-plies", type=int, default=10)
    ap.add_argument("--random-plies", type=int, default=6)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--a-opt", action="append", default=[])
    ap.add_argument("--b-opt", action="append", default=[])
    ap.add_argument("--pgn", default="scripts/games.pgn")
    args = ap.parse_args()

    name_a = os.path.basename(args.engine_a)
    name_b = os.path.basename(args.engine_b)
    if name_a == name_b:
        name_a, name_b = name_a + "(A)", name_b + "(B)"
    opts_a = dict(parse_opt(o) for o in args.a_opt)
    opts_b = dict(parse_opt(o) for o in args.b_opt)

    openings = make_openings(args)
    tasks, a_white_flags = [], []
    for round_no, opening in enumerate(openings, 1):
        for a_is_white in (True, False):
            if a_is_white:
                task = (args.engine_a, opts_a, args.engine_b, opts_b, name_a, name_b)
            else:
                task = (args.engine_b, opts_b, args.engine_a, opts_a, name_b, name_a)
            tasks.append(task + (opening, args.movetime, round_no))
            a_white_flags.append(a_is_white)
    total = len(tasks)
    print(f"{total} games, {args.workers} worker(s), {args.movetime}s per move")

    w = d = l = 0
    endings = Counter()
    pool = ProcessPoolExecutor(max_workers=args.workers)
    try:
        with open(args.pgn, "w") as pgn:
            results = pool.map(one_game, tasks)
            for n, (a_is_white, res) in enumerate(zip(a_white_flags, results), 1):
                result, plies, note, text = res
                white_score = {"1-0": 1.0, "0-1": 0.0}.get(result, 0.5)
                a_score = white_score if a_is_white else 1.0 - white_score
                if a_score == 1.0:
                    w += 1
                elif a_score == 0.0:
                    l += 1
                else:
                    d += 1
                endings[note or "unknown"] += 1
                pgn.write(text + "\n\n")
                pgn.flush()
                print(f"game {n}/{total}: {result:7s} ply {plies:3d} {note:22s} "
                      f"running A: +{w} ={d} -{l}")
    except KeyboardInterrupt:
        print("\ninterrupted: reporting the games finished so far")
        pool.shutdown(wait=False, cancel_futures=True)
    finally:
        pool.shutdown(wait=True, cancel_futures=True)

    if w + d + l:
        report(w, d, l, endings)


if __name__ == "__main__":
    main()
