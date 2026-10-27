# pley-server on Windows

`pley-server-<version>-windows-<arch>-setup.exe`, on each release, installs
the server as a Windows service or in the notification area, with FFmpeg,
its data folder, the firewall and an uninstaller. It needs Windows 10 1809
or newer and an administrator.

The release archives (`pley-<version>-windows-<arch>.zip`) still run as they
are: [../README.md](../README.md#windows).

## Installing

Run the setup. Its pages:

1. **Where pley goes**: `C:\Program Files\pley-server`.
2. **How pley runs**
   - **As a service with an account of its own** (recommended): it starts
     with Windows, before anyone signs in, as `NT SERVICE\pley`, an account
     with no password that can't sign in. For media on this computer.
   - **As a service, as a user of its own**: for media on a NAS or another
     computer, which the first account reaches as the computer, not as a
     user. Setup creates a standard user (`pley`) with a random password
     nobody needs, and can save its sign-in to your NAS: the NAS, and a user
     and password that may read your media there. In pley, add libraries by
     their network paths, such as `\\nas\media\Movies`.
   - **In the notification area, as you**: it runs while you are signed in,
     with your account, network drives included, and starts when you sign in.
   - **Let the service write only in its data folder** (on by default):
     Windows checks every change the service makes against its own SID,
     which only the data folder allows, and takes away the privileges it
     doesn't need. If no GPU encoder is found (the last page says), run
     Setup again and turn this off.
3. **Data folder**: `C:\ProgramData\pley` for a service, `%LOCALAPPDATA%\pley`
   in the notification area. It can start with the data of the pley you used
   until now (its accounts, libraries, watch history and artwork): that pley
   must be stopped, and nothing is copied over existing data. When the data
   folder changes from one install to the next, Setup offers to copy it.
4. **Network**
   - **Only this computer**: pley listens on `127.0.0.1`, for a reverse proxy
     on this computer that serves it with HTTPS (Caddy, nginx, IIS).
   - **Devices on my network too**: pley listens on every network, and
     Setup lets it through Windows Firewall on networks Windows treats as
     private.
   - The port (7539), the Plex-compatible API (port 32400, for Plex apps) and
     whether the pley apps find the server on the local network.
   Setup says when another program has the port.
5. **FFmpeg**: the one the installer carries, when it was built with one
   (see Building the installer), copied next to pley from a folder (a
   build's `bin` folder, with its DLLs: gyan.dev's, or one built for Dolby
   Vision FEL), downloaded (gyan.dev's full build, checked against its
   SHA-256; x64 only), or the one on PATH. A service sees only the system's
   PATH.

The last page gives pley's address and the link that creates the
administrator account, and says which encoders pley found in FFmpeg.

## What Setup sets up

| | |
|---|---|
| Programs | `pley.exe`, `pley-transcoder.exe` and FFmpeg (`ffmpeg\`) in the install folder |
| Options | `pley.toml` in the data folder: the address, the Plex-compatible API, discovery and FFmpeg |
| The service | `pley` ("pley-server" in Services): starts with Windows (delayed), restarts when it stops on its own, has 30 s to stop as Windows shuts down, and keeps its temporary files in the data folder's `tmp` |
| The data folder | Owned by the administrators; only SYSTEM, the administrators, the service (and its user) and you may open it |
| The firewall | A rule named `pley` for its port, on private networks, when it listens on the network |
| The notification area | pley starts when you sign in (Start pley at login, in its menu) |
| Start menu | `pley-server` (pley's page, or pley itself) and `pley-server logs` |

## Updating

Run the new version's setup. It remembers the last install's choices, stops
pley, replaces it and starts it again.

## Uninstalling

**Settings → Apps → pley-server → Uninstall**. The uninstaller removes the
service, the user Setup made, the firewall rule, the start at sign-in, the
programs and the FFmpeg copy, and asks whether to delete the data folder
too (no, unless you say so).

## Installing without the wizard

```
pley-server-0.2.0-windows-x86_64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /MODE=service /FFMPEG=download
```

| Option | Values | Default |
|---|---|---|
| `/MODE=` | `service`, `user` (a user of its own) or `tray` | `service` |
| `/USER=` | The user's name, with `user` | `pley` |
| `/SHARE=`, `/SHAREUSER=`, `/SHAREPASSWORD=` | The NAS and its sign-in, with `user` | none |
| `/RESTRICT=` | `yes` or `no`: writes only in its data folder | `yes` |
| `/AUTOSTART=` | `yes` or `no`: start at sign-in, with `tray` | `yes` |
| `/DATADIR=` | The data folder | as above |
| `/COPYDATA=` | A data folder to copy first | none |
| `/LAN=` | `yes` or `no`: listen on the network | `no` |
| `/PORT=` | The port | `7539` |
| `/PLEX=`, `/DISCOVERY=` | `yes` or `no` (discovery with `/LAN=yes`) | `no`, `yes` |
| `/FFMPEG=` | `bundled`, `keep`, `download`, `path` or a folder | the installer's own, or the FFmpeg on PATH, copied; downloaded without one |

Setup's own options work too, such as `/DIR=` (where pley goes) and
`/LOG=<file>`. An update keeps the last choices for the options not given. A
silent install stops when the port is taken. A silent uninstall
(`unins000.exe /VERYSILENT`) keeps the data folder unless `/DELETEDATA`.

## By hand: `pley service`

What Setup runs, from an administrator's terminal:

```
pley --data-dir C:\ProgramData\pley --bind 127.0.0.1:7539 --plex-bind off ^
  --ffmpeg "C:\Program Files\pley-server\ffmpeg\ffmpeg.exe" ^
  --ffprobe "C:\Program Files\pley-server\ffmpeg\ffprobe.exe" service install
```

saves the options in `pley.toml`, installs or updates the service, locks the
data folder down, starts pley and prints its address and setup link.
`pley service install --help` lists the rest (`--account user`,
`--unrestricted`, `--grant`, `--share`, `--copy-data-from`, `--dry-run`…);
`pley service start`, `stop` and `uninstall` do what they say. `pley
save-config` saves the same options for pley outside a service.

## Security

- **Accounts.** `NT SERVICE\pley` has no password and can't sign in. The
  user of its own is a standard user whose random password is set again by
  each install; it may run services, and nobody needs to sign in as it. Its
  NAS sign-in is in its own Windows Credential Manager. Setup only ever
  changes or deletes a user it made.
- **Writes and privileges.** Unless turned off, Windows lets the service
  change only what allows its SID, which is the data folder, and keeps only
  two of its account's privileges (going through folders, and the shared
  objects GPU drivers make). Impersonation, which a compromised service
  could use to become SYSTEM, is gone.
- **The data folder** (its database, the key that seals two-step secrets,
  its logs) is owned by the administrators and closed to other users.
  Setup refuses a data folder that is, or holds, a link or junction: one
  made in ProgramData before pley was installed could otherwise send its
  changes anywhere.
- **The network.** pley listens on `127.0.0.1` unless told otherwise, and
  the firewall rule, for the network choice only, covers private networks.
- **Setup isn't signed yet**: SmartScreen warns (**More info → Run anyway**).
  Each release's installer has build provenance:
  `gh attestation verify <file> --repo <owner>/<repo>`.
- **RedirectionGuard is off** for Setup: the pley it starts would otherwise
  inherit it, and couldn't follow the links WinGet puts FFmpeg behind, nor
  junctions in media folders, until it restarted. Setup itself writes only
  in Program Files and its own temporary folder.

## When something is wrong

- **pley's log** is `logs\pley.log` in the data folder (Start menu:
  `pley-server logs`). The service is `pley-server` in Services
  (`services.msc`).
- **No GPU encoder** although the computer has one: run Setup again and turn
  off **Let the service write only in its data folder**. It is on by
  default, but no GPU has been tried with it yet.
- **A NAS isn't reached**: choose **As a service, as a user of its own**,
  give the NAS by the name the library paths use (`\\nas`, not its address,
  or the other way round), and a user the NAS knows.
- **The port is taken**: another pley, or another program, listens there.
  Stop it or choose another port.

## Building the installer

With [Inno Setup](https://jrsoftware.org/isinfo.php) 6.7 or newer
(`winget install JRSoftware.InnoSetup`):

```
powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Archive -Installer
```

or, around a release folder already built,
`scripts\build-installer.ps1 -Release dist\pley-<version>-windows-x86_64`.
`-Ffmpeg <an FFmpeg build's bin folder>` makes the installer carry that
FFmpeg (and the DLLs next to it) and offer it first, for computers that
don't have it, and `-Flavor fel` adds a name to the file:
`pley-server-<version>-fel-windows-x86_64-setup.exe`.
`ISCC /DTest …` builds one that installs in the notification area without
administrator rights (no shortcuts nor start at sign-in), to try Setup's
code on a development machine.
