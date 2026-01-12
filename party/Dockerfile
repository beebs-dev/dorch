ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/synapse-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY party/src party/src
WORKDIR /app/party
RUN cargo build --release --bin dorch-party

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-party /usr/local/bin/dorch-party
CMD ["dorch-party"]
