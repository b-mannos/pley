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
  apps warn before connecting to an unencrypted address on the internet.
- **Set it up before exposing it.** A new server is set up with the one-time
  code pley prints in its log (the tray icon's "Open pley" fills it in).
- **Keep what doesn't need to be public off the internet:** the
  Plex-compatible API on port 32400 (turn it off with `plex_bind = "off"` if
  no Plex app needs it), and any transcoder on another machine.
- **Mount media read-only** (the Docker example does), run pley as an
  unprivileged user, and don't let untrusted people write to library
  folders.
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
minutes from each address; a restart makes a new code.

**Passwords.** Argon2id hashes; at least 8 characters, not the username, not
a common password. Sign-ins, PINs and the current password on changes are
limited to 5 failures every 15 minutes per account, and password checks
queue, so a flood of sign-ins can't exhaust memory. A PIN stops working
after 100 wrong tries in a row, until it is set again. A new password signs
the account out on every other device.

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
token. Event streams end once their session is signed out. Switching from a
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
is refused, and the Plex-compatible API refuses their tokens. Apps also
don't see library folders or file paths, the transcoder's error messages
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
start one. At most 20 are in progress from one address and 1,000 in all,
and none is dropped to make room; past 10 wrong codes from one person or 300
from everyone in 10 minutes, no code is looked up. A TV asking too often is
slowed down. Its answer comes once, and a TV allowed before the account's
sign-ins end, or from a session signed out since, gets nothing.

**Pages.** A `Content-Security-Policy` allows only pley's own scripts and
styles, no other site may frame pley (`frame-ancestors 'none'`,
`X-Frame-Options: DENY`), and answers carry `nosniff` and
`Referrer-Policy: same-origin`. Clients get 30 seconds to send a request's
headers.

**Files.** A symbolic link in a library is followed only to a file of the
same kind (a video to a video), when the library is scanned and again when
the file is played: a link named like a film can't hand out other files. On
Unix the data folder (database, backups, log) is readable by pley's account
only.

**Transcoder.** The transcoder pley starts listens on `127.0.0.1` with a
random token. One started on another machine refuses to listen on the
network without a token of at least 16 characters, since it reads whatever
files it can open, and keeps its work folder private.

**Apps.** The desktop and Android windows show the server's own pages only:
other sites open in the browser. The desktop app hands only web and mail
links to the system and gives server pages none of its commands. The
Android app offers its player to the server's own pages only, sends the
session token to the server only (not across redirects), and keeps WebView
file access off. Its native sign-in, not yet used by its screens, opens the
server's page in the browser and keeps only app tokens, encrypted with a
key from the Android Keystore and sent to the server's own address only; it
never sees passwords or the web view's cookie.
