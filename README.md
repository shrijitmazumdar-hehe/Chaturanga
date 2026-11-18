# Chaturanga

Chaturanga is a UCI chess engine written in Rust. It is a work in progress.

## Build and run

    cargo build --release
    ./target/release/chaturanga

Add the resulting binary to a UCI-compatible chess GUI such as PyChess.

## Features

- Negamax search with alpha-beta pruning, iterative deepening, and quiescence search
- Transposition table, PVS, killer moves, and history-based move ordering
- Evaluation with material, piece-square tables, passed pawns, king safety, and endgame terms
- Legal move generation and chess rules from the shakmaty library

## Testing

    cargo test
    cargo clippy --release -- -D warnings
    bash scripts/perft.sh
    bash scripts/tactics.sh

The opening positions in scripts/openings/ are used by the match-testing script.
They are test data, not an opening book, and they do not train the engine.
The files come from the Lichess Openings dataset: https://github.com/lichess-org/chess-openings. The dataset is released under CC0.

## Experimental strength result

In one 400-game test against the local reference engine engine_v06, at 0.1 seconds
per move, this build scored 41%: 80 wins, 168 draws, and 152 losses. The estimated
difference was -63 Elo, with a 95% range from -90 to -37. This is an experimental
engine-to-engine result, not a human rating. The reference engine is not included,
and this test used a random opening sample.

## Limitations

- UCI stop is not supported; searches run synchronously
- Single-threaded, with a fixed-size transposition table
- No opening book or endgame tablebases
- Playing strength and endgame technique are still being improved

## License

Chaturanga is licensed under GPL-3.0-or-later. See [LICENSE](LICENSE).
