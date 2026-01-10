ARG RUNTIME_IMAGE=ubuntu:24.04
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY server/src server/src
WORKDIR /app/server
RUN cargo build --release --bin dorch-server

FROM ${RUNTIME_IMAGE}
RUN apt-get update && \
    apt-get install -y --no-install-recommends woof-doom ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY server/server.sh /server.sh
COPY server/download.sh /download.sh
RUN chmod +x /server.sh /download.sh
COPY --from=builder /app/target/release/dorch-server /usr/local/bin/dorch-server
EXPOSE 5030/udp
CMD ["/server.sh"]
