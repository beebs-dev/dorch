#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

: "${IWAD_BUCKET:?IWAD_BUCKET is required}"
: "${IWADS_DIR:?IWADS_DIR is required}"
: "${REDIS_HOST:?REDIS_HOST is required}"
: "${REDIS_PORT:?REDIS_PORT is required}"

mkdir -p "$IWADS_DIR"

aws_args=()
if [[ -n "${AWS_ENDPOINT_URL:-}" ]]; then
	aws_args+=(--endpoint-url "$AWS_ENDPOINT_URL")
fi

redis_args=(--no-auth-warning -h "$REDIS_HOST" -p "$REDIS_PORT")
if [[ -n "${REDIS_USERNAME:-}" ]]; then
	redis_args+=(--user "$REDIS_USERNAME")
fi
if [[ -n "${REDIS_PASSWORD:-}" ]]; then
	redis_args+=(-a "$REDIS_PASSWORD")
fi
if [[ "${REDIS_PROTO:-redis}" == "rediss" ]]; then
	redis_args+=(--tls)
fi

echo "Listing s3://$IWAD_BUCKET ..."
listing="$(aws "${aws_args[@]}" s3 ls "s3://$IWAD_BUCKET/" --recursive)"
echo "$listing"

while IFS= read -r key; do
	[[ -z "$key" ]] && continue

	name="$(basename "$key")"
	dest="$IWADS_DIR/$name"
	redis_key="dorch:iwad:${name}"

	exists="$(redis-cli "${redis_args[@]}" EXISTS "$redis_key")"
	if [[ "$exists" == "1" ]]; then
		echo "Redis cache hit: $redis_key -> $dest"
		redis-cli "${redis_args[@]}" --raw GET "$redis_key" > "$dest"
		continue
	fi

	echo "Redis cache miss: $redis_key; downloading s3://$IWAD_BUCKET/$key -> $dest"
	aws "${aws_args[@]}" s3 cp "s3://$IWAD_BUCKET/$key" "$dest"

	echo "Caching in Redis: $redis_key"
	redis-cli "${redis_args[@]}" -x SET "$redis_key" < "$dest" > /dev/null
done < <(printf '%s\n' "$listing" | awk '{print $4}')
