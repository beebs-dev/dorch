ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY types/src types/src
COPY master/src master/src
WORKDIR /app/master
RUN cargo build --release --bin dorch-master

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-master /usr/local/bin/dorch-master
CMD ["dorch-master"]
