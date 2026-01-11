#!/bin/bash
cd "$(dirname "$0")"
set -euo pipefail
docker build -t thavlik/omtfs:latest -f ./Dockerfile ../
