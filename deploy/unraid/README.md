# Pley on Unraid

[`pley.xml`](pley.xml) is an Unraid Docker template for Pley's image
(`ghcr.io/hhaidar/pley`, [deploy/docker](../docker/README.md)): the web UI on
port 7539, its data in appdata, your media read-only, and the GPU for
hardware transcoding when you give it one.

## Add it

In Unraid's terminal (the `>_` icon), put the template where Unraid keeps
yours:

```sh
wget -O /boot/config/plugins/dockerMan/templates-user/my-Pley.xml \
  https://raw.githubusercontent.com/hhaidar/pley/master/deploy/unraid/pley.xml
```

Then **Docker → Add Container → Template: Pley**, check the paths and
**Apply**. Open the web UI once it has started: the first visit sets up the
administrator.

| Field | Default | |
|---|---|---|
| Web UI | `7539` | The web app, the apps' API and the Plex-compatible API |
| Config | `/mnt/user/appdata/pley` | Database, artwork, caches, transcodes: the only folder Pley writes. Put appdata on a cache pool |
| Media | `/mnt/user/media` | Your media, read-only; in Pley, libraries are folders under `/media` |
| Intel or AMD GPU | empty | `/dev/dri` for hardware transcoding (below) |
| NVIDIA GPU | empty | The GPU's UUID, for hardware transcoding (below) |
| PUID, PGID | `99`, `100` | The user and group Pley runs as: Unraid's `nobody:users`, who own appdata and read the shares |

The container starts as root, as Unraid starts every container, and Pley
runs as PUID:PGID with no capabilities: on the first start it gives the
config folder (which Docker created as root's) to that user, and it joins the
groups that may open the GPU you pass in. The image's
[`pley.sh`](../docker/pley.sh) does this; its log says what it changed.

To reach Pley from outside your network, put it behind a reverse proxy (Nginx
Proxy Manager, SWAG) and set its public address in **Settings → Network**.

## Hardware transcoding

Pley picks what the GPU can do on its own (**Settings → Transcoder** shows
what it found): H.264 and HEVC encoding and decoding, scaling and HDR tone
mapping on the GPU.

### Intel (Quick Sync) and AMD

1. Load the GPU's driver: the **Intel GPU TOP** or **Radeon TOP** plugin
   (Apps) loads it at boot. `ls /dev/dri` in the terminal then lists
   `card0` and `renderD128`.
2. In the template, set **Intel or AMD GPU** to `/dev/dri`.

If the log says only root may open the GPU, run `chmod 666
/dev/dri/renderD128` (and add it to `/boot/config/go` to keep it after a
reboot): Pley never joins root's group to reach it.

### NVIDIA (NVENC)

1. Install the **Nvidia Driver** plugin (Apps) and reboot. Its page shows
   the GPU's UUID (`GPU-…`).
2. In the template, set **NVIDIA GPU** to that UUID (or `all`), and add
   `--runtime=nvidia` to **Extra Parameters** (Advanced View).

The image already asks for the driver's video, compute and graphics parts
(`NVIDIA_DRIVER_CAPABILITIES`): graphics is Vulkan, for HDR tone mapping.
