FROM ubuntu:24.04

ENV DOOM_PORT=5030
ENV IWAD_PATH=/usr/local/share/games/doom/doom2.wad

RUN apt-get update && \
    apt-get install -y --no-install-recommends woof-doom ca-certificates && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 5030/udp

# NOTE: JSON-form CMD does NOT expand env vars, so either hardcode
# or use /bin/sh -c.
CMD ["/bin/sh", "-c", "woof -iwad \"$IWAD_PATH\" -server -dedicated -port \"$DOOM_PORT\""]
