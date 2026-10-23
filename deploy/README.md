# Running pley

Release archives for Linux, macOS and Windows (x86_64 and arm64) contain the
two programs, `pley` and `pley-transcoder`, which must stay in the same folder.
FFmpeg and ffprobe come from the system (or `--ffmpeg` / `--ffprobe`).

By default pley listens on port 7539 on all interfaces and keeps its data in:

| OS | Data folder |
|---|---|
| Linux | `~/.local/share/pley` (`/var/lib/pley` with the systemd unit) |
| macOS | `~/Library/Application Support/pley` |
| Windows | `%LOCALAPPDATA%\pley` |

Change it with `--data-dir` or `PLEY_DATA_DIR`. Every option is listed in
`pley --help` and [`pley.example.toml`](../pley.example.toml).

## Docker

Releases publish the image to GitHub Container Registry for amd64 and
arm64 (`ghcr.io/<owner>/<repo>:latest`, or a version such as `:0.2`). To build
it yourself instead:

```sh
docker build -f deploy/docker/Dockerfile -t pley .
docker run -d --name pley -p 7539:7539 \
  -v ./config:/config -v /srv/media:/media:ro pley
```

or `docker compose -f deploy/docker/compose.yaml up -d` (edit the media path
first). The image is based on Debian 13 with its FFmpeg 7, which supports
NVENC, VAAPI, QSV and Vulkan (libplacebo) tone mapping:

- **Intel / AMD:** pass `/dev/dri` and add the host's `render` group id with
  `--group-add` (see the comments in `compose.yaml`).
- **NVIDIA:** install the NVIDIA Container Toolkit and run with `--gpus all`.
  The image already asks for the `video` driver capability that NVENC needs.

The container runs as uid 1000, with no extra privileges; the `/config`
folder must be writable by it. Setting pley up takes the code it prints at
start: `docker logs pley` shows a link that fills it in.
`docker exec pley pley reset-password <user> <password>` recovers a lost
password.

## Linux (systemd)

```sh
sudo useradd --system --home-dir /var/lib/pley --shell /usr/sbin/nologin pley
sudo install -m755 pley pley-transcoder /usr/local/bin/
sudo cp deploy/systemd/pley.service /etc/systemd/system/
sudo systemctl enable --now pley
```

Give the `pley` user read access to your media. For hardware transcoding add
it to the `render` group (and uncomment `SupplementaryGroups` in the unit).
Logs: `journalctl -u pley -f`.

## macOS (launchd)

```sh
brew install ffmpeg
sudo cp pley pley-transcoder /usr/local/bin/
cp deploy/launchd/pley.plist ~/Library/LaunchAgents/pley.server.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/pley.server.plist
```

It runs as your user at login. VideoToolbox hardware encoding is used
automatically. Downloaded binaries may need
`xattr -d com.apple.quarantine pley pley-transcoder` until releases are signed.

## Windows

Install FFmpeg (for example `winget install Gyan.FFmpeg`), extract the archive
and run `pley.exe`. NVENC, QSV, AMF and Vulkan tone mapping work with the
gyan.dev "full" FFmpeg build.

## The tray icon

On a desktop (Windows, macOS, Linux with a StatusNotifier host such as KDE or
GNOME with the AppIndicator extension), pley shows an icon in the
notification area or menu bar while it runs: click it to open the web UI;
its menu scans the libraries, shows the logs, turns **Start pley at login** on
or off and quits pley. Started by double-click on Windows, pley closes its
console window once the icon is up; logs are in `logs/pley.log` in the data
folder. Services, containers and SSH sessions never show the icon, and
`--no-tray` (or `PLEY_NO_TRAY=1`) turns it off.

## Firewall and remote access

Port 7539 (TCP) is the only one that needs to be reachable. The apps find
servers on the local network through UDP port 7539; allow it too, or enter
the server's address in the app. Containers only answer them with host
networking (`--network host`), since broadcasts don't reach a bridge
network. For access outside your network,
put pley behind a reverse proxy with TLS (Caddy, nginx, Traefik); it serves
plain HTTP and works under any host name. Docker publishes ports around host
firewalls: behind a proxy on the same host, publish pley on `127.0.0.1` only.

## Single sign-on

pley signs people in with an OpenID Connect provider. In **Settings →
Server → Single sign-on**:

1. Enter pley's **public address** (`https://pley.example.com`): sign-ins
   happen there, and the provider sends people back to it.
2. Enter the provider's **issuer** exactly as it shows it, and test it.
3. Either register pley with the provider as a client (redirect URI
   `https://pley.example.com/api/auth/sso/callback`, shown in the settings)
   and enter its client ID and secret, or leave the client ID empty if the
   provider takes client metadata documents: it then reads pley's at
   `https://pley.example.com/api/auth/sso/client`.
4. Turn it on. People link their account from their account page (with
   their password) and then sign in with the provider.

Optionally, let only members of one group in (the provider must send a
`groups` claim), or give accounts to people the provider knows but pley
doesn't: they start without libraries. pley asks the provider only who
signed in and keeps none of its tokens.
