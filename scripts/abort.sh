#!/usr/bin/env bash
set -euo pipefail

# ---- CONFIG ----
BUCKET="wadarchive2"
REGION="nyc3"   # e.g. nyc3, sfo3, ams3
ENDPOINT="https://${REGION}.digitaloceanspaces.com"
# ----------------

echo "Listing multipart uploads in bucket: $BUCKET"
echo "Endpoint: $ENDPOINT"
echo

uploads=$(aws s3api list-multipart-uploads \
  --endpoint-url "$ENDPOINT" \
  --bucket "$BUCKET" \
  --output json)

count=$(echo "$uploads" | jq '.Uploads | length')

if [[ "$count" -eq 0 ]]; then
  echo "No multipart uploads found."
  exit 0
fi

echo "Found $count multipart uploads"
echo

echo "$uploads" | jq -r '.Uploads[] | [.Key, .UploadId] | @tsv' | while IFS=$'\t' read -r key upload_id; do
  echo "Aborting:"
  echo "  Key: $key"
  echo "  UploadId: $upload_id"

  aws s3api abort-multipart-upload \
    --endpoint-url "$ENDPOINT" \
    --bucket "$BUCKET" \
    --key "$key" \
    --upload-id "$upload_id"

  echo "  ✔ aborted"
  echo
done

echo "All multipart uploads aborted."
