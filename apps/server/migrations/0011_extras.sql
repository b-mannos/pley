-- Bonus material (trailers, featurettes, …) is stored as items of kind
-- 'extra' under their movie or show; this says which kind of extra.
ALTER TABLE items ADD COLUMN extra_type TEXT;
