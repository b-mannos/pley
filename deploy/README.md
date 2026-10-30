# Running Pley

Release archives for Linux, macOS and Windows (x86_64 and arm64) contain the
two programs, `pley` and `pley-transcoder`, which must stay in the same
folder; the Linux and macOS archives also hold the service files below.
FFmpeg and ffprobe come from the system, or from `--ffmpeg` and `--ffprobe`.
The FFmpeg build decides what Pley can do with a GPU, so each section below
names one.

By default Pley listens on port 7539 on all interfaces and keeps its data in:

| OS | Data folder |
|---|---|
| Linux | `~/.local/share/pley` (`/var/lib/pley` with the systemd unit) |
| macOS | `~/Library/Application Support/pley` |
| Windows | `%LOCALAPPDATA%\pley` (`%ProgramData%\pley` for the service) |

Change it with `--data-dir` or `PLEY_DATA_DIR`. `pley --help` and
[`pley.example.toml`](../pley.example.toml) list every option.

## Docker

`setup.sh` sets Pley up with Docker Compose and starts it, locked down and
published on `127.0.0.1` for a reverse proxy:

```sh
cd deploy/docker && ./setup.sh
```

[docker/README.md](docker/README.md) is the full guide: media folders,
reverse proxies, hardware transcoding, security, updates and backups.

The image is based on Debian 13 and its FFmpeg 7.1:

- NVIDIA (NVENC, CUDA scaling), Intel (Quick Sync, VAAPI) and AMD (VAAPI)
  transcoding;
- HDR tone mapping with libplacebo (Vulkan), OpenCL or zscale;
- Pley's licence in `/usr/share/doc/pley`, and each Debian package's,
  FFmpeg's among them, in `/usr/share/doc/<package>/copyright`.

Releases publish it to GitHub Container Registry for amd64 and arm64:
`ghcr.io/hhaidar/pley:latest`, or a server version such as `:0.0.1` or
`:0.0`.

Without Compose:

```sh
docker build -f deploy/docker/Dockerfile -t pley .
mkdir -p config && sudo chown 1000:1000 config
docker run -d --name pley --restart unless-stopped --stop-timeout 30 \
  -p 127.0.0.1:7539:7539 --read-only --tmpfs /tmp \
  --cap-drop ALL --security-opt no-new-privileges \
  -v ./config:/config -v /srv/media:/media:ro pley
```

- Intel or AMD GPU: add
  `--device /dev/dri/renderD128 --group-add "$(stat -c %g /dev/dri/renderD128)"`.
- NVIDIA: add `--gpus all` (needs the NVIDIA Container Toolkit).
- The container runs as uid 1000 (`--user` changes it), so `/config` must be
  writable by that user.
- `docker logs pley` shows the setup link with its one-time code.
- `docker exec -it pley pley reset-password <user>` sets a lost password
  (it asks for the new one).

## Linux (systemd)

From the extracted archive (`pley.service` is
[systemd/pley.service](systemd/pley.service)):

```sh
sudo useradd --system --user-group --home-dir /var/lib/pley --shell /usr/sbin/nologin pley
sudo install -m755 pley pley-transcoder /usr/local/bin/
sudo cp pley.service /etc/systemd/system/
sudo systemctl enable --now pley
```

Give the `pley` user read access to your media. Logs:
`journalctl -u pley -f`.

**FFmpeg and hardware transcoding.** The distribution's FFmpeg works (Debian
13's has NVENC, VAAPI, Quick Sync and libplacebo). Elsewhere,
[jellyfin-ffmpeg](https://repo.jellyfin.org) is a good choice; it isn't on
`PATH`, so set `Environment=PLEY_FFMPEG=...` and `PLEY_FFPROBE` in the unit.
Then:

- **Intel and AMD:** install the VAAPI driver (`intel-media-va-driver` or
  `mesa-va-drivers`) and Mesa's Vulkan driver (`mesa-vulkan-drivers`, for
  tone mapping), and uncomment `SupplementaryGroups=render video` in the
  unit. With several GPUs Pley takes the first Intel or AMD one;
  `Environment=PLEY_VAAPI_DEVICE=/dev/dri/renderD129` picks another.
- **NVIDIA:** the proprietary driver is enough. If NVENC works by hand but
  not from the service, load `nvidia-uvm` at boot (the unit can't load
  kernel modules).

