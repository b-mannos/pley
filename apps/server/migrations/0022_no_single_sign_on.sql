-- Single sign-on is gone (pley signed people in through an OpenID Connect
-- provider): the identities linked to accounts, and its settings, the
-- provider's client secret among them.
DROP INDEX IF EXISTS user_identities_user;
DROP TABLE IF EXISTS user_identities;
UPDATE settings
SET value = json_remove(value, '$.public_url', '$.sso_enabled', '$.sso_name', '$.sso_issuer',
                        '$.sso_client_id', '$.sso_client_secret', '$.sso_new_accounts', '$.sso_group')
WHERE key = 'server' AND json_valid(value);
