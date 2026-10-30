-- Live TV channels that don't play: `offline_since` is when the provider
-- first gave nothing for one (no answer, a 404, no data), cleared when it
-- plays again; `offline_checked` is when it was last tried in the
-- background, while its source had a connection free.
ALTER TABLE live_channels ADD COLUMN offline_since INTEGER;
ALTER TABLE live_channels ADD COLUMN offline_checked INTEGER;
