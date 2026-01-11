#!/bin/bash
set -euo pipefail
echo "Game ID: ${GAME_ID:-unset}"
echo "Using IWAD: $IWAD"
echo "Warp Level: ${WARP:-unset}"
echo "Using Game Skill: ${SKILL:-unset}"
echo "Using Data Root: ${DATA_ROOT:-unset}"
PORT="${GAME_PORT:-2342}"
PLAYERS="${PLAYERS:-4}"
TICDUP="${TICDUP:-1}"
XTRATICS="${XTRATICS:-1}"

CMD=(
    /usr/local/bin/dorch-game-server
    -p "$PORT"
    -N "$PLAYERS"
)

# Net smoothing knobs (PrBoomX server flags):
# -t <ticdup>: duplicate each tic this many times (helps with loss/jitter)
# -x <xtratics>: send extra tics (helps hide latency)
if [[ -n "${TICDUP}" && "${TICDUP}" != "0" && "${TICDUP}" != "1" ]]; then
    CMD+=( -t "$TICDUP" )
fi
if [[ -n "${XTRATICS}" && "${XTRATICS}" != "0" ]]; then
    CMD+=( -x "$XTRATICS" )
fi
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
        # prboomX-game-server uses -w to announce WADs to clients.
        # It does not need to read these files locally.
        echo "Adding WAD: $wad"
        CMD+=(-w "$wad")
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
