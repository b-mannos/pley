# Pley in Docker

Pley and its transcoder in one container, set up with Docker Compose for a
computer at home, behind a reverse proxy that handles TLS. Everything is in
this folder.

## Quick start

You need Docker Engine with the Compose plugin (or Docker Desktop).

```sh
git clone https://github.com/hhaidar/pley.git
cd pley/deploy/docker
./setup.sh
```

`setup.sh`:

1. writes `.env` on its first run (Pley runs as you, with hardware
   transcoding when Docker can use a GPU);
2. builds the image and starts Pley;
3. prints a link with a one-time setup code.

Open the link to create the administrator account, then add libraries in
**Settings → Libraries**, with folders under `/media` (`/media/Movies`, ...).
Media goes in `media/` here unless `.env` points elsewhere;
[media/README.md](media/README.md) shows how to name it.

Without the script, or on Windows:

```sh
cp .env.example .env        # then edit it: media folder, user, port
docker compose up -d
docker compose logs pley    # shows the setup link
```

The first build takes several minutes (it compiles Pley). Pley is then on
`http://127.0.0.1:7539`, on this computer only: point your reverse proxy at
it ([below](#behind-a-reverse-proxy)).

## What's here

| File | What |
|---|---|
| `compose.yaml` | The pley service, locked down ([Security](#security)) |
| `.env.example` | Every setting, with its default; copied to `.env` |
| `setup.sh` | First-run setup, start and update |
| `compose.vaapi.yaml` | Add-on: Intel / AMD hardware transcoding |
| `compose.nvidia.yaml` | Add-on: NVIDIA hardware transcoding |
| `compose.proxy.yaml` | Add-on: a reverse proxy in Docker, no published port |
| `compose.plex.yaml` | Add-on: the [compatibility API](../../docs/COMPATIBILITY.md) on port 32400 (else on Pley's) |
| `config/` | Pley's database, artwork and caches (default `PLEY_CONFIG_DIR`) |
| `media/` | An example media folder (default `PLEY_MEDIA_DIR`) |
| `Dockerfile` | The image: pley, pley-transcoder, FFmpeg 7.1, GPU drivers |

Add-ons are turned on in `.env`, with `COMPOSE_FILE` (setup.sh does it for
GPUs it finds):

```sh
COMPOSE_FILE=compose.yaml:compose.vaapi.yaml:compose.plex.yaml
```

On Windows, separate them with `;` instead of `:`. Your own changes go in
`compose.override.yaml` next to `compose.yaml` (git ignores it). Compose reads
it automatically unless `COMPOSE_FILE` is set; then add it to the list.

## Settings

All in `.env` ([.env.example](.env.example) explains each one):

| Setting | Default | What |
|---|---|---|
| `PLEY_CONFIG_DIR` | `./config` | Database, artwork, caches, transcodes |
| `PLEY_MEDIA_DIR` | `./media` | Your media, read-only at `/media` |
| `PLEY_UID`, `PLEY_GID` | `1000` | The user Pley runs as: owns the config folder, reads the media |
| `PLEY_LISTEN` | `127.0.0.1:7539` | Where the web port is published |
| `PLEY_RENDER_GID` | | Intel / AMD: the GPU render node's group |
| `PLEY_RENDER_DEVICE` | `/dev/dri/renderD128` | Intel / AMD: which GPU |
| `PLEY_PROXY_NETWORK` | `proxy` | `compose.proxy.yaml`: the proxy's Docker network |
| `PLEY_PLEX_LISTEN` | `127.0.0.1:32400` | `compose.plex.yaml`: where the compatibility API is published |
| `PLEY_IMAGE` | `pley:local` | The image; a release is `ghcr.io/hhaidar/pley:<version>` |
| `PLEY_PULL_POLICY` | `build` | `build` from this checkout, or `missing` for a release |
| `PLEY_SHM_SIZE` | `2gb` | `/dev/shm`: transcodes keep their segments in half of it |
| `PLEY_LOG` | `info` | Log detail (`debug` to look into a problem) |

Settings that change at runtime are in Pley's web app: the TMDB key in
**Settings → Server**; hardware acceleration, tone mapping and transcode
limits in **Settings → Transcoder**. **Settings → Network** only shows where
Pley listens inside the container, which the image and `compose.yaml` fix;
where it can be reached from is `PLEY_LISTEN` (and `PLEY_PLEX_LISTEN`) in
`.env`.

### More media folders

`PLEY_MEDIA_DIR` can be a folder holding all your disks (`/mnt`). Or mount
each extra folder at a path of its own in `compose.override.yaml`, outside
`/media` (which is read-only, so nothing can be mounted inside it):

```yaml
services:
  pley:
    volumes:
      - /mnt/archive/Movies:/archive/Movies:ro
      - /mnt/archive/Music:/archive/Music:ro
```

Always add `:ro`: Pley never writes to media folders.

## Behind a reverse proxy

Pley serves plain HTTP; your proxy adds TLS. What the proxy must pass on,
and Caddy and nginx examples, are in
[Apps and reverse proxies](../README.md#apps-and-reverse-proxies). Pley
trusts `X-Forwarded-For` from a proxy on this computer, your network or a
Docker network.

**A proxy on this computer, outside Docker:** point it at
`http://127.0.0.1:7539` (the default `PLEY_LISTEN`).

**A proxy in Docker** (Caddy, Traefik, Nginx Proxy Manager): turn on
`compose.proxy.yaml`. Pley joins the proxy's network, publishes no port, and
the proxy reaches it at `http://pley:7539`. Create the network once
(`docker network create proxy`) and attach the proxy to it, or set
`PLEY_PROXY_NETWORK` to the proxy's own. With Traefik, give Pley these labels
in `compose.override.yaml`:

```yaml
services:
  pley:
    labels:
      traefik.enable: "true"
      traefik.http.routers.pley.rule: Host(`pley.example.com`)
      traefik.http.routers.pley.entrypoints: websecure
      traefik.http.routers.pley.tls.certresolver: letsencrypt
      traefik.http.services.pley.loadbalancer.server.port: "7539"
```

**A proxy on another computer:** publish Pley on this computer's LAN address
(`PLEY_LISTEN=192.168.1.10:7539`) and let only the proxy reach it. Docker's
port rules go around ufw and firewalld: filter in the `DOCKER-USER` chain
instead, for example
`iptables -I DOCKER-USER -p tcp --dport 7539 ! -s <proxy address> -j DROP`.

Set Pley up (the link from `setup.sh`) before the proxy makes it reachable
from the internet.

## Hardware transcoding

- **Intel and AMD** (VAAPI, Quick Sync, Vulkan tone mapping):
  `compose.vaapi.yaml` hands Pley one GPU's render node.
  - `setup.sh` turns it on at its first run for the first Intel or AMD GPU it
    finds, and on later runs says when there is one Pley doesn't use.
  - By hand: add it to `COMPOSE_FILE` and set `PLEY_RENDER_GID` to the node's
    group (`stat -c %g /dev/dri/renderD128`). With several GPUs,
    `PLEY_RENDER_DEVICE` picks one (`ls -l /dev/dri/by-path` names them).
  - Quick Sync needs an 11th generation (Tiger Lake) or newer Intel GPU;
    older ones encode through VAAPI. Intel's drivers are in the amd64 image
    only.
- **NVIDIA** (NVENC, NVDEC, CUDA scaling, Vulkan tone mapping): install the
  driver and the
  [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html),
  then turn on `compose.nvidia.yaml` (`setup.sh` does when Docker has the
  toolkit).
- **HDR** is tone mapped on the GPU (libplacebo, through Vulkan) when the GPU
  has Vulkan, else on the CPU (zscale).
- **arm64** boards such as the Raspberry Pi have no encoder Pley uses: they
  transcode on the CPU.
- **Windows (Docker Desktop on WSL 2)** shows Linux the GPU as `/dev/dxg`,
  which Mesa's D3D12 VA-API driver encodes on (AMD, Intel or NVIDIA):
  - pass `/dev/dxg` and `/dev/dri/renderD128`, mount `/usr/lib/wsl`
    read-only, and set `LD_LIBRARY_PATH=/usr/lib/wsl/lib` and
    `LIBVA_DRIVER_NAME=d3d12`;
  - the render node there is root's only (`PLEY_UID=0`);
  - it takes Mesa 26: the image's Mesa 25.0 (Debian 13) can't open WSL's
    render node on the 6.18 kernel Docker Desktop runs ("MESA-LOADER:
    failed to retrieve device information"), and a newer libdrm alone
    doesn't help;
  - WSL has no Vulkan or OpenCL on the GPU, so HDR is tone mapped on the
    CPU, below real time for 4K. For HDR, or AMD's encoder (AMF), install
    Pley on Windows itself.

Pley logs the encoders it found at start
(`docker compose logs pley | grep detected`), and checks again when the GPUs
the container sees change. **Settings → Transcoder** picks the encoder
(Automatic takes the GPU's), and **Settings → Dashboard** shows each stream
as GPU, hybrid or CPU.

## Security

The container starts locked down, and nothing below needs changing for Pley
to work:

- **One port, on this computer only.** Pley is published on `127.0.0.1`, so
  the reverse proxy is the only way in from other computers, with its TLS and
  access rules. The compatibility API answers on that port too, to its
  clients only (with a token); network discovery is off, and the transcoder
  listens inside the container only, so nothing else can be reached.
- **Not root, no privileges.** Pley runs as your user, with every Linux
  capability dropped and `no-new-privileges`; the image has no setuid
  programs and never runs `privileged`.
- **Read-only everywhere but its data.** The image is mounted read-only and
  `/tmp` can't hold programs (`noexec`); media is read-only. Pley can only
  write its config folder, which `setup.sh` makes private (`chmod 700`): it
  holds the database and the key that encrypts two-step sign-in secrets.
- **Only the GPU, not the machine.** The Intel/AMD add-on hands over one
  render node; NVIDIA's goes through the toolkit.
- **Bounded.** Processes are capped (`pids_limit`) and Docker's log is rotated
  (3 × 10 MB), so a fault can't exhaust the host. On a machine that does
  other work, cap memory and CPU too in `compose.override.yaml`
  (`mem_limit: 8g`, `cpus: 6`; each transcode takes about 1 GB of memory).
- **Nothing private in the image.** The build context leaves out `config/`,
  `media/` and `.env`.

On Pley's side ([SECURITY.md](../../SECURITY.md) has the details):

- Set Pley up before it can be reached from the internet: until then, the
  setup code in its log is all it takes to claim it.
- Turn on two-step sign-in for administrators (**Account → Two-step
  sign-in**).
- Leave apps by other publishers off (**Settings → Server → Apps**) unless you
  need one. Give tools that use the compatibility API (Tautulli, Kometa) a
  sign-in of an account of their own, not an administrator's, unless they
  must change things.
- Keep Pley up to date (below): `setup.sh` also fetches newer base images,
  which bring Debian's security fixes for FFmpeg and the rest of the image.

## Everyday use

From this folder:

| To | Run |
|---|---|
| See the log | `docker compose logs -f pley` |
| Update | `git pull && ./setup.sh` |
| Stop, start | `docker compose stop`, `docker compose start` |
| Remove the container (data stays) | `docker compose down` |
| Set a lost password | `docker compose exec pley pley reset-password <user>` (it asks for the new one) |

**Backups:** Pley keeps the last 3 daily copies of its database in
`config/backups`. For a full backup, stop Pley and copy the whole config
folder: the database alone can't decrypt two-step sign-in without
`secret.key`.

```sh
docker compose stop && tar -czf pley-config-$(date +%F).tar.gz -C config . && docker compose start
```

**Using a published image** instead of building: set
`PLEY_IMAGE=ghcr.io/hhaidar/pley:<version>` and `PLEY_PULL_POLICY=missing`;
update with `docker compose pull && docker compose up -d`.

## Troubleshooting

- **The log says `Permission denied` for `/config`:** the config folder isn't
  owned by `PLEY_UID`. `sudo chown -R <PLEY_UID>:<PLEY_GID> config`, or set
  `PLEY_UID`/`PLEY_GID` to its owner (`ls -ln`).
- **A library stays empty, or "cannot open /media/...":** `PLEY_UID` can't
  read the media. If a group can (`ls -ln`), put its id in `PLEY_GID`.
- **`bind source path does not exist`:** a folder in `.env` is missing.
  Compose doesn't create it for you (it would belong to root).
- **`port is already allocated`:** something else uses 7539; change the port
  in `PLEY_LISTEN` (`127.0.0.1:7600`) and in your proxy.
- **Transcodes stay on the CPU:** the log's `detected` line lists the
  encoders Pley found (`h264_vaapi`, `h264_qsv`, `h264_nvenc`...). For Intel
  and AMD, `docker compose exec pley ls -l /dev/dri` should list
  `renderD128`, with `PLEY_RENDER_GID` its group on the host. For NVIDIA,
  `docker compose exec pley nvidia-smi` should list the GPU.
- **Rootless Docker:** `PLEY_UID=0` and `PLEY_GID=0` (what `setup.sh` picks):
  root in a rootless container is your own user outside it. The host's
  render group doesn't carry over into it, so `setup.sh` leaves Intel and
  AMD transcoding off there.
