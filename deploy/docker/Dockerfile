# syntax=docker/dockerfile:1.7
# pley media server and its transcoder, on Debian 13 with its FFmpeg 7.1:
# NVIDIA (NVENC, CUDA scaling), Intel (Quick Sync, VAAPI) and AMD (VAAPI)
# transcoding, and HDR tone mapping on the GPU (libplacebo over Vulkan,
# OpenCL) or the CPU (zscale).
#
#   docker build -f deploy/docker/Dockerfile -t pley .
#   docker run -d -p 127.0.0.1:7539:7539 -v ./config:/config -v /srv/media:/media:ro pley
#
# deploy/docker/README.md sets it up with Docker Compose, hardened, and
# covers hardware transcoding.

# ---- web app ----------------------------------------------------------------
FROM node:24-trixie-slim AS web
# The commit apps show beside the version (scripts/commit.mjs): the build
# context has no .git.
ARG PLEY_COMMIT=unknown
ENV PLEY_COMMIT=$PLEY_COMMIT
WORKDIR /src
COPY VERSION package.json package-lock.json ./
COPY scripts/commit.mjs scripts/commit.d.mts scripts/
COPY apps/web/package.json apps/web/
RUN npm ci --no-fund --no-audit
COPY apps/web apps/web
RUN npm run build

# ---- Rust binaries ----------------------------------------------------------
FROM rust:1-trixie AS build
ARG PLEY_COMMIT=unknown
ENV PLEY_COMMIT=$PLEY_COMMIT
WORKDIR /src
COPY VERSION Cargo.toml Cargo.lock rustfmt.toml ./
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
ARG PLEY_COMMIT=unknown
LABEL org.opencontainers.image.title="pley" \
      org.opencontainers.image.description="pley media server, with its transcoder and FFmpeg" \
      org.opencontainers.image.source="https://github.com/hhaidar/pley" \
      org.opencontainers.image.documentation="https://github.com/hhaidar/pley/blob/master/deploy/docker/README.md" \
      org.opencontainers.image.licenses="LicenseRef-Restricted" \
      org.opencontainers.image.revision=$PLEY_COMMIT
# Mesa brings VAAPI for AMD and Vulkan for AMD and Intel. Intel's own VAAPI
# driver and the runtime FFmpeg's Quick Sync encoders load (oneVPL) exist for
# amd64 only. NVIDIA's container toolkit brings its driver's libraries; the
# OpenCL loader learns of NVIDIA's from the file below. No program keeps a
# setuid or setgid bit: nothing in the image can gain privileges, even when
# it is run without no-new-privileges.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates ffmpeg tini \
        libvulkan1 mesa-vulkan-drivers mesa-va-drivers \
    && if [ "$(dpkg --print-architecture)" = amd64 ]; then \
        apt-get install -y --no-install-recommends intel-media-va-driver libmfx-gen1.2; \
    fi \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /etc/OpenCL/vendors && echo libnvidia-opencl.so.1 > /etc/OpenCL/vendors/nvidia.icd \
    && find / -xdev -perm /6000 -type f -exec chmod a-s {} + \
    && useradd --uid 1000 --user-group --no-create-home --home-dir /config --shell /usr/sbin/nologin pley \
    && mkdir -p /config /media && chown pley:pley /config && chmod 700 /config

COPY --from=build /out/bin/ /usr/local/bin/
COPY apps/server/LICENSE /usr/share/doc/pley/LICENSE.txt
COPY LICENSING.md /usr/share/doc/pley/

# HOME is /config for any user the container runs as (`--user`), so the GPU
# drivers' shader caches and fontconfig's land in the one writable folder.
ENV PLEY_DATA_DIR=/config \
    PLEY_BIND=0.0.0.0:7539 \
    HOME=/config \
    NVIDIA_DRIVER_CAPABILITIES=compute,video,utility,graphics
VOLUME ["/config"]
# The web app and its API; the Plex-compatible API.
EXPOSE 7539 32400
USER pley
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --start-interval=2s \
    CMD ["pley", "health"]
ENTRYPOINT ["tini", "--", "pley"]
