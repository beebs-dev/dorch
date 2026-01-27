#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

: "${CONTEXT:=do-nyc3-beeb}"

secret() {
    kubectl get secret --context "$CONTEXT" -n "$1" "$2" -o json \
    | jq .data."$3" \
    | xargs echo \
    | base64 -d
}

old_secret() {
    secret old postgres-cred "$1"
}

new_secret() {
    secret dorch postgres-cred "$1"
}

# ---- Source (old cluster) ----
SRC_HOST=$(old_secret host)
SRC_USER=$(old_secret username)
SRC_PASS=$(old_secret password)
SRC_PORT=$(old_secret port)
SRC_DB=slop
SRC_SSLMODE=$(old_secret sslmode)

# ---- Destination (new cluster) ----
DEST_HOST=$(new_secret host)
DEST_USER=$(new_secret username)
DEST_PASS=$(new_secret password)
DEST_PORT=$(new_secret port)
DEST_DB=slop
DEST_SSLMODE=$(new_secret sslmode)

export SRC_HOST SRC_USER SRC_PASS SRC_PORT SRC_DB SRC_SSLMODE
export DEST_HOST DEST_USER DEST_PASS DEST_PORT DEST_DB DEST_SSLMODE

echo "Migrating database with pg_dump | pg_restore"
echo "  From: $SRC_USER@$SRC_HOST:$SRC_PORT/$SRC_DB (sslmode=$SRC_SSLMODE)"
echo "    To: $DEST_USER@$DEST_HOST:$DEST_PORT/$DEST_DB (sslmode=$DEST_SSLMODE)"
echo

docker run --rm -i \
  -e SRC_HOST -e SRC_USER -e SRC_PASS -e SRC_PORT -e SRC_DB -e SRC_SSLMODE \
  -e DEST_HOST -e DEST_USER -e DEST_PASS -e DEST_PORT -e DEST_DB -e DEST_SSLMODE \
  postgres:18 bash -c '
    set -euo pipefail

    echo "Starting pg_dump from source..." >&2
    PGPASSWORD="$SRC_PASS" PGSSLMODE="$SRC_SSLMODE" pg_dump \
      -h "$SRC_HOST" \
      -p "$SRC_PORT" \
      -U "$SRC_USER" \
      -d "$SRC_DB" \
      -Fc -v \
      --no-owner --no-privileges \
    | PGPASSWORD="$DEST_PASS" PGSSLMODE="$DEST_SSLMODE" pg_restore \
      -h "$DEST_HOST" \
      -p "$DEST_PORT" \
      -U "$DEST_USER" \
      -d "$DEST_DB" \
      -v \
      --no-owner --no-privileges

    echo "Migration completed successfully." >&2
  '
