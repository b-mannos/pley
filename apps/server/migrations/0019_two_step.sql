-- Two-step sign-in (src/two_step.rs): after the password, a code from an
-- authenticator app, or one of the account's recovery codes.
CREATE TABLE two_step (
    user_id    INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- The authenticator's secret, sealed with the key in <data>/secret.key.
    secret     TEXT    NOT NULL,
    -- The time step of the last code accepted: each code works once.
    last_step  INTEGER NOT NULL DEFAULT 0,
    -- Wrong codes in a row; from 100 on, only recovery codes sign in.
    failures   INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Each works once. SHA-256 of the user id and the code (80 random bits).
CREATE TABLE recovery_codes (
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT    NOT NULL,
    PRIMARY KEY (user_id, code_hash)
) WITHOUT ROWID;

-- Wrong PINs in a row when switching profiles: from 100 on, the PIN stops
-- working until it is set again.
ALTER TABLE users ADD COLUMN pin_failures INTEGER NOT NULL DEFAULT 0;
