#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source .venv/bin/activate
rm -rf output
python main.py \
    --iwad ./assets/freedoom2.wad \
    --files ./assets/sunlust.wad \
    --output output \
    --panorama \
    --no-monsters \
    -n 6