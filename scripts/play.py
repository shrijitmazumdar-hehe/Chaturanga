import chess
import chess.engine

engine = chess.engine.SimpleEngine.popen_uci("./target/release/chaturanga")
board = chess.Board()

try:
    while not board.is_game_over(claim_draw=True):
        print()
        print(board)

        if board.turn == chess.WHITE:
            text = input("\nYour move (SAN or UCI, or 'quit'): ").strip()
            if text.lower() == "quit":
                print("Game abandoned.")
                break

            try:
                try:
                    move = board.parse_san(text)
                except ValueError:
                    move = chess.Move.from_uci(text)
                    if move not in board.legal_moves:
                        raise ValueError("That move is not legal.")
                board.push(move)
            except ValueError as error:
                print(f"Invalid move: {error}")
        else:
            result = engine.play(board, chess.engine.Limit(time=0.5))
            print(f"\nChaturanga plays {board.san(result.move)}")
            board.push(result.move)

    if board.is_game_over(claim_draw=True):
        print()
        print(board)
        print(f"\nGame over: {board.result(claim_draw=True)}")
finally:
    engine.quit()
