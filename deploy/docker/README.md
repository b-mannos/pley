# pley in Docker

pley and its transcoder in one container, set up with Docker Compose for a
computer at home, behind a reverse proxy that handles TLS. Everything is in
this folder.

## Quick start

You need Docker Engine with the Compose plugin (or Docker Desktop).

```sh
git clone https://github.com/hhaidar/pley.git
cd pley/deploy/docker
./setup.sh
```

`setup.sh` writes `.env` on its first run (pley runs as you, with hardware
transcoding when Docker can use a GPU), builds the image, starts pley and
prints a link with a one-time setup code. Open it to create the administrator
account, then add libraries in **Settings → Libraries**, with folders under
`/media` (`/media/Movies`, ...). Media goes in `media/` here unless `.env`
points elsewhere; [media/README.md](media/README.md) shows how to name it.

Without the script, or on Windows:

```sh
cp .env.example .env        # then edit it: media folder, user, port
docker compose up -d
docker compose logs pley    # shows the setup link
```

The first build takes several minutes (it compiles pley). pley is then on
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
| `compose.plex.yaml` | Add-on: the [Plex-compatible API](../../docs/PLEX.md) on port 32400 |
| `config/` | pley's database, artwork and caches (default `PLEY_CONFIG_DIR`) |
| `media/` | An example media folder (default `PLEY_MEDIA_DIR`) |
| `Dockerfile` | The image: pley, pley-transcoder, FFmpeg 7, GPU drivers |

Add-ons are turned on in `.env`, with `COMPOSE_FILE` (setup.sh does it for
GPUs it finds):

```sh
COMPOSE_FILE=compose.yaml:compose.vaapi.yaml:compose.plex.yaml
```

Separate them with `;` instead of `:` on Windows. Your own changes go in a
`compose.override.yaml` next to `compose.yaml` (git ignores it). Compose reads
it by itself unless `COMPOSE_FILE` is set; then add it to the list.

## Settings

All in `.env` ([.env.example](.env.example) explains each one):

| Setting | Default | What |
|---|---|---|
| `PLEY_CONFIG_DIR` | `./config` | Database, artwork, caches, transcodes |
| `PLEY_MEDIA_DIR` | `./media` | Your media, read-only at `/media` |
| `PLEY_UID`, `PLEY_GID` | `1000` | The user pley runs as: owns the config folder, reads the media |
| `PLEY_LISTEN` | `127.0.0.1:7539` | Where the web port is published |
| `PLEY_RENDER_GID` | | Intel / AMD: the GPU render node's group |
| `PLEY_RENDER_DEVICE` | `/dev/dri/renderD128` | Intel / AMD: which GPU |
| `PLEY_PROXY_NETWORK` | `proxy` | `compose.proxy.yaml`: the proxy's Docker network |
| `PLEY_PLEX_LISTEN` | `127.0.0.1:32400` | `compose.plex.yaml`: where the Plex API is published |
| `PLEY_IMAGE` | `pley:local` | The image; a release is `ghcr.io/hhaidar/pley:<version>` |
| `PLEY_PULL_POLICY` | `build` | `build` from this checkout, or `missing` for a release |
| `PLEY_LOG` | `info` | Log detail (`debug` to look into a problem) |

Settings that change at runtime (TMDB key, hardware acceleration, tone
mapping, transcode limits) are in pley's web UI, in **Settings → Server**.

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

Always add `:ro`: pley never writes to media folders.

## Behind a reverse proxy

pley serves plain HTTP; your proxy adds TLS. It must:

- pass the original `Host` header and set `X-Forwarded-Proto`, replacing what
  clients send: pley names itself after the address each request came to, and
  marks its cookies `Secure` behind HTTPS;
- add the client's address to `X-Forwarded-For` (pley trusts it only from a
  proxy on this computer, your network or a Docker network);
- not buffer responses: live updates are server-sent events, and videos
  stream as they are read;
- accept uploads of up to 25 MB (artwork).

**Caddy** does all of it by default:

```caddyfile
pley.example.com {
	reverse_proxy 127.0.0.1:7539
}
```

**nginx:**

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name pley.example.com;
    # ssl_certificate, ssl_certificate_key ...

    client_max_body_size 25m;

    location / {
        proxy_pass http://127.0.0.1:7539;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_buffering off;
    }
}
```

**A proxy in Docker** (Caddy, Traefik, nginx Proxy Manager): turn on
`compose.proxy.yaml`. pley then joins the proxy's network and publishes no
port at all; the proxy reaches it at `http://pley:7539`. Create the network
once (`docker network create proxy`) and attach the proxy to it, or set
`PLEY_PROXY_NETWORK` to the proxy's own. With Traefik, give pley these labels
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

