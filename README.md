# Chaturanga

Chaturanga is a UCI chess engine written in Rust. It is a work in progress.

## Build and run

    cargo build --release
    ./target/release/chaturanga

Chaturanga uses the UCI protocol and can be loaded into a compatible chess GUI.

## Opening test data

The files in scripts/openings/ are from the Lichess chess openings dataset:
https://github.com/lichess-org/chess-openings

The dataset is released under CC0.

## Development status

The engine and its match-testing tools are still being developed. Match results
measure performance against another engine; they do not train Chaturanga.
