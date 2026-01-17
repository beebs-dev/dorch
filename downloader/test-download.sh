#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"/
export WADINFO_BASE_URL=http://localhost:8000
export S3_ENDPOINT=https://nyc3.digitaloceanspaces.com
export S3_REGION=nyc3
export S3_BUCKET=wadarchive2
export DATA_ROOT=output
export DOWNLOAD_LIST=17bdc0a8-8a81-4b00-90d1-972bf406fa10,1011e3b6-de21-4f1d-b4fa-0df4bb56cf54
./download.sh