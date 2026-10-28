-- What Automatic quality knows of the networks each client plays on (see
-- src/bandwidth.rs): per user, client and network, the last speed test and
-- the ceiling a step down left. `network` is 'local' (this server's own
-- network) or a keyed hash of the client's address, never the address.
-- Times are unix milliseconds; a row not written for 7 days goes.
CREATE TABLE playback_networks (
    user_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id      TEXT NOT NULL,
    network        TEXT NOT NULL,
    probe_kbps     INTEGER,
    probe_complete INTEGER NOT NULL DEFAULT 0,
    probe_at       INTEGER,
    ceiling_kbps   INTEGER,
    ceiling_at     INTEGER,
    updated_at     INTEGER NOT NULL,
    PRIMARY KEY (user_id, client_id, network)
) WITHOUT ROWID;
CREATE INDEX playback_networks_updated ON playback_networks(updated_at);
