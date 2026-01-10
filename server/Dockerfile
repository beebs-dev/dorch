ARG RUNTIME_IMAGE=ubuntu:24.04
FROM ${RUNTIME_IMAGE} AS builder
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl unzip && \
    rm -rf /var/lib/apt/lists/*
RUN curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "/tmp/awscliv2.zip" && \
    unzip /tmp/awscliv2.zip -d /tmp

FROM ${RUNTIME_IMAGE}
RUN apt-get update && \
    apt-get install -y --no-install-recommends woof-doom ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/aws /tmp/aws
RUN /tmp/aws/install && \
    rm -rf /tmp/aws
COPY server.sh /server.sh
COPY download.sh /download.sh
RUN chmod +x /server.sh /download.sh
EXPOSE 2342/udp
CMD ["/server.sh"]