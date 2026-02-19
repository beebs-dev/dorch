#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
: "${CONTEXT:=do-nyc3-beeb}"
secret() {
    kubectl get secret --context $CONTEXT -n dorch postgres-cred -o json \
    | jq .data.$1 \
    | xargs echo \
    | base64 -d
}
export POSTGRES_HOST=$(secret host)
export POSTGRES_USER=$(secret username)
export POSTGRES_PASSWORD=$(secret password)
export POSTGRES_PORT=$(secret port)
export POSTGRES_DB=$(secret database)
export POSTGRES_SSL_MODE=$(secret sslmode)
docker run --rm \
    -e PGPASSWORD="$POSTGRES_PASSWORD" \
    -e PGSSLMODE="$POSTGRES_SSL_MODE" \
    postgres:18 \
    pg_dump \
        -h "$POSTGRES_HOST" \
        -p "$POSTGRES_PORT" \
        -U "$POSTGRES_USER" \
        -d dorch \
    > dorch.sql