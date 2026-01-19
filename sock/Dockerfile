ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY sock/src sock/src
WORKDIR /app/sock
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-sock && \
    cp /app/target/release/dorch-sock /app/dorch-sock && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
RUN apt update && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/* && apt clean
ENV SOCK_ENDPOINT=http://localhost:80
COPY --from=builder /app/dorch-sock /usr/local/bin/dorch-sock
CMD ["dorch-sock"]
