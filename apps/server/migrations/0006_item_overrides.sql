-- Metadata fields edited by hand. They are re-applied after every metadata
-- refresh and scan, so edits stick (like locked fields in other servers).
CREATE TABLE item_overrides (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    field   TEXT    NOT NULL,
    value   TEXT,
    PRIMARY KEY (item_id, field)
) WITHOUT ROWID;
