-- Enable pgcrypto for gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Organizations (GitHub orgs or personal accounts)
CREATE TABLE IF NOT EXISTS organizations (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    github_org_login     TEXT        NOT NULL UNIQUE,
    -- Rules source
    rules_repo           TEXT        NOT NULL DEFAULT '',
    rules_ref            TEXT        NOT NULL DEFAULT 'main',
    -- Policy configuration stored as JSONB
    policy_json          JSONB       NOT NULL DEFAULT '{"fail_on_severity":["error"],"block_merge":true}',
    -- Encrypted GitHub PAT for posting statuses / fetching private rule repos
    github_pat_encrypted BYTEA,
    -- ETag-cached rules YAML
    rules_cache_yaml     TEXT,
    rules_cache_etag     TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Users
CREATE TABLE IF NOT EXISTS users (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    github_id   BIGINT      NOT NULL UNIQUE,
    login       TEXT        NOT NULL,
    name        TEXT,
    avatar_url  TEXT,
    org_id      UUID        NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    is_admin    BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sessions (cookie value is stored as SHA-256 hash — never raw)
CREATE TABLE IF NOT EXISTS sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA       NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- API keys (key itself stored as SHA-256 hash — never raw)
CREATE TABLE IF NOT EXISTS api_keys (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id       UUID        NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    key_hash     BYTEA       NOT NULL UNIQUE,
    created_by   UUID        REFERENCES users (id) ON DELETE SET NULL,
    last_used_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Repositories seen during scans
CREATE TABLE IF NOT EXISTS repositories (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID        NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    github_repo TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, github_repo)
);

-- Scans (one per CI run)
CREATE TABLE IF NOT EXISTS scans (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    repo_id     UUID        NOT NULL REFERENCES repositories (id) ON DELETE CASCADE,
    commit_sha  TEXT        NOT NULL,
    branch      TEXT,
    sarif_json  JSONB       NOT NULL,
    passed      BOOLEAN     NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Findings (denormalized from SARIF for fast querying)
CREATE TABLE IF NOT EXISTS findings (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    scan_id     UUID        NOT NULL REFERENCES scans (id) ON DELETE CASCADE,
    rule_id     TEXT        NOT NULL,
    severity    TEXT        NOT NULL,
    file_path   TEXT        NOT NULL,
    line_number INTEGER,
    message     TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_sessions_token_hash    ON sessions    (token_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash       ON api_keys    (key_hash);
CREATE INDEX IF NOT EXISTS idx_findings_scan_id        ON findings    (scan_id);
CREATE INDEX IF NOT EXISTS idx_findings_severity       ON findings    (severity);
CREATE INDEX IF NOT EXISTS idx_scans_repo_id           ON scans       (repo_id);
CREATE INDEX IF NOT EXISTS idx_repositories_org_id     ON repositories (org_id);
