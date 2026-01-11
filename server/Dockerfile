ARG RUNTIME_IMAGE=ubuntu:24.04

# Build the native Linux game server binary from Dwasm.
FROM ${RUNTIME_IMAGE} AS dwasm_build
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates \
      cmake \
      build-essential \
      pkg-config \
      libsdl2-dev \
      libsdl2-net-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy only the Dwasm tree (build context is repo root).
COPY Dwasm/ /src/Dwasm/

RUN mkdir -p /out \
        && cmake -S /src/Dwasm -B /tmp/build \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_SERVER=ON \
      -DWITH_NET=ON \
      -DWITH_IMAGE=OFF \
      -DWITH_MIXER=OFF \
      -DWITH_PCRE=OFF \
      -DWITH_ZLIB=OFF \
      -DWITH_MAD=OFF \
      -DWITH_FLUIDSYNTH=OFF \
      -DWITH_DUMB=OFF \
    && cmake --build /tmp/build --target prboomX-game-server -j \
    && install -m 0755 /tmp/build/prboomX-game-server /out/dorch-game-server

# Build AWS CLI in a separate stage (keeps runtime small and avoids build deps).
FROM ${RUNTIME_IMAGE} AS awscli_build
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl unzip && \
    rm -rf /var/lib/apt/lists/*
RUN curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "/tmp/awscliv2.zip" && \
    unzip /tmp/awscliv2.zip -d /tmp

FROM ${RUNTIME_IMAGE}
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates \
      libsdl2-2.0-0 \
      libsdl2-net-2.0-0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=dwasm_build /out/dorch-game-server /usr/local/bin/dorch-game-server

COPY --from=awscli_build /tmp/aws /tmp/aws
RUN /tmp/aws/install && \
    rm -rf /tmp/aws

COPY server/server.sh /server.sh
COPY server/download.sh /download.sh
RUN chmod +x /server.sh /download.sh

EXPOSE 2342/udp
CMD ["/server.sh"]