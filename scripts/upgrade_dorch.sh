#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
helm upgrade dorch ../chart \
    --kube-context do-nyc1-beebs \
    --create-namespace \
    --install \
    -n dorch \
    -f scripts/dorch_values.yaml
