#!/usr/bin/env bash
# usage: scripts/tactics.sh [engine-binary]
ENGINE=${1:-./target/release/chess-engine}
fail=0

puzzle() {  # name fen expected-move depth
  got=$(printf "position fen %s\ngo depth %s\nquit\n" "$2" "$4" | "$ENGINE" | awk '/^bestmove/ {print $2}')
  if [ "$got" = "$3" ]; then echo "PASS $1: $got"
  else echo "FAIL $1: expected $3, got $got"; fail=1; fi
}

puzzle "scholar's mate"         "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4" h5f7 4
puzzle "fool's mate (black)"    "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2" d8h4 4
puzzle "back rank (white)"      "6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1" a1a8 4
puzzle "back rank (black)"      "r5k1/8/8/8/8/8/5PPP/6K1 b - - 0 1" a8a1 4
puzzle "take the queen (white)" "4k3/8/8/3q4/8/8/8/3QK3 w - - 0 1" d1d5 4
puzzle "take the queen (black)" "3qk3/8/8/8/3Q4/8/8/4K3 b - - 0 1" d8d4 4
puzzle "knight fork"            "r3k3/8/4N3/8/8/8/8/4K3 w - - 0 1" e6c7 5
puzzle "promote to queen"       "8/P6k/8/8/8/8/8/K7 w - - 0 1" a7a8q 4
puzzle "already stalemated"     "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1" 0000 3
puzzle "already checkmated"     "R5k1/5ppp/8/8/8/8/8/6K1 b - - 0 1" 0000 3

if [ "$fail" = 0 ]; then echo "ALL PASSED"; else echo "SOME FAILED"; fi
exit $fail
