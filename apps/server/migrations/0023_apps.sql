-- Apps signed in through pley's own page (OAuth 2.1: src/oauth and
-- src/auth/grants.rs): what a person allowed an app to do. An app is known
-- by the address of its client metadata document (client_id). A grant ends
-- 90 days after it was last used, when it is signed out, or with the
-- account's other sign-ins.
CREATE TABLE app_grants (
    id           INTEGER PRIMARY KEY,
    -- The profile the app acts as (it changes when an app switches profiles).
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Who allowed it: their password change, sign-out everywhere, disabling
    -- or deletion ends it too.
    approved_by  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id    TEXT    NOT NULL,
    -- What the app called itself when it was allowed, for the account page.
    client_name  TEXT,
    -- What it may do, space-separated: 'library play'.
    scope        TEXT    NOT NULL,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    -- 90 days after the last token request.
    expires_at   INTEGER NOT NULL,
    -- Apps by other publishers are asked about again after a year; NULL: no end.
    ends_at      INTEGER,
    -- Of the last token request, for the account page.
    user_agent   TEXT,
    last_ip      TEXT
);
CREATE INDEX app_grants_user ON app_grants(user_id);
CREATE INDEX app_grants_approved_by ON app_grants(approved_by);
-- Their tokens, stored as SHA-256 like session tokens. An access token lasts
-- an hour. A refresh token is exchanged once (used_at) and kept until it
-- expires: shown again, it ends its grant, since someone has a copy.
CREATE TABLE app_tokens (
    token_hash TEXT    PRIMARY KEY,
    grant_id   INTEGER NOT NULL REFERENCES app_grants(id) ON DELETE CASCADE,
    kind       TEXT    NOT NULL CHECK (kind IN ('access', 'refresh')),
    expires_at INTEGER NOT NULL,
    used_at    INTEGER
) WITHOUT ROWID;
CREATE INDEX app_tokens_grant ON app_tokens(grant_id, kind);
