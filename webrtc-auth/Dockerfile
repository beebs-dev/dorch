ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/synapse-base:latest

FROM ${BASE_IMAGE} AS builder
COPY seer/common/src seer/common/src
COPY common/src common/src
COPY iam/src iam/src
COPY iam-client/src iam-client/src
COPY scribe/src scribe/src
COPY webrtc-auth/src webrtc-auth/src
WORKDIR /app/webrtc-auth
RUN cargo build --release --bin dorch-party

FROM ${RUNTIME_IMAGE}
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dorch-party /usr/local/bin/dorch-party
CMD ["dorch-party"]
