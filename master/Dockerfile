ARG RUNTIME_IMAGE=debian:trixie-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
ENV SCCACHE_SERVER_PORT=4229
COPY common/src common/src
COPY auth/src auth/src
COPY types/src types/src
COPY master/src master/src
WORKDIR /app/master
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-master && \
    cp /app/target/release/dorch-master /app/dorch-master && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/dorch-master /usr/local/bin/dorch-master
CMD ["dorch-master"]
