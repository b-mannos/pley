-- Each user's own star rating: 0-10, one point per half star (Plex's
-- userRating). NULL: not rated.
ALTER TABLE user_item_state ADD COLUMN rating REAL;
ALTER TABLE user_item_state ADD COLUMN rated_at INTEGER;
