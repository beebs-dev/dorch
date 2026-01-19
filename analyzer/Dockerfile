ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY wadinfo/src wadinfo/src
COPY analyzer/src analyzer/src
WORKDIR /app/analyzer
RUN cargo build --release --bin dorch-analyzer

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-analyzer /usr/local/bin/dorch-analyzer
CMD ["dorch-analyzer"]
