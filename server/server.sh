#!/bin/bash
set -euo pipefail
echo "Game ID: ${GAME_ID:-unset}"
echo "Using IWAD: $IWAD"
echo "Warp Level: ${WARP:-unset}"
echo "Using Game Skill: ${SKILL:-unset}"
echo "Using Data Root: ${DATA_ROOT:-unset}"
CMD=(
  /usr/games/woof
  -privateserver
  -complevel boom
)
# if [[ -n "${WARP:-}" ]]; then
#     CMD+=(-warp "$WARP")
# fi
# if [[ -n "${SKILL:-}" ]]; then
#     CMD+=(-skill "$SKILL")
# fi
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

export XDG_RUNTIME_DIR=/tmp/xdg
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export SDL_AUDIODRIVER=dummy
export DOOMWADDIR="$DATA_ROOT"
cd "$DATA_ROOT"
echo ">>> ${CMD[*]}"
exec "${CMD[@]}"
