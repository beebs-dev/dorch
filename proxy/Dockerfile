ARG RUNTIME_IMAGE=ubuntu:24.04
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY proxy/src proxy/src
COPY common/src common/src
WORKDIR /app/proxy
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-proxy && \
    cp /app/target/release/dorch-proxy /app/dorch-proxy && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
COPY --from=builder /app/dorch-proxy /usr/local/bin/dorch-proxy
CMD ["dorch-proxy", "server"]
