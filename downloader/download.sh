#!/bin/bash
set -euo pipefail

echo "wadinfo base url: $WADINFO_BASE_URL"
echo "S3 Endpoint: $S3_ENDPOINT"
echo "S3 Region:   $S3_REGION"
echo "S3 Bucket:   $S3_BUCKET"
echo "Wad IDs:     $DOWNLOAD_LIST" # comma-separated WAD IDs
echo "Data Root:   $DATA_ROOT"

mkdir -p $DATA_ROOT
cd $DATA_ROOT

download_all() {
    if [[ -z "${DOWNLOAD_LIST:-}" ]]; then
        echo "No DOWNLOAD_LIST provided; nothing to download." >&2
        return 0
    fi

    # Build JSON body: {"items": ["id1","id2",...]}
    # Requires jq. DOWNLOAD_LIST is comma-separated.
    json_body=$(jq -nc --arg ids "$DOWNLOAD_LIST" '
        {
          wad_ids: (
            $ids
            | split(",")
            | map(. | gsub("^\\s+|\\s+$"; ""))   # trim whitespace
            | map(select(. != ""))              # drop empties
          )
        }
    ')

    response=$(
        curl -sfSL \
            -X POST "$WADINFO_BASE_URL/wad_urls" \
            -H "Content-Type: application/json" \
            -d "$json_body"
    )

    echo "$response" \
        | jq -rc '.items[] | [.wad_id, .url] | @tsv' \
        | while IFS=$'\t' read -r wad_id url; do
            if [[ -z "$wad_id" || -z "$url" ]]; then
                echo "Skipping malformed item (wad_id or url empty)" >&2
                continue
            fi
            dst="$wad_id"
            aws s3 cp "$url" "$dst" \
                --endpoint-url "$S3_ENDPOINT" \
                --region "$S3_REGION" \
                --no-progress
        done
}

download_all
