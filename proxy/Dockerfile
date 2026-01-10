ARG RUNTIME_IMAGE=ubuntu:24.04
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY proxy/src proxy/src
COPY common/src common/src
WORKDIR /app/proxy
RUN cargo build --release --bin dorch-proxy

FROM ${RUNTIME_IMAGE}
COPY --from=builder /app/target/release/dorch-proxy /usr/local/bin/dorch-proxy
CMD ["dorch-proxy"]
