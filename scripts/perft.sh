#!/usr/bin/env bash
# usage: scripts/perft.sh [engine-binary]
ENGINE=${1:-./target/release/chess-engine}
fail=0

count() {  # position-command depth
  printf "%s\nperft %s\nquit\n" "$1" "$2" | "$ENGINE" | awk '/^nodes/ {print $2}'
}

check() {  # fen depth expected
  got=$(count "position fen $1" "$2")
  if [ "$got" = "$3" ]; then echo "PASS depth $2 $3  $1"
  else echo "FAIL depth $2 expected $3 got $got  $1"; fail=1; fi
}

equiv() {  # depth, position command with moves, equivalent fen
  a=$(count "$2" "$1")
  b=$(count "position fen $3" "$1")
  if [ -n "$a" ] && [ "$a" = "$b" ]; then echo "PASS moves-vs-fen depth $1 $a  ($2)"
  else echo "FAIL moves-vs-fen depth $1 moves=$a fen=$b  ($2)"; fail=1; fi
}

check "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" 5 4865609
check "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" 4 4085603
check "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1" 5 674624
check "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1" 4 422333
check "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8" 4 2103487
check "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10" 4 3894594

# castling, en passant and promotion played through UCI move text
equiv 3 "position startpos moves e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 e1g1" \
  "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQ1RK1 b kq - 5 4"
equiv 3 "position startpos moves e2e4 a7a6 e4e5 d7d5 e5d6" \
  "rnbqkbnr/1pp1pppp/p2P4/8/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3"
equiv 3 "position fen 8/P6k/8/8/8/8/8/K7 w - - 0 1 moves a7a8q" \
  "Q7/7k/8/8/8/8/8/K7 b - - 0 1"

if [ "$fail" = 0 ]; then echo "ALL PASSED"; else echo "SOME FAILED"; fi
exit $fail
