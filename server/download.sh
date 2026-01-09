#!/bin/bash
set -euo pipefail
echo "S3 Endpoint: $S3_ENDPOINT"
echo "S3 Region: $S3_REGION"
echo "Download List: $DOWNLOAD_LIST"
echo "Data Root: $DATA_ROOT"
download() {
    local key="$1"
    local dst="$DATA_ROOT/$key"
    mkdir -p "$(dirname "$dst")"
    aws s3 cp "s3://$key" "$dst" \
        --endpoint-url "$S3_ENDPOINT" \
        --region "$S3_REGION" \
        --no-progress
    echo "Downloaded $key to $dst"
}
IFS=',' read -r -a WADS <<< "$DOWNLOAD_LIST"
for wad in "${WADS[@]}"; do
    wad="${wad#"${wad%%[![:space:]]*}"}"  # ltrim
    wad="${wad%"${wad##*[![:space:]]}"}"  # rtrim
    echo "Downloading WAD: $wad"
    download "$wad"
done
