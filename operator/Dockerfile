ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY types/src types/src
COPY operator/src operator/src
WORKDIR /app/operator
RUN cargo build --release --bin dorch-operator
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-operator && \
    cp /app/target/release/dorch-operator /app/dorch-operator && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
COPY --from=builder /app/dorch-operator /usr/local/bin/dorch-operator
CMD ["dorch-operator"]