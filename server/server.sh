#!/bin/bash
set -euo pipefail
echo "Game ID: ${GAME_ID:-unset}"
echo "Using IWAD: $IWAD"
echo "Warp level: ${WARP:-unset}"
echo "Using game skill: ${SKILL:-unset}"
CMD=(
  woof
  -privateserver
  -complevel boom
  -iwad "$IWAD"
)
if [[ -n "${WARP:-}" ]]; then
    CMD+=(-warp "$WARP")
fi
if [[ -n "${SKILL:-}" ]]; then
    CMD+=(-skill "$SKILL")
fi
if [[ -n "${WAD_LIST:-}" ]]; then
    IFS=',' read -r -a WADS <<< "$WAD_LIST"
    for wad in "${WADS[@]}"; do
        wad="${wad#"${wad%%[![:space:]]*}"}"  # ltrim
        wad="${wad%"${wad##*[![:space:]]}"}"  # rtrim
        wad="$DATA_ROOT/$wad"
        if [[ ! -f "$wad" ]]; then
            echo "❌ WAD not found: $wad" >&2
            exit 1
        fi
        echo "Adding WAD: $wad"
        CMD+=(-file "$wad")
    done
fi
echo ">>> ${CMD[*]}"
exec "${CMD[@]}"
