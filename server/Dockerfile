ARG RUNTIME_IMAGE=ubuntu:24.04
FROM ${RUNTIME_IMAGE}
RUN apt-get update && \
    apt-get install -y --no-install-recommends woof-doom ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY server.sh /server.sh
COPY download.sh /download.sh
RUN chmod +x /server.sh /download.sh
EXPOSE 5030/udp
CMD ["/server.sh"]