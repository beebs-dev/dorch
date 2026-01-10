ARG RUNTIME_IMAGE=debian:bookworm-slim
ARG BASE_IMAGE=thavlik/dorch-base:latest

FROM ${BASE_IMAGE} AS builder
COPY common/src common/src
COPY types/src types/src
COPY operator/src operator/src
WORKDIR /app/operator
RUN cargo build --release --bin dorch-operator

FROM ${RUNTIME_IMAGE}
COPY --from=builder /app/target/release/dorch-operator /usr/local/bin/dorch-operator
CMD ["dorch-operator"]