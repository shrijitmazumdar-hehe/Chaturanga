#!/usr/bin/env python3
"""Play two UCI engines against each other and report the result.

usage: match.py ENGINE_A ENGINE_B [--openings N] [--movetime SECONDS]
                [--a-opt NAME=VALUE] [--b-opt NAME=VALUE] [--pgn FILE]

Every opening is played twice with the colors swapped. Results are from
engine A's point of view.
"""
import argparse
import math
import os

import chess
import chess.engine
import chess.pgn

OPENINGS = [
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


def play_game(white, black, opening, movetime, game_id):
    """Returns (result, board, note); result is '1-0', '0-1' or '1/2-1/2'."""
    board = chess.Board()
    for san in opening.split():
        board.push_san(san)
    engines = {chess.WHITE: white, chess.BLACK: black}

    while not board.is_game_over(claim_draw=True):
        if board.ply() >= MAX_PLIES:
            return "1/2-1/2", board, "move limit"
        mover = board.turn
        move, note = None, "no move returned"
        try:
            res = engines[mover].play(
                board, chess.engine.Limit(time=movetime), game=game_id
            )
            move = res.move
        except (chess.engine.EngineError, chess.engine.EngineTerminatedError) as e:
            note = f"engine error: {e}"
        if move is None:
            return ("0-1" if mover == chess.WHITE else "1-0"), board, note
        board.push(move)
    outcome = board.outcome(claim_draw=True)
    note = outcome.termination.name.lower() if outcome else ""
    return board.result(claim_draw=True), board, note


def elo(score):
    return -400 * math.log10(1 / score - 1)


def report(w, d, l):
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("engine_a")
    ap.add_argument("engine_b")
    ap.add_argument("--openings", type=int, default=12)
    ap.add_argument("--movetime", type=float, default=0.2)
    ap.add_argument("--a-opt", action="append", default=[])
    ap.add_argument("--b-opt", action="append", default=[])
    ap.add_argument("--pgn", default="scripts/games.pgn")
    args = ap.parse_args()

    name_a = os.path.basename(args.engine_a)
    name_b = os.path.basename(args.engine_b)
    if name_a == name_b:
        name_a, name_b = name_a + "(A)", name_b + "(B)"

    a = chess.engine.SimpleEngine.popen_uci(args.engine_a)
    b = chess.engine.SimpleEngine.popen_uci(args.engine_b)
    w = d = l = 0
    try:
        if args.a_opt:
            a.configure(dict(parse_opt(o) for o in args.a_opt))
        if args.b_opt:
            b.configure(dict(parse_opt(o) for o in args.b_opt))

        game_no = 0
        with open(args.pgn, "w") as pgn:
            for i in range(args.openings):
                opening = OPENINGS[i % len(OPENINGS)]
                for a_is_white in (True, False):
                    game_no += 1
                    white, black = (a, b) if a_is_white else (b, a)
                    result, board, note = play_game(
                        white, black, opening, args.movetime, game_no
                    )
                    white_score = {"1-0": 1.0, "0-1": 0.0}.get(result, 0.5)
                    a_score = white_score if a_is_white else 1.0 - white_score
                    if a_score == 1.0:
                        w += 1
                    elif a_score == 0.0:
                        l += 1
                    else:
                        d += 1

                    wn = name_a if a_is_white else name_b
                    bn = name_b if a_is_white else name_a
                    print(f"game {game_no:2d}: {wn} (W) vs {bn} (B): "
                          f"{result:7s} ply {board.ply():3d} {note}")

                    game = chess.pgn.Game.from_board(board)
                    game.headers["White"] = wn
                    game.headers["Black"] = bn
                    game.headers["Result"] = result
                    print(game, file=pgn, end="\n\n")
                    pgn.flush()
    finally:
        for e in (a, b):
            try:
                e.quit()
            except Exception:
                pass

    if w + d + l:
        report(w, d, l)


if __name__ == "__main__":
    main()
