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
	# If another concurrent process already populated the file, skip.
	if [[ -s "$dest" ]]; then
		# Ensure readability for the main container. mktemp creates 0600 files,
		# and those permissions can persist across pod restarts on hostPath.
		chmod a+r "$dest" 2>/dev/null || true
		continue
	fi
	exists="$(redis-cli "${redis_args[@]}" EXISTS "$redis_key")"
	if [[ "$exists" == "1" ]]; then
		tmp="$(mktemp "$IWADS_DIR/.${name}.tmp.XXXXXX")"
		trap 'rm -f "$tmp"' RETURN
		redis-cli "${redis_args[@]}" --raw GET "$redis_key" > "$tmp" || true
		# If the redis value was empty/missing, fall back to S3.
		if [[ -s "$tmp" ]]; then
			if mv -n "$tmp" "$dest" 2>/dev/null; then
				chmod a+r "$dest" 2>/dev/null || true
				trap - RETURN
				continue
			fi
		fi
		rm -f "$tmp"
		trap - RETURN
		continue
	fi
	tmp="$(mktemp "$IWADS_DIR/.${name}.tmp.XXXXXX")"
	trap 'rm -f "$tmp"' RETURN
	aws "${aws_args[@]}" s3 cp "s3://$IWAD_BUCKET/$key" "$tmp"
	# Atomic publish into place (avoid clobbering if another process won the race).
	if mv -n "$tmp" "$dest" 2>/dev/null; then
		chmod a+r "$dest" 2>/dev/null || true
		trap - RETURN
		redis-cli "${redis_args[@]}" -x SET "$redis_key" < "$dest" > /dev/null
		continue
	fi
	# Destination already created by another process; don't overwrite.
	rm -f "$tmp"
	trap - RETURN
done < <(printf '%s\n' "$listing" | awk '{print $4}')
