#!/usr/bin/env bash
# Usage: fruit_analyze.sh "e2e4 d7d5 ..."
# For each prefix of the move list, ask fruit to search the resulting
# position (go movetime 800) and print the mover-relative score it reports
# just before bestmove. Score convention: UCI "score cp N" is always from
# the engine's own perspective = whoever is to move in that position.
set -uo pipefail
MOVES="$1"
MOVETIME="${2:-800}"

# shellcheck disable=SC2206
MOVE_ARR=($MOVES)
PREFIX=""

for i in "${!MOVE_ARR[@]}"; do
  PREFIX="$PREFIX ${MOVE_ARR[$i]}"
  PREFIX="${PREFIX# }"

  if [ -z "$PREFIX" ]; then
    POS="position startpos"
  else
    POS="position startpos moves $PREFIX"
  fi

  coproc FRUIT { /usr/bin/fruit; }
  exec 3>&"${FRUIT[1]}"
  exec 4<&"${FRUIT[0]}"

  echo "uci" >&3
  echo "isready" >&3
  echo "ucinewgame" >&3
  echo "$POS" >&3
  echo "go movetime $MOVETIME" >&3

  LAST_SCORE=""
  BESTMOVE=""
  while IFS= read -r -t 15 line <&4; do
    if [[ "$line" == info*score* ]]; then
      LAST_SCORE=$(echo "$line" | grep -oE 'score (cp|mate) -?[0-9]+' | tail -1)
    fi
    if [[ "$line" == bestmove* ]]; then
      BESTMOVE="$line"
      break
    fi
  done

  echo "quit" >&3 2>/dev/null
  exec 3>&-
  exec 4<&-
  wait "$FRUIT_PID" 2>/dev/null

  PLY=$((i + 1))
  echo "ply=$PLY move=${MOVE_ARR[$i]} $LAST_SCORE $BESTMOVE"
done
