-- Labels on items: free-form tags an administrator, or a tool such as
-- Kometa through the Plex-compatible API, puts on titles to find them
-- again (`label=` filters) and keep track of what it did.
CREATE TABLE item_labels (
    item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    label   TEXT    NOT NULL COLLATE NOCASE,
    PRIMARY KEY (item_id, label)
) WITHOUT ROWID;
CREATE INDEX item_labels_label ON item_labels(label, item_id);
