#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
# TODO: aws s3 ls the bucket, try to get from redis, fallback to s3, cache in redis (if necessary), etc.
