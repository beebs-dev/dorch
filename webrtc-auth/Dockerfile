ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY webrtc-auth/src webrtc-auth/src
WORKDIR /app/webrtc-auth
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-webrtc-auth && \
    cp /app/target/release/dorch-webrtc-auth /app/dorch-webrtc-auth && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/dorch-webrtc-auth /usr/local/bin/dorch-webrtc-auth
CMD ["dorch-webrtc-auth"]
