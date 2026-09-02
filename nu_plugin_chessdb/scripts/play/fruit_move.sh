#!/usr/bin/env bash
# Usage: fruit_move.sh "e2e4 e7e5 g1f3 ..."   (space-separated UCI move list, may be empty)
set -uo pipefail
MOVES="${1:-}"
MOVETIME="${2:-1000}"

if [ -z "$MOVES" ]; then
  POS="position startpos"
else
  POS="position startpos moves $MOVES"
fi

coproc FRUIT { /usr/bin/fruit; }

exec 3>&"${FRUIT[1]}"
exec 4<&"${FRUIT[0]}"

echo "uci" >&3
echo "isready" >&3
echo "ucinewgame" >&3
echo "$POS" >&3
echo "go movetime $MOVETIME" >&3

BESTMOVE=""
while IFS= read -r -t 15 line <&4; do
  if [[ "$line" == bestmove* ]]; then
    BESTMOVE="$line"
    break
  fi
done

echo "quit" >&3 2>/dev/null
exec 3>&-
exec 4<&-
wait "$FRUIT_PID" 2>/dev/null

if [ -z "$BESTMOVE" ]; then
  echo "ERROR: no bestmove received" >&2
  exit 1
fi
echo "$BESTMOVE"
