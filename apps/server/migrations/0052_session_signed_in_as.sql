-- The profile that signed a browser in, when it switched to another one
-- since: it goes back to that profile without its password (see
-- api::auth::check_switch), and ends with that account's sign-ins or the
-- account itself, as an app ends with whoever allowed it
-- (app_grants.approved_by). NULL: the session's own profile.
ALTER TABLE sessions ADD COLUMN signed_in_as INTEGER REFERENCES users(id) ON DELETE CASCADE;
CREATE INDEX sessions_signed_in_as ON sessions(signed_in_as);
