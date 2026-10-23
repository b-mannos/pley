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
- **Single sign-on:** use a provider you control, or set the group that may
  sign in; turn on new accounts only then.
- **Two-step sign-in:** turn it on for administrators of a server reachable
  from the internet (Account → Two-step sign-in), and keep the recovery codes
  somewhere safe.

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
Switching to a profile with its password, and linking the account to a
single sign-on identity, take its code too. Single sign-on itself skips it:
the provider asks for its own.

**Sessions.** Random 256-bit tokens, stored hashed. Browsers keep them in an
`HttpOnly`, `SameSite=Lax` cookie only: no answer gives a page's script the
token. Event streams end once their session is signed out. Switching from a
restricted profile to one that can see more (an administrator, a higher age
limit, other libraries) takes that profile's PIN, or its password (and code).

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

**Single sign-on** (OpenID Connect): the authorization code flow with PKCE,
a `state` bound to the browser by a cookie, and the ID token's signature,
issuer, audience, nonce and expiry checked. pley asks only who signed in
(the scopes `openid email`, `profile` to name new accounts, `groups` to check
a group) and keeps none of the provider's tokens. Identities are linked to
accounts explicitly, with the account's password, never matched by name or
e-mail address; new accounts start without libraries. The client secret is
never sent back to browsers.

**Apps.** The desktop app hands only web and mail links to the system and
gives server pages none of its commands. The Android app offers its player
to the server's own pages only, sends the session token to the server only
(not across redirects), and keeps WebView file access off.
