# Security

## Reporting a vulnerability

Report vulnerabilities privately through GitHub: **Security → Report a
vulnerability** on this repository. Please don't open a public issue for
them. Include the version (`pley --version`), how pley is deployed, and the
steps to reproduce.

Only the latest release gets fixes.

## Running pley safely

- pley speaks plain HTTP. When it can be reached from outside your network,
  put it behind a reverse proxy with TLS that sets `X-Forwarded-Proto: https`
  (pley then marks its session cookie `Secure`).
- The Plex-compatible API listens on port 32400 on every interface. Turn it
  off with `plex_bind = "off"` (or `--plex-bind off`) if no Plex app needs it.
- The transcoder listens on `127.0.0.1:7540`. When it runs on another
  machine, give it and the server the same token (`PLEY_TRANSCODER_TOKEN`)
  and keep its port off the internet: it reads whatever files it can open.
- Mount media read-only (the Docker example does) and run pley as an
  unprivileged user.
