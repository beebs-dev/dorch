ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY webrtc-auth/src webrtc-auth/src
WORKDIR /app/webrtc-auth
RUN cargo build --release --bin dorch-webrtc-auth

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-webrtc-auth /usr/local/bin/dorch-webrtc-auth
CMD ["dorch-webrtc-auth"]
