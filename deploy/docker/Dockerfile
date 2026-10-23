# syntax=docker/dockerfile:1.7
# pley media server + transcoder, with FFmpeg 7 from Debian (NVENC, VAAPI,
# QSV, Vulkan/libplacebo tone mapping).
#
#   docker build -f deploy/docker/Dockerfile -t pley .
#   docker run -d -p 7539:7539 -v ./config:/config -v /srv/media:/media:ro pley
#
# See deploy/README.md for hardware transcoding.

# ---- web UI -----------------------------------------------------------------
FROM node:24-bookworm-slim AS web
WORKDIR /src
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/
RUN npm ci --no-fund --no-audit
COPY apps/web apps/web
RUN npm run build

# ---- Rust binaries ----------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY apps/server apps/server
COPY apps/transcoder apps/transcoder
COPY packages packages
COPY docs docs
COPY scripts scripts
COPY --from=web /src/apps/web/dist apps/web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p pley-server -p pley-transcoder \
    && install -Dm755 target/release/pley target/release/pley-transcoder -t /out/bin

# ---- runtime ----------------------------------------------------------------
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates ffmpeg tini \
        libvulkan1 mesa-vulkan-drivers mesa-va-drivers \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 1000 --user-group --home-dir /config --shell /usr/sbin/nologin pley \
    && mkdir -p /config /media && chown pley:pley /config

COPY --from=build /out/bin/ /usr/local/bin/
COPY apps/server/LICENSE /usr/share/doc/pley/LICENSE.txt
COPY LICENSING.md /usr/share/doc/pley/

ENV PLEY_DATA_DIR=/config \
    PLEY_BIND=0.0.0.0:7539 \
    NVIDIA_DRIVER_CAPABILITIES=compute,video,utility,graphics
VOLUME ["/config"]
EXPOSE 7539
USER pley
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s \
    CMD ["pley", "health"]
ENTRYPOINT ["tini", "--", "pley"]
