ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY iam-client/src iam-client/src
COPY iam/src iam/src
WORKDIR /app/iam
RUN cargo build --release --bin dorch-iam

FROM ${RUNTIME_IMAGE}
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-iam /usr/local/bin/dorch-iam
CMD ["dorch-iam"]
