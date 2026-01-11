ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY sock/src sock/src
WORKDIR /app/sock
RUN cargo build --release --bin dorch-sock

FROM ${RUNTIME_IMAGE}
RUN apt update && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/* && apt clean
ENV SOCK_ENDPOINT=http://localhost:80
COPY --from=builder /app/target/release/dorch-sock /usr/local/bin/dorch-sock
CMD ["dorch-sock"]
