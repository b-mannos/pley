# Security

## Reporting a vulnerability

Report vulnerabilities privately through GitHub: **Security → Report a
vulnerability** on this repository. Please don't open a public issue for
them. Include the version (`pley --version`), how pley is deployed, and the
steps to reproduce.

Only the latest release gets fixes.

## Running pley safely

- **Use HTTPS beyond your home network.** pley speaks plain HTTP. When it can
  be reached from outside, put it behind a reverse proxy with TLS that sets
  `X-Forwarded-Proto: https` (pley then marks its cookies `Secure`). The
  desktop app warns before connecting to an unencrypted address on the
  internet, and the Android app refuses one.
- **Set it up before exposing it.** A new server is set up with the one-time
  code pley prints in its log (the tray icon's "Open pley" fills it in).
- **Keep what doesn't need to be public off the internet:** any
  transcoder on another machine, and the compatibility API's own port 32400
  if you gave it one. By default that API answers on pley's port, to its
  clients only, with a token; give the tools that use it an account of
  their own rather than an administrator's, and turn it off
  (`plex_bind = "off"`) if no tool needs it.
- **Mount media read-only**, run pley as an unprivileged user, and don't let
  untrusted people write to library folders. The Docker setup does the
  first two and more: a read-only image, no capabilities, the port on
  127.0.0.1 only ([deploy/docker/README.md](deploy/docker/README.md#security)).
- **Two-step sign-in:** turn it on for administrators of a server reachable
  from the internet (Account → Two-step sign-in), and keep the recovery codes
  somewhere safe.
- **Leave apps by other publishers off** (Settings → Server → Apps) unless
  you need one.
- **Behind a reverse proxy, pass the original `Host`** and set
  `X-Forwarded-Proto`, overwriting what clients send (see
  [deploy/README.md](deploy/README.md#apps-and-reverse-proxies)).

## What pley does

**Setup.** The administrator account takes the setup code from the log,
wherever the request seems to come from: Docker, reverse proxies and DNS
rebinding make the internet look local. Guesses are limited to 5 every 15
minutes from each address (behind a proxy, each client's), 1000 from all of
them; a restart makes a new code.

**Passwords.** Argon2id hashes; at least 8 characters, not the username, not
a common password. Sign-ins are limited to 5 failures every 15 minutes per
account from each address and 50 from all of them, so someone guessing
elsewhere doesn't lock its owner out; PINs and the current password on
changes to 5 per account. Password checks queue and failures are forgotten
as their window ends, so a flood of sign-ins can't exhaust memory; when the
table of counts fills, those that block nobody give way, so lockouts stay
and everyone else signs in. A PIN stops working
after 100 wrong tries in a row, until it is set again. A new password signs
the account out on every other device. `pley reset-password` asks for the
new password (or reads it from standard input) rather than taking it on its
command line, where the shell's history and the list of processes keep it.

**Two-step sign-in** (optional): after the password, a code from an
authenticator app (TOTP) or one of ten recovery codes. Turning it on takes
the password and a first code; turning it off, the password and a code. A
code works once; wrong codes are limited like passwords, and after 100 in a
row only a recovery code signs in. The authenticator secrets are encrypted
with a key kept in the data folder (`secret.key`), so a copy of the database
or of its backups can't make codes; recovery codes are stored hashed.
Switching to a profile with its password takes its code too. Apps sign in
on the same page, so they take the code as well.

**Sessions.** Random 256-bit tokens, stored hashed. Browsers keep them in an
`HttpOnly`, `SameSite=Lax` cookie only: no answer gives a page's script the
token. What changes anything with the cookie must come from pley's own page
(`Sec-Fetch-Site`, or `Origin`): a page on another port of the same host is
the same site to the browser. Other sites' pages can't have the API fetch
anything with it either (links, pictures), only the address bar and
bookmarks; a `?token=` in their address changes none of that, since the
cookie is what would sign them in. The devices and apps lists show the
address a request came from as the rate limits count it, which nobody on
the internet can choose. Administrators change their own password and
two-step sign-in in Account, which asks for the password (and code), not
through the users list. Event streams end once their session is signed out. Switching from a
restricted profile to one that can see more (an administrator, a higher age
limit, other libraries) takes that profile's PIN, or its password (and code).

**Apps signing in.** Apps sign in on pley's own page with OAuth 2.1:
authorization codes with PKCE (S256 only), return addresses matched exactly
(only a loopback port may differ), codes that work once within 60 seconds,
and `iss` in every answer. Tokens are random, stored hashed and go only in
the `Authorization` header. Access tokens last an hour; refresh tokens are
rotated on every use, and one used twice ends the app's access. People are
asked every time, pley's own apps included, on a page that shows the app's
address first and can't be framed. Apps never get administrator rights:
every route they may use is listed with the scope it needs, anything else
is refused, and the compatibility API refuses their tokens. Apps, and
people who aren't administrators, don't see library folders or file paths
on either API (only a file's name), the transcoder's error messages
included. pley's own apps are built in and sign in without internet access.
Descriptions of other publishers' apps are read only when an administrator
allows those apps, for someone signed in, from public addresses only,
without redirects, at most 5 KiB; reading one tells the app's host that this
server is in use. A new password, "Sign out everywhere", `pley reset-password`
and deleting the account sign its apps out with its other sign-ins, and the
apps it was signing in get nothing: a code is exchanged only while the
browser session that allowed it is signed in, and codes not exchanged yet
end with the account's sign-ins. App sign-in works over plain HTTP at home,
as passwords do; beyond the home network it needs HTTPS like the rest of
pley.

**TVs signing in.** pley's own apps on a TV show a code of 8 letters (about
2.6 × 10¹⁰ of them) that someone signed in types on pley's `/link` page,
where they see the same consent as other apps and are told to allow only a
code on a screen in front of them (RFC 8628 §5.4). Only pley's own apps can
start one. At most 20 are in progress from one address (an IPv6 /64), 50
from one home's /56 and 1,000 in all, and none is dropped to make room; past 10 wrong codes from one person or 300
from everyone in 10 minutes, no code is looked up. A TV asking too often is
slowed down. Its answer comes once, and a TV allowed before the account's
sign-ins end, or from a session signed out since, gets nothing. A signed
out app's number is never given to another, so its keyed media addresses
stay dead.

**Pages.** A `Content-Security-Policy` allows only pley's own scripts and
styles, no other site may frame pley (`frame-ancestors 'none'`,
`X-Frame-Options: DENY`), and answers carry `nosniff` and
`Referrer-Policy: same-origin`. Clients get 30 seconds to send a request's
headers, and a body may pause for 20 seconds at most. At most 8,192
connections are open at once, 256 from one address on the internet (a
proxy on this computer or its network carries everyone's, so its aren't
counted), and a session keeps at most 16 event streams; on Unix pley raises
its open-files limit to the most it may have.

**Files.** A symbolic link in a library is followed only to a file of the
same kind (a video to a video, a picture to a picture), when the library is
scanned and again whenever the file is used (played directly or
transcoded, downloaded, its pictures or subtitles served): a link named like
a film can't hand out other files. The data folder (database, backups, log,
settings) is kept to pley's account: mode 0700 on Unix; on Windows, a
folder every account could use (one on D:\, say) is limited to pley's
account, SYSTEM and the administrators. pley warns at start when every
account may change the folder its programs are in, since any of them could
replace them. The Windows service takes a data
folder only when it is new or the administrators' (one an ordinary user
made beforehand would stay theirs to open up again), makes a new one closed
from the start, and keeps two privileges, never impersonation, restricted
or not.

**Untrusted media.** What FFmpeg hands back for one picture or subtitle
track is capped (64 MB, and 512 MB for an extracted track); a crafted
file's subtitle styles and MP4 sample tables can't take more memory than a
real one's. Pictures come only from the metadata services' hosts over
HTTPS, redirects included, read up to 25 MB whatever their headers say; a
profile picture is decoded only up to 4096 pixels.

**Transcoder.** The transcoder pley starts listens on `127.0.0.1` with a
random token. One started on another machine refuses to listen on the
network without a token of at least 16 characters, since it reads whatever
files it can open, and keeps its work folder private: on Windows too, the
folders it makes in one chosen elsewhere (segments, converted downloads, its
caches). A converted download several people asked for stops only when the
last of them, or an administrator, cancels it.

**The desktop app.** Its window shows the server's own pages only: other
sites open in the browser. It hands only web and mail links to the system
and gives server pages none of its commands.

**The Android app** is native and shows no web pages. It holds app tokens
only, with a viewer's rights whoever signs in: never a session or a
password. Phones sign in on the server's own page in a Custom Tab, with
PKCE (a private tab where the browser offers one, unless turned off in
settings); TVs show a code that is typed on the server's `/link` page. A
token answer with more than the app asked for is revoked at once and never
used.

Its tokens are encrypted with AES-GCM under a 256-bit key that the Android
Keystore makes and keeps, bound to their server's address, in the app's
private storage. Backups and transfers to a new device are off, so they
never leave the device; when the key is lost (a reset), the app is signed
out. Signing out revokes them on the server.

Tokens go in the `Authorization` header only, to the server's own origin
(scheme, host and port) only, checked on every hop: a redirect or a playlist
pointing elsewhere gets none, and redirects never switch between https and
http. Over plain http, codes and tokens go only to private addresses
(loopback, RFC 1918, link-local, CGNAT, IPv6 unique local), checked on the
connection itself, so a name that turns out to be on the internet is
refused too; a server on the internet needs https. Only the system's
certificate authorities are trusted.

It replaced a web view app, whose session had the account's full rights.
An update over it ends that session once: the cookie it kept for each saved
server goes to `POST /api/auth/logout` (best effort: a server out of reach
keeps it until it expires), then the web view's cookies, storage and files
are deleted. Nothing else in the app reads cookies. Besides its two
launchers, the app exports only the sign-in's return address and the music
service.
