# Pley Media Server on Windows

`pley-server-<version>-windows-<arch>-setup.exe`, published with each
release, installs the server as a Windows service or in the notification
area, with FFmpeg, its data folder, a firewall rule and an uninstaller. It
needs Windows 10 1809 or newer and an administrator.

The release archives (`pley-server-<version>-windows-<arch>.zip`) also run
without installing: see [Running Pley](../README.md#windows).

## Installing

Run the setup. Its pages:

1. **Where Pley goes**: `C:\Program Files\pley-server`.
2. **How Pley runs**
   - **As a service with an account of its own** (recommended), for media on
     this computer. It starts with Windows, before anyone signs in, as
     `NT SERVICE\pley`: an account with no password that can't sign in.
   - **As a service, as a user of its own**, for media on a NAS or another
     computer (the first option reaches them as the computer, not as a
     user). Setup creates a standard user (`pley`) with a random password
     nobody needs, and can save its sign-in to your NAS: the NAS name, and a
     user and password that can read your media there. In Pley, add
     libraries by their network paths, such as `\\nas\media\Movies`.
   - **In the notification area, as you**: runs while you are signed in, with
     your account (network drives included), and starts when you sign in.
     Opening it again from the Start menu (**Pley Media Server**) opens its
     page.
   - **Let the service write only in its data folder** (on by default):
     Windows checks every change the service makes against its own SID,
     which only the data folder allows, and removes privileges it doesn't
     need. If no GPU encoder is found (the last page says), run Setup again
     and turn this off.
3. **Data folder**: `C:\ProgramData\pley` for a service, `%LOCALAPPDATA%\pley`
   in the notification area. It can start from the data of a Pley you
   already use (accounts, libraries, watch history, artwork); that Pley must
   be stopped, and nothing is copied over existing data. When the data
   folder changes between installs, Setup offers to copy it.
4. **Network**: where Pley listens.
   - **Only this computer** (the default): `127.0.0.1`, for a reverse proxy
     on this computer that adds HTTPS (Caddy, nginx, IIS).
   - **This computer and these networks**: lists the network adapters with
     their addresses and the kind of network Windows considers each. Pley
     listens on those ticked (and `127.0.0.1`). An adapter that isn't up yet
     when Pley starts (Wi-Fi, a VPN such as Tailscale) is retried every
     30 s.
   - **Every network**, including those added later.
   - The port (7539), which no other program may use (Setup says when one
     does), and the compatibility API, on by default on Pley's port.
   - All of this can be changed later while Pley runs, in
     **Settings → Network**.
5. **Windows Firewall**: which kinds of network to let Pley through on
   (private and domain by default, not public), from devices on those
   networks only or from any address (a VPN too), and whether Pley's apps
   on the network can find the server. The rule follows Pley wherever it
   listens, so a network chosen later in its settings is let through if it
   is of a kind chosen here.

**FFmpeg** comes with the installer and goes next to Pley; each install
replaces it. Releases carry gyan.dev's full build (NVENC, Quick Sync, AMF and
Vulkan tone mapping); an installer built with another FFmpeg carries that one
(see [Building the installer](#building-the-installer)). The ARM64 installer
carries the same x64 build, which Windows runs emulated, so on Windows on Arm
Pley transcodes on the CPU.

**The last page** gives Pley's address and the link that creates the
administrator account (or, with existing data, says to sign in as before),
and lists the encoders Pley found in FFmpeg. A service has no icon in the
notification area: the Start menu's **Pley Media Server** opens it.

## What Setup sets up

| | |
|---|---|
| Programs | `pley.exe`, `pley-transcoder.exe` and FFmpeg (`ffmpeg\`) in the install folder |
| Options | `pley.toml` in the data folder: the address, the compatibility API, discovery and FFmpeg |
| The service | `pley` ("Pley Media Server" in Services): starts with Windows (delayed), restarts when it stops on its own, has 30 s to stop at shutdown, and keeps its temporary files in the data folder's `tmp` |
| The data folder | Owned by the administrators; only SYSTEM, the administrators, the service (and its user) and you can open it |
| The firewall | A rule named `pley`, for the kinds of network and devices chosen |
| The notification area | Pley starts when you sign in (**Start Pley at login** in its menu) |
| Start menu | **Pley Media Server** (Pley's page, or Pley itself) and **Pley Media Server logs** |

## Media folders

Pley only reads media, as its account: a service as `NT SERVICE\pley` (or its
own user), in the notification area as you. A folder on another drive
(`D:\Movies`) usually lets every user read it, services included; one in your
user folder (`C:\Users\<you>\Videos`) doesn't. To let the service read one,
from an administrator's terminal:

```
icacls "C:\Users\<you>\Videos" /grant "NT SERVICE\pley:(OI)(CI)RX"
```

`(OI)(CI)` applies it to the files and folders inside, and `RX` is read only.
For the service's own user, use its name instead (`pley` unless you chose
another).

Or in Explorer: the folder's **Properties → Security → Edit → Add**,
**Locations**: this computer, `NT SERVICE\pley`, **Check Names**, **OK**, and
**Read & execute**. It applies at once: scan the library again.

- **Let the service write only in its data folder** doesn't limit what it
  reads.
- A drive letter mapped to a NAS exists only for you: add libraries by their
  network paths (`\\nas\media\Movies`), with **As a service, as a user of its
  own**.

## Updating

Run the new version's setup. It reuses the last install's choices, stops
Pley, replaces it and its FFmpeg, and starts it again.

## Uninstalling

**Settings → Apps → Pley Media Server → Uninstall**. The uninstaller removes
the service, the user Setup made, the firewall rule, the start at sign-in,
the programs and FFmpeg. It asks whether to delete the data folder too (no
by default); a data folder holding files other than Pley's stays. (Setup
accepts only an empty or new folder for Pley's data, or one of Pley's.)

## Installing without the wizard

```
pley-server-0.0.1-windows-x86_64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /MODE=service /LISTEN=192.168.1.10
```

| Option | Values | Default |
|---|---|---|
| `/MODE=` | `service`, `user` (a user of its own) or `tray` | `service` |
| `/USER=` | The user's name, with `user` | `pley` |
| `/SHARE=`, `/SHAREUSER=` | The NAS and its user name, with `user` (password: below) | none |
| `/RESTRICT=` | `yes` or `no`: write only in its data folder | `yes` |
| `/AUTOSTART=` | `yes` or `no`: start at sign-in, with `tray` | `yes` |
| `/DATADIR=` | The data folder | as above |
| `/COPYDATA=` | A data folder to copy first | none |
| `/LISTEN=` | `local`, `all`, or the addresses of the networks to listen on, separated by commas | `local` |
| `/PORT=` | The port | `7539` |
| `/PLEX=` | `yes` or `no`: the compatibility API, on Pley's port | `yes` |
| `/FIREWALL=` | The kinds of network to let Pley through on: `private`, `domain`, `public` (separated by commas), or `none` | `private,domain` |
| `/FROM=` | `subnet` (devices on those networks) or `any` (a VPN too) | `subnet` |
| `/DISCOVERY=` | `yes` or `no`: Pley's apps can find the server, where others may reach it | `yes` |

The NAS password comes from the environment variable `PLEY_SHARE_PASSWORD`,
never the command line, which Setup writes to its log. Set it in an
administrator's PowerShell and run Setup from there (a Setup that asks for
administrator rights itself starts without it):

```
$env:PLEY_SHARE_PASSWORD = [Net.NetworkCredential]::new('', (Read-Host 'NAS password' -AsSecureString)).Password
.\pley-server-0.0.1-windows-x86_64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /MODE=user /SHARE=\\nas /SHAREUSER=media
```

- Inno Setup's own options work too, such as `/DIR=` (where Pley goes) and
  `/LOG=<file>`.
- An update keeps the last choices for options not given.
- A silent install stops when the port is taken.
- A silent uninstall (`unins000.exe /VERYSILENT`) keeps the data folder
  unless given `/DELETEDATA`.

## By hand: `pley service`

What Setup runs, from an administrator's terminal:

```
pley --data-dir C:\ProgramData\pley --bind 127.0.0.1:7539 --plex-bind web ^
  --ffmpeg "C:\Program Files\pley-server\ffmpeg\ffmpeg.exe" ^
  --ffprobe "C:\Program Files\pley-server\ffmpeg\ffprobe.exe" service install
```

It saves the options in `pley.toml`, installs or updates the service, locks
the data folder down, starts Pley and prints its address and setup link.

- `pley service install --help` lists the other options (`--account user`,
  `--unrestricted`, `--grant`, `--share`, `--copy-data-from`, `--dry-run`, …).
- `pley service start`, `stop` and `uninstall` do what they say.
- `pley save-config` saves the same options for a Pley that isn't a service.

## Security

- **Accounts.** `NT SERVICE\pley` has no password and can't sign in. The
  service's own user is a standard user whose random password is reset by
  each install; it may run services, and nobody needs to sign in as it. Its
  NAS sign-in is kept in its own Windows Credential Manager. Setup only
  changes or deletes a user it created.
- **Writes and privileges.** Unless turned off, Windows lets the service
  change only what allows its SID (the data folder), and keeps only two of
  its account's privileges: traversing folders, and the shared objects GPU
  drivers create. Impersonation, which a compromised service could use to
  become SYSTEM, is removed.
- **The data folder** (database, the key that seals two-step sign-in
  secrets, logs) is owned by the administrators and closed to other users.
  Setup refuses a data folder that is, or contains, a link or junction: one
  planted in ProgramData before Pley was installed could redirect its
  writes.
- **The network.** Pley listens on `127.0.0.1` unless told otherwise (in
  Setup, or by an administrator in **Settings → Network**). The firewall
  lets it through on private and domain networks only, unless you add
  public ones.
- **Setup isn't signed**: SmartScreen warns (**More info → Run anyway**).
  Each release's installer has build provenance:
  `gh attestation verify <file> --repo <owner>/<repo>`.
- **RedirectionGuard is off** for Setup. Otherwise the Pley it starts would
  inherit it and, until restarted, couldn't follow the links WinGet puts
  FFmpeg behind, nor junctions in media folders. Setup itself writes only in
  Program Files and its own temporary folder.

## When something is wrong

- **Pley's log** is `logs\pley.log` in the data folder (Start menu:
  **Pley Media Server logs**). The service is **Pley Media Server** in
  Services (`services.msc`).
- **No GPU encoder** although the computer has one: run Setup again and turn
  off **Let the service write only in its data folder**. It is on by
  default.
- **A NAS isn't reached**: choose **As a service, as a user of its own**,
  name the NAS the way the library paths do (`\\nas`, not its address, or
  the other way round), and give a user the NAS knows.
- **The port is taken** by another Pley or another program: stop it or
  choose another port.

## Building the installer

You need [Inno Setup](https://jrsoftware.org/isinfo.php) 6.7 or newer
(`winget install JRSoftware.InnoSetup`), and Rust and Bun to build Pley and
its web app:

```
powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Archive -Installer
```

Or, around a release folder already built:
`scripts\build-installer.ps1 -Release dist\pley-server-<version>-windows-x86_64`.

- **FFmpeg**: the installer carries the FFmpeg on `PATH` (where WinGet's link
  leads), or the one in `-Ffmpeg <an FFmpeg build's bin folder>`, with the
  DLLs next to it and, from beside `bin`, the build's `LICENSE*`,
  `README.txt` and `source\` (a patched build's patch and script, which the
  GPL requires to go with it). Releases carry gyan.dev's full build, at the
  version and checksum set in `.github/workflows/server.yml`.
- **`-Flavor <name>`** adds a name to the file, for an installer built with a
  different FFmpeg: `-Flavor custom` makes
  `pley-server-<version>-custom-windows-x86_64-setup.exe`.
- `scripts\build.ps1 -Installer` passes `-Ffmpeg` and `-Flavor` on.
- `ISCC /DTest …` builds an installer that installs in the notification area
  without administrator rights (no shortcuts, no start at sign-in), to try
  Setup's code on a development machine.
