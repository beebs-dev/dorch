#!/bin/bash
set -euo pipefail

echo "wadinfo base url: $WADINFO_BASE_URL"
echo "S3 Endpoint:      $S3_ENDPOINT"
echo "S3 Region:        $S3_REGION"
echo "S3 Bucket:        $S3_BUCKET"
echo "Wad IDs:          $DOWNLOAD_LIST" # comma-separated WAD IDs
echo "Data Root:        $DATA_ROOT"
echo "IWAD Override:    $IWAD_OVERRIDE"

mkdir -p "$DATA_ROOT"
cd "$DATA_ROOT"

download_iwad_override() {
    if [[ -z "${IWAD_OVERRIDE:-}" ]]; then
        echo "No IWAD_OVERRIDE provided; skipping." >&2
        return 0
    fi
    url="s3://$S3_BUCKET/iwads/$IWAD_OVERRIDE"
    dst="$DATA_ROOT/$IWAD_OVERRIDE"
    aws s3 cp "$url" "$dst" \
        --endpoint-url "$S3_ENDPOINT" \
        --region "$S3_REGION" \
        --no-progress
    echo "Downloaded IWAD override to $dst"
}

download_all() {
    if [[ -z "${DOWNLOAD_LIST:-}" ]]; then
        echo "No DOWNLOAD_LIST provided; nothing to download." >&2
        return 0
    fi

    # Build JSON body: {"wad_ids": ["id1","id2",...]}
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

        # Derive extension from *inner* filename (strip a trailing .gz)
        filename=$(basename "${url%%\?*}")   # strip ?query if present
        base="${filename%.gz}"               # if ends with .gz, remove it
        ext="${base##*.}"                    # extension from inner name

        [[ "$base" == "$ext" ]] && ext=""    # no dot -> no extension

        if [[ -n "$ext" ]]; then
            dst="${wad_id}.${ext}"
        else
            dst="$wad_id"
        fi

        if [[ "$url" == *.gz ]]; then
            echo "Downloading and extracting $url -> $dst"
            aws s3 cp "$url" - \
                --endpoint-url "$S3_ENDPOINT" \
                --region "$S3_REGION" \
                --no-progress \
                | gzip -dc > "$dst"
        else
            echo "Downloading $url -> $dst"
            aws s3 cp "$url" "$dst" \
                --endpoint-url "$S3_ENDPOINT" \
                --region "$S3_REGION" \
                --no-progress
        fi
    done
}

download_iwad_override
download_all

echo "Download complete. Files in $DATA_ROOT:"
ls -al "$DATA_ROOT"
