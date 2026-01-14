#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"/..
secret() {
    echo $(./scripts/get-secret-value.sh redis-cred $1)
}

# This image is just redis:7 with ca-certificates installed.
# It is needed because DigitalOcean redis uses SSL.
#echo "Testing with thavlik/redli:latest..."
#docker run -it thavlik/redli:latest \
#    redli \
#        --tls \
#        -a "$(secret password)" \
#        -h "$(secret host)" \
#        -p "$(secret port)" \
#        GET foo
# docker run -it valkey/valkey:latest \
#     valkey-cli \
#         --tls \
#         --insecure \
#         -a "$(secret password)" \
#         -h "$(secret host)" \
#         -p "$(secret port)" \
#         --raw \
#         --no-auth-warning \
#         GET d29054e9933a5abe7d7febee4e5aba3b > image.png
KEY_NAME=foo
IMAGE_PATH=/mnt/kuchera-thumbnail.jpeg

# docker run -i -v "$(pwd)/fixtures:/mnt" redis:7 \
#     bash -c "redis-cli \
#         --tls \
#         --insecure \
#         --no-auth-warning \
#         --raw \
#         -x \
#         -a \"$(secret password)\" \
#         -h \"$(secret host)\" \
#         -p \"$(secret port)\" \
#         SET ${KEY_NAME} < /mnt/kuchera-thumbnail.jpeg"
# docker run -i -v "$(pwd)/fixtures:/mnt" redis:7 \
#     redis-cli \
#         --tls \
#         --insecure \
#         --raw \
#         --no-auth-warning \
#         -a "$(secret password)" \
#         -h "$(secret host)" \
#         -p "$(secret port)" \
#         GET ${KEY_NAME} > ./fixtures/kuchera-retrieved.jpeg

docker run --rm -it -v "$(pwd)/fixtures:/mnt" redis:7 \
    redis-cli \
        --tls \
        --insecure \
        --no-auth-warning \
        -a "$(secret password)" \
        -h "$(secret host)" \
        -p "$(secret port)"