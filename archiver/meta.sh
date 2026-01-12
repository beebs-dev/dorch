#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
source ../.venv/bin/activate
python3 meta.py \
  --wads-json ../../wads.json \
  --idgames-json ../../idgames.json \
  --pretty \
  --limit 50
