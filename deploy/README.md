# Running pley

Release archives for Linux, macOS and Windows (x86_64 and arm64) contain the
two programs, `pley` and `pley-transcoder`, which must stay in the same folder,
and the Linux and macOS ones the service files below. FFmpeg and ffprobe come
from the system (or `--ffmpeg` / `--ffprobe`); which FFmpeg decides what pley
can do with a GPU, so each section below names one.

By default pley listens on port 7539 on all interfaces and keeps its data in:

| OS | Data folder |
|---|---|
| Linux | `~/.local/share/pley` (`/var/lib/pley` with the systemd unit) |
| macOS | `~/Library/Application Support/pley` |
| Windows | `%LOCALAPPDATA%\pley` (`%ProgramData%\pley` for the service) |

Change it with `--data-dir` or `PLEY_DATA_DIR`. Every option is listed in
`pley --help` and [`pley.example.toml`](../pley.example.toml).

## Docker

`setup.sh` sets pley up with Docker Compose and starts it, locked down and
published on `127.0.0.1` for a reverse proxy:

```sh
cd deploy/docker && ./setup.sh
```

[docker/README.md](docker/README.md) is the whole guide (media folders,
reverse proxies, hardware transcoding, security, updates and backups).

The image is based on Debian 13 and its FFmpeg 7.1: NVIDIA (NVENC, CUDA
scaling), Intel (Quick Sync, VAAPI) and AMD (VAAPI) transcoding, and HDR tone
mapping with libplacebo (Vulkan), OpenCL or zscale. Releases publish it to
GitHub Container Registry for amd64 and arm64 (`ghcr.io/hhaidar/pley:latest`,
or a version such as `:0.2`). Without Compose:

```sh
docker build -f deploy/docker/Dockerfile -t pley .
mkdir -p config && sudo chown 1000:1000 config
docker run -d --name pley --restart unless-stopped --stop-timeout 30 \
  -p 127.0.0.1:7539:7539 --read-only --tmpfs /tmp \
  --cap-drop ALL --security-opt no-new-privileges \
  -v ./config:/config -v /srv/media:/media:ro pley
```

For an Intel or AMD GPU add
`--device /dev/dri/renderD128 --group-add "$(stat -c %g /dev/dri/renderD128)"`;
for NVIDIA, `--gpus all` (with the NVIDIA Container Toolkit).

The container runs as uid 1000 (`--user` changes it), so `/config` must be
writable by it. Setting pley up takes the code it prints at start:
`docker logs pley` shows a link that fills it in.
`docker exec -it pley pley reset-password <user>` recovers a lost
password (it asks for the new one).

## Linux (systemd)

From the extracted archive (`pley.service` is
[systemd/pley.service](systemd/pley.service) here):

```sh
sudo useradd --system --user-group --home-dir /var/lib/pley --shell /usr/sbin/nologin pley
sudo install -m755 pley pley-transcoder /usr/local/bin/
sudo cp pley.service /etc/systemd/system/
sudo systemctl enable --now pley
```

Give the `pley` user read access to your media. Logs:
`journalctl -u pley -f`.

**FFmpeg and hardware transcoding.** The distribution's FFmpeg works
(Debian 13's has NVENC, VAAPI, Quick Sync and libplacebo, as the Docker
image shows);
[jellyfin-ffmpeg](https://repo.jellyfin.org) is a good one elsewhere, set
with `Environment=PLEY_FFMPEG=...` and `PLEY_FFPROBE` in the unit, since it
isn't on `PATH`. Then:

- **Intel and AMD:** install the VAAPI driver (`intel-media-va-driver` or
  `mesa-va-drivers`) and Mesa's Vulkan driver (`mesa-vulkan-drivers`, for
  tone mapping), and uncomment `SupplementaryGroups=render video` in the
  unit. With several GPUs pley takes the first Intel or AMD one;
  `Environment=PLEY_VAAPI_DEVICE=/dev/dri/renderD129` picks another.
- **NVIDIA:** the proprietary driver is enough. If NVENC works by hand but
  not from the service, load `nvidia-uvm` at boot (the unit can't load
  kernel modules).

