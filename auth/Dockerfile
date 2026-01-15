ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY types/src types/src
COPY auth/src auth/src
WORKDIR /app/auth
RUN cargo build --release --bin dorch-auth

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-auth /usr/local/bin/dorch-auth
CMD ["dorch-auth"]
