ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY wadinfo/src wadinfo/src
WORKDIR /app/wadinfo
RUN cargo build --release --bin dorch-wadinfo

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-wadinfo /usr/local/bin/dorch-wadinfo
CMD ["dorch-wadinfo"]
