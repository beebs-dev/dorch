ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY iam-client/src iam-client/src
COPY iam/src iam/src
WORKDIR /app/iam
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    sccache --start-server && \
    cargo build --release --bin dorch-iam && \
    cp /app/target/release/dorch-iam /app/dorch-iam && \
    sccache --show-stats

FROM ${RUNTIME_IMAGE}
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/dorch-iam /usr/local/bin/dorch-iam
CMD ["dorch-iam"]
