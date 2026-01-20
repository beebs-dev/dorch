#!/bin/bash
set -euo pipefail

while true; do
    (
        cd "$(dirname "$0")"
        source ../.venv/bin/activate
        python download_wads_local.py "$@"
    ) && break

    echo "Script failed; retrying in 5 seconds..." >&2
    sleep 5
done
