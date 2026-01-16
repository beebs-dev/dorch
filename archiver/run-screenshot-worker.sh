#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ../.venv/bin/activate
export WADINFO_BASE_URL=http://localhost:8000
export DORCH_PANORAMA="1"
export DORCH_SCREENSHOT_COUNT="1"
export IWADS_DIR="/home/thavlik/Repositories/wads"
export DOOMWADDIR="/home/thavlik/Repositories/wads"
export TMPDIR="/tmp/dorch"
export NATS_URL=nats://localhost:4222
export NATS_USER=app
export NATS_PASSWORD=devpass
export DORCH_METRICS_ENABLED=false
python ./screenshot-worker.py