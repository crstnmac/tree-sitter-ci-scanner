-- PostgreSQL 18 optimisations
-- ──────────────────────────────────────────────────────────────────────────────

-- 1. UUIDv7 primary-key defaults for high-write tables
-- ─────────────────────────────────────────────────────
-- UUIDv4 (gen_random_uuid) is uniformly random, so every INSERT lands at a
-- random leaf page in the B-tree, causing constant page splits and random I/O.
-- UUIDv7 encodes a millisecond-precision Unix timestamp in the top 48 bits,
-- making new rows always append to the rightmost leaf — sequential writes,
-- no splits, much lower write amplification.
--
-- uuidv7() is a built-in function since PG 17 (no extension required).
-- Existing rows keep their UUIDv4 values; only new rows get UUIDv7.
ALTER TABLE scans    ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE findings ALTER COLUMN id SET DEFAULT uuidv7();


-- 2. Virtual generated column: severity sort order
-- ─────────────────────────────────────────────────
-- PostgreSQL 18 introduces VIRTUAL generated columns: values are computed
-- at query time from other columns, with zero storage overhead (unlike
-- STORED generated columns which write to disk on every INSERT/UPDATE).
--
-- severity_order maps the free-text severity to a stable integer so callers
-- can ORDER BY severity_order instead of embedding a CASE expression in every
-- query. It costs nothing to store and is always consistent with severity.
ALTER TABLE findings
    ADD COLUMN IF NOT EXISTS severity_order SMALLINT
        GENERATED ALWAYS AS (
            CASE severity
                WHEN 'error'   THEN 0
                WHEN 'warning' THEN 1
                WHEN 'note'    THEN 2
                ELSE                3
            END
        ) VIRTUAL;
