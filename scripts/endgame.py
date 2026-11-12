#!/usr/bin/env python3
"""Let an engine play both sides of a won endgame and report whether it mates.

usage: endgame.py ENGINE [--movetime SECONDS]
"""
import argparse

import chess
import chess.engine

POSITIONS = {
    "king and queen vs king": "8/8/8/4k3/8/8/8/KQ6 w - - 0 1",
    "king and rook vs king": "8/8/8/4k3/8/8/8/K6R w - - 0 1",
}
MAX_PLIES = 150


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("engine")
    ap.add_argument("--movetime", type=float, default=0.1)
    args = ap.parse_args()

    engine = chess.engine.SimpleEngine.popen_uci(args.engine)
    try:
        for name, fen in POSITIONS.items():
            board = chess.Board(fen)
            moves = []
            while not board.is_game_over(claim_draw=True) and board.ply() < MAX_PLIES:
                res = engine.play(board, chess.engine.Limit(time=args.movetime), game=name)
                moves.append(board.san(res.move))
                board.push(res.move)
            outcome = board.outcome(claim_draw=True)
            if outcome and outcome.winner is not None:
                print(f"{name}: checkmate after {board.ply()} plies")
            elif outcome:
                print(f"{name}: DRAW ({outcome.termination.name.lower()}) after {board.ply()} plies")
            else:
                print(f"{name}: NOT FINISHED after {board.ply()} plies")
            print("Moves:", " ".join(moves))

    finally:
        engine.quit()


if __name__ == "__main__":
    main()