pley's log names the encoders it found as it starts (`journalctl -u pley |
grep detected`), and looks again when the GPUs change.

## macOS (launchd)

From the extracted archive (`pley.plist` is
[launchd/pley.plist](launchd/pley.plist) here):

```sh
brew install ffmpeg-full
sudo mkdir -p /usr/local/bin && sudo cp pley pley-transcoder /usr/local/bin/
cp pley.plist ~/Library/LaunchAgents/pley.server.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/pley.server.plist
```

It runs as your user at login. VideoToolbox hardware encoding is used
automatically. Homebrew's `ffmpeg-full`, which pley finds by itself, has the
tone mapping and subtitle burn-in that its `ffmpeg` leaves out. Downloaded
binaries may need `xattr -d com.apple.quarantine pley pley-transcoder` until
releases are signed.

## Windows

`pley-server-<version>-windows-<arch>-setup.exe` installs pley as a Windows
service, locked down to its own account and data folder, or in the
notification area, with FFmpeg, the firewall and an uninstaller:
[windows/README.md](windows/README.md) is the whole guide (accounts, NAS
sign-ins, silent installs, `pley service`).

Or install FFmpeg (for example `winget install Gyan.FFmpeg`), extract the
archive and run `pley.exe`. NVENC, QSV, AMF and Vulkan tone mapping work with
the gyan.dev "full" FFmpeg build. On Windows on Arm, FFmpeg runs emulated and
pley transcodes on the CPU.

## The tray icon

On a desktop (Windows, macOS, Linux with a StatusNotifier host such as KDE or
GNOME with the AppIndicator extension), pley shows an icon in the
notification area or menu bar while it runs: click it to open the web UI;
its menu scans the libraries, shows the logs, turns **Start pley at login** on
or off and quits pley. Started by double-click on Windows, pley closes its
console window once the icon is up; logs are in `logs/pley.log` in the data
folder. Services, containers and SSH sessions never show the icon, and
`--no-tray` (or `PLEY_NO_TRAY=true`) turns it off.

## Firewall and remote access

Port 7539 (TCP) is the only one that needs to be reachable: the
Plex-compatible API answers there too, unless you give it a port of its own
(`--plex-bind 0.0.0.0:32400`, or **Settings → Network**). The apps find
servers on the local network through UDP port 7539; allow it too, or enter
the server's address in the app. Containers only answer them with host
networking (`--network host`), since broadcasts don't reach a bridge
network. For access outside your network,
put pley behind a reverse proxy with TLS (Caddy, nginx, Traefik); it serves
plain HTTP and works under any host name. Docker publishes ports around host
firewalls: behind a proxy on the same host, publish pley on `127.0.0.1` only.

## Apps and reverse proxies

Apps sign in on pley's own page (OAuth 2.1), and pley names itself by the
address each request came to. A reverse proxy must therefore pass the
original `Host` header and set `X-Forwarded-Proto`, overwriting what
clients send rather than adding to it (RFC 9700 §4.13). Caddy does both by
default. With nginx (a whole server block is in
[docker/README.md](docker/README.md#behind-a-reverse-proxy)):

```nginx
client_max_body_size 25m;
proxy_set_header Host $http_host;
proxy_set_header X-Forwarded-Proto $scheme;
proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
proxy_buffering off;
```

Uploads (artwork) are up to 25 MB, and live updates and videos stream, so
the proxy must not buffer them.

`$http_host` is the address as the browser or app sent it, with its port;
nginx's `$host` leaves the port out. A proxy that does that (Nginx Proxy
Manager does by default) works when the address is reached at its scheme's
own port (`https://media.example.com`). At another
(`https://media.example.com:9999`), pley would name itself without the
port, and apps would refuse it: add the address in **Settings → Network →
Reached through a reverse proxy** (the page offers it when opened there),
and pley puts the port back.

TVs signing in with a code are limited per address. Behind a proxy on the
same computer or network, pley counts the address the proxy adds last to
`X-Forwarded-For` (Caddy and the line above add it); without it, every TV
counts as the proxy.

Otherwise apps refuse the server: its sign-in description
(`/.well-known/oauth-authorization-server`) names another address than the
one they used (RFC 8414 §3.3).

pley's own apps can always sign in. Apps by other publishers are off until
an administrator turns them on in **Settings → Server → Apps**; pley then
reads their descriptions from the internet when someone allows one.

## Upgrading from single sign-on

pley no longer signs people in through an OpenID Connect provider: if you
set one up, delete pley's client at the provider. Upgrading removes the
single sign-on settings and the identities linked to accounts, but the copy
of the database made before the upgrade (`backups/pley-…-before-upgrade.db`
in the data folder) and older backups still hold the old client secret.

Accounts that signed in only through the provider stay signed in where they
are, but have no password to sign in again: an administrator gives them one
in **Settings → Users**, or `pley reset-password <user>` does.
