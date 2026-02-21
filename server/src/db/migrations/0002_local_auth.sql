-- Allow local (username + password) accounts alongside GitHub OAuth.

-- github_id is no longer mandatory — local accounts won't have one.
ALTER TABLE users ALTER COLUMN github_id DROP NOT NULL;

-- Argon2id hash of the user's password (NULL for OAuth-only accounts).
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_hash TEXT;

-- Every row must have at least one auth method.
ALTER TABLE users
    ADD CONSTRAINT users_has_auth_method
    CHECK (github_id IS NOT NULL OR password_hash IS NOT NULL);