Pley logs the encoders it found at start (`journalctl -u pley | grep
detected`), and checks again when the GPUs change.

## macOS (launchd)

From the extracted archive (`pley.plist` is
[launchd/pley.plist](launchd/pley.plist)):

```sh
brew install ffmpeg-full
sudo mkdir -p /usr/local/bin && sudo cp pley pley-transcoder /usr/local/bin/
cp pley.plist ~/Library/LaunchAgents/pley.server.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/pley.server.plist
```

- It runs as your user at login.
- VideoToolbox hardware encoding is used automatically.
- Pley finds Homebrew's `ffmpeg-full` by itself; it has the tone mapping and
  subtitle burn-in that Homebrew's `ffmpeg` leaves out.
- Until releases are signed, downloaded binaries may need
  `xattr -d com.apple.quarantine pley pley-transcoder`.

## Windows

`pley-server-<version>-windows-<arch>-setup.exe` installs Pley as a locked-down
Windows service or in the notification area, with FFmpeg, the firewall rule
and an uninstaller. [windows/README.md](windows/README.md) is the full guide
(accounts, NAS sign-ins, silent installs, `pley service`).

Or install FFmpeg (for example `winget install Gyan.FFmpeg`), extract the
archive and run `pley.exe`. NVENC, QSV, AMF and Vulkan tone mapping work with
the gyan.dev "full" FFmpeg build. On Windows on Arm, FFmpeg runs emulated and
Pley transcodes on the CPU.

## The tray icon

On a desktop (Windows, macOS, or Linux with a StatusNotifier host such as KDE,
or GNOME with the AppIndicator extension), Pley shows an icon in the
notification area or menu bar while it runs:

- Click it to open the web app.
- Its menu scans the libraries, shows the logs, turns **Start Pley at login**
  on or off, and quits Pley.
- Started by double-click on Windows, Pley closes its console window once the
  icon is up; logs are in `logs/pley.log` in the data folder.

Services, containers and SSH sessions never show the icon. `--no-tray` (or
`PLEY_NO_TRAY=true`) turns it off.

## Firewall and remote access

- **TCP 7539** is the only port that must be reachable. The compatibility API
  answers there too, unless you give it a port of its own
  (`--plex-bind 0.0.0.0:32400`, or **Settings → Network**).
- **UDP 7539** lets the apps find servers on the local network. Allow it, or
  enter the server's address in the app. Containers answer only with host
  networking (`--network host`), since broadcasts don't reach a bridge
  network.
- **Outside your network**, put Pley behind a reverse proxy with TLS (Caddy,
  nginx, Traefik). Pley serves plain HTTP and works under any host name.
- Docker publishes ports around host firewalls: with a proxy on the same
  host, publish Pley on `127.0.0.1` only.

## Apps and reverse proxies

Apps sign in on Pley's own page (OAuth 2.1), and Pley names itself after the
address each request came to. If that name differs from the address an app
used, the app refuses the server: the sign-in description
(`/.well-known/oauth-authorization-server`) must name the same address
(RFC 8414 §3.3).

So a reverse proxy must:

- pass the original `Host` header and set `X-Forwarded-Proto`, replacing what
  clients send rather than adding to it (RFC 9700 §4.13). Pley also marks its
  cookies `Secure` behind HTTPS;
- add the client's address to `X-Forwarded-For`. Pley trusts it only from a
  proxy on the same computer, the local network or a Docker network, and
  limits TVs signing in with a code by the address the proxy added last.
  Without it, every TV counts as the proxy;
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
        proxy_set_header Host $http_host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_buffering off;
    }
}
```

`$http_host` is the address as the client sent it, port included; nginx's
`$host` leaves the port out. A proxy that drops the port (Nginx Proxy Manager
does by default) works when the address uses its scheme's default port
(`https://media.example.com`). On another port
(`https://media.example.com:9999`), add the address in **Settings → Network →
Reached through a reverse proxy** (the page offers it when opened through
the proxy), and Pley puts the port back.

Pley's own apps can always sign in. Apps by other publishers are off until
an administrator turns them on in **Settings → Server → Apps**; Pley then
fetches an app's description from the internet when someone allows it.

For Docker-specific setups (a proxy in Docker, Traefik labels, a proxy on
another computer), see [docker/README.md](docker/README.md#behind-a-reverse-proxy).