**A proxy on another computer:** publish pley on this computer's LAN address
(`PLEY_LISTEN=192.168.1.10:7539`) and let only the proxy reach it. Docker's
port rules go around ufw and firewalld: filter in the `DOCKER-USER` chain
instead, for example
`iptables -I DOCKER-USER -p tcp --dport 7539 ! -s <proxy address> -j DROP`.

Set pley up (the link from `setup.sh`) before the proxy makes it reachable
from the internet.

## Hardware transcoding

- **Intel and AMD** (VAAPI, Quick Sync, Vulkan tone mapping): `setup.sh`
  turns on `compose.vaapi.yaml` when `/dev/dri/renderD128` exists. By hand:
  add it to `COMPOSE_FILE` and set `PLEY_RENDER_GID` to
  `stat -c %g /dev/dri/renderD128`. With several GPUs, `PLEY_RENDER_DEVICE`
  picks one (`ls -l /dev/dri/by-path`). Intel drivers are in the amd64 image.
- **NVIDIA** (NVENC, NVDEC, Vulkan tone mapping): install the driver and the
  [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html),
  then turn on `compose.nvidia.yaml` (`setup.sh` does when Docker has the
  toolkit).

Then pick the encoder in **Settings → Server** (Automatic finds it), and
**Settings → Dashboard** shows each stream as GPU, hybrid or CPU.

## Security

The container starts locked down, and nothing below needs changing for pley
to work:

- **One port, on this computer only.** pley is published on `127.0.0.1`, so
  the reverse proxy is the only way in from other computers, with its TLS and
  access rules. The Plex-compatible API and network discovery are off, and
  the transcoder listens inside the container only, so nothing else can be
  reached.
- **Not root, no privileges.** pley runs as your user, with every Linux
  capability dropped and `no-new-privileges`; the image has no setuid
  programs and never runs `privileged`.
- **Read-only everywhere but its data.** The image is mounted read-only and
  `/tmp` can't hold programs (`noexec`); media is read-only. pley can only
  write its config folder, which `setup.sh` makes private (`chmod 700`): it
  holds the database and the key that encrypts two-step sign-in secrets.
- **Only the GPU, not the machine.** The Intel/AMD add-on hands over one
  render node; NVIDIA's goes through the toolkit.
- **Bounded.** Processes are capped (`pids_limit`) and Docker's log is rotated
  (3 × 10 MB), so a fault can't exhaust the host. On a machine that does
  other work, cap memory and CPU too in `compose.override.yaml`
  (`mem_limit: 8g`, `cpus: 6`; each transcode takes about 1 GB).
- **Nothing private in the image.** The build context leaves out `config/`,
  `media/` and `.env`.

On pley's side ([SECURITY.md](../../SECURITY.md) has the details):

- Set pley up before it can be reached from the internet: until then, the
  setup code in its log is all it takes to claim it.
- Turn on two-step sign-in for administrators (**Account → Two-step
  sign-in**).
- Leave apps by other publishers off (**Settings → Server → Apps**) unless you
  need one, and keep the Plex-compatible API off the internet.
- Keep pley up to date (below): `setup.sh` also fetches newer base images,
  which bring Debian's security fixes for FFmpeg and the rest of the image.

## Everyday use

From this folder:

| To | Run |
|---|---|
| See the log | `docker compose logs -f pley` |
| Update | `git pull && ./setup.sh` |
| Stop, start | `docker compose stop`, `docker compose start` |
| Remove the container (data stays) | `docker compose down` |
| Set a lost password | `docker compose exec pley pley reset-password <user> '<new password>'` |

Change a password set that way in the web UI afterwards: it went through your
shell's history.

**Backups:** pley keeps the last 3 daily copies of its database in
`config/backups`. For a full backup, stop pley and copy the whole config
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
- **Transcodes stay on the CPU:** `docker compose exec pley ls -l /dev/dri`
  should list `renderD128` (Intel/AMD); check `PLEY_RENDER_GID`. For NVIDIA,
  `docker compose exec pley nvidia-smi` should list the GPU.
- **Rootless Docker:** `PLEY_UID=0` and `PLEY_GID=0` (what `setup.sh` picks):
  root in a rootless container is your own user outside it.
