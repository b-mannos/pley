# pley-server on Windows

`pley-server-<version>-windows-<arch>-setup.exe`, on each release, installs
the server as a Windows service or in the notification area, with FFmpeg,
its data folder, the firewall and an uninstaller. It needs Windows 10 1809
or newer and an administrator.

The release archives (`pley-server-<version>-windows-<arch>.zip`) still run as they
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
     Opening it again (the Start menu's `pley-server`) opens its page.
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
4. **Network**: where pley listens.
   - **Only this computer** (the default): on `127.0.0.1`, for a reverse
     proxy on this computer that serves it with HTTPS (Caddy, nginx, IIS).
   - **This computer and these networks**: the computer's network adapters
     are listed with their addresses and the kind of network Windows takes
     each for; pley listens on those ticked (and 127.0.0.1). An adapter
     that isn't up yet as pley starts (Wi-Fi, a VPN such as Tailscale) is
     tried again every 30 s.
   - **Every network**, those that come later too.
   - The port (7539) and the Plex-compatible API (port 32400, for Plex apps),
     on the same networks. Setup says when another program has a port.
   - All of it changes later in pley, while it runs: **Settings → Network**.
5. **Windows Firewall**: which kinds of network to let pley through on
   (private and domain, not public, unless you say otherwise), from devices
   on those networks only or from any address (through a VPN too), and
   whether the pley apps on the network find the server. The rule is for
   pley wherever it listens, so a network chosen later in its settings is
   let through too, if it is of a kind chosen here.

FFmpeg comes with the installer and goes next to pley: a release's carries
gyan.dev's full build (NVENC, Quick Sync, AMF and Vulkan tone mapping), and
an installer built with another one carries that one, such as an FFmpeg built
for Dolby Vision FEL (see Building the installer). Each install replaces it.

The last page gives pley's address and the link that creates the
administrator account (or, with an earlier pley's data, says to sign in as
before), and says which encoders pley found in FFmpeg. A service runs in the
background, without an icon in the notification area: the Start menu's
`pley-server` opens it.

## What Setup sets up

| | |
|---|---|
| Programs | `pley.exe`, `pley-transcoder.exe` and FFmpeg (`ffmpeg\`) in the install folder |
| Options | `pley.toml` in the data folder: the address, the Plex-compatible API, discovery and FFmpeg |
| The service | `pley` ("pley-server" in Services): starts with Windows (delayed), restarts when it stops on its own, has 30 s to stop as Windows shuts down, and keeps its temporary files in the data folder's `tmp` |
| The data folder | Owned by the administrators; only SYSTEM, the administrators, the service (and its user) and you may open it |
| The firewall | A rule named `pley` for pley, on the kinds of network and from the devices chosen |
| The notification area | pley starts when you sign in (Start pley at login, in its menu) |
| Start menu | `pley-server` (pley's page, or pley itself) and `pley-server logs` |

## Media folders

pley only reads them, as its account: a service as `NT SERVICE\pley` (or
the user of its own), in the notification area as you. A folder on another
drive (`D:\Movies`) usually lets every user read it, services included; one
in your user folder (`C:\Users\<you>\Videos`) doesn't. To let the service
read one, from an administrator's terminal:

```
icacls "C:\Users\<you>\Videos" /grant "NT SERVICE\pley:(OI)(CI)RX"
```

(`(OI)(CI)` passes it on to the files and folders inside, `RX` is read
only; for the user of its own, its name instead, `pley` unless you chose
another.) Or in Explorer: the folder's **Properties → Security → Edit →
Add**, **Locations**: this computer, `NT SERVICE\pley`, **Check Names**,
**OK**, and **Read & execute**. It applies at once: scan the library again.
**Let the service write only in its data folder** doesn't limit what it
reads. A drive letter mapped to a NAS exists only for you: give libraries
by their network paths (`\\nas\media\Movies`), with **As a service, as a
user of its own**.

## Updating

Run the new version's setup. It remembers the last install's choices, stops
pley, replaces it and its FFmpeg, and starts it again.

## Uninstalling

**Settings → Apps → pley-server → Uninstall**. The uninstaller removes the
service, the user Setup made, the firewall rule, the start at sign-in, the
programs and FFmpeg, and asks whether to delete the data folder
too (no, unless you say so). A data folder with other files in it than
pley's stays. Setup takes an empty or a new folder for pley's data, or one of
pley's.

## Installing without the wizard

```
pley-server-0.2.0-windows-x86_64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /MODE=service /LISTEN=192.168.1.10
```

| Option | Values | Default |
|---|---|---|
| `/MODE=` | `service`, `user` (a user of its own) or `tray` | `service` |
| `/USER=` | The user's name, with `user` | `pley` |
| `/SHARE=`, `/SHAREUSER=` | The NAS and its user name, with `user` (the password: below) | none |
| `/RESTRICT=` | `yes` or `no`: writes only in its data folder | `yes` |
| `/AUTOSTART=` | `yes` or `no`: start at sign-in, with `tray` | `yes` |
| `/DATADIR=` | The data folder | as above |
| `/COPYDATA=` | A data folder to copy first | none |
| `/LISTEN=` | `local`, `all`, or the addresses of the networks to listen on, separated by commas | `local` |
| `/PORT=` | The port | `7539` |
| `/PLEX=` | `yes` or `no`: the Plex-compatible API | `no` |
| `/FIREWALL=` | The kinds of network to let pley through on, wherever it listens: `private`, `domain`, `public` (separated by commas), or `none` | `private,domain` |
| `/FROM=` | `subnet` (devices on those networks) or `any` (a VPN too) | `subnet` |
| `/DISCOVERY=` | `yes` or `no`: found by the pley apps, when others may reach it | `yes` |

The NAS's password comes from the environment variable
`PLEY_SHARE_PASSWORD`, never the command line, which Setup writes in its
log. Set it in an administrator's PowerShell and run Setup from there (a
Setup that asks for administrator rights itself starts without it):

```
$env:PLEY_SHARE_PASSWORD = [Net.NetworkCredential]::new('', (Read-Host 'NAS password' -AsSecureString)).Password
.\pley-server-0.2.0-windows-x86_64-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /MODE=user /SHARE=\\nas /SHAREUSER=media
```

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
- **The network.** pley listens on `127.0.0.1` unless told otherwise (in
  Setup, or by an administrator in **Settings → Network**), and the
  firewall lets it through on private and domain networks only, unless you
  choose public ones too.
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
`scripts\build-installer.ps1 -Release dist\pley-server-<version>-windows-x86_64`.
The installer carries the FFmpeg on PATH (where WinGet's link leads), or the
one `-Ffmpeg <an FFmpeg build's bin folder>` names, with the DLLs next to it;
`-Flavor fel` adds a name to the file:
`pley-server-<version>-fel-windows-x86_64-setup.exe`. Releases carry
gyan.dev's full build, at the version and checksum set in `release.yml`.
`ISCC /DTest …` builds one that installs in the notification area without
administrator rights (no shortcuts nor start at sign-in), to try Setup's
code on a development machine.
