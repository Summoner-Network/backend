/*======================================================================
  TAO core schema  –  multi-tenant objects  +  associations
  ----------------------------------------------------------------------
  • Adds mandatory tenant BIGINT to every row
  • Compatible with PostgreSQL ≥ 12 and YugabyteDB ≥ 2.17 (YSQL layer)
  • Idempotent: CREATE … IF NOT EXISTS everywhere
  • Uses optimistic concurrency via the `version` column
  • All balances / counters stored as BIGINT for audit-friendliness
  • Free-form attrs are kept in JSONB (binary JSON) so GIN indexes work
======================================================================*/

-----------------------------------------------------------------------
-- 1. Namespaces
-----------------------------------------------------------------------
CREATE SCHEMA IF NOT EXISTS tao;
SET search_path TO tao, public;

-----------------------------------------------------------------------
-- 2. Objects
--    PK = (tenant, type, id) — “type” is a small-int enum, id is a 64-bit snowflake
-----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS objects (
    tenant      BIGINT  NOT NULL,        -- ⬅ multi-tenant key
    type        INT     NOT NULL,
    id          BIGINT  NOT NULL,
    version     INT     NOT NULL DEFAULT 0,
    attributes  JSONB   NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT objects_pk PRIMARY KEY (tenant, type, id)
);

-- Bump updated_at + optimistic version on change
CREATE OR REPLACE FUNCTION trg_objects_touch()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    NEW.version    := OLD.version + 1;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS objects_touch ON objects;
CREATE TRIGGER objects_touch
BEFORE UPDATE ON objects
FOR EACH ROW
EXECUTE FUNCTION trg_objects_touch();

-----------------------------------------------------------------------
-- 3. Associations
--    PK = (tenant, type, source_id, target_id)
--    Secondary key = (tenant, type, source_id, position) for fast paging
-----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS associations (
    tenant      BIGINT  NOT NULL,        -- ⬅ multi-tenant key
    type        TEXT    NOT NULL,        -- string
    source_id   BIGINT  NOT NULL,
    target_id   BIGINT  NOT NULL,
    time        BIGINT  NOT NULL,        -- unix epoch millis
    position    BIGINT  NOT NULL,        -- monotonically increasing
    attributes  JSONB   NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT associations_pk PRIMARY KEY (tenant, type, source_id, target_id)
);

-- Fast forward scans by source & position (GetAssociationsRequest)
CREATE INDEX IF NOT EXISTS associations_srcpos_idx
    ON associations (tenant, type, source_id, position DESC);

-- Reverse lookup (fan-in queries) if you need them later
-- CREATE INDEX IF NOT EXISTS associations_target_idx
--     ON associations (tenant, type, target_id);

CREATE INDEX IF NOT EXISTS associations_attrs_gin
    ON associations
 USING gin (attributes);

-----------------------------------------------------------------------
-- 4. Upsert helpers  (used by PutObject & CreateAssociation RPCs)
--------------------------------------------------------------------
/*--------------------------------------------------------------------
  Upsert with optimistic concurrency
  • Returns TRUE  if row was inserted or updated
  • Returns FALSE if UPDATE skipped because version clash
--------------------------------------------------------------------*/
CREATE OR REPLACE FUNCTION tao_upsert_object(
    p_tenant   BIGINT,
    p_type     INT,
    p_id       BIGINT,
    p_exp_ver  INT,       -- expected version from client
    p_attrs    JSONB
) RETURNS BOOL LANGUAGE plpgsql AS $$
DECLARE
    _touched  INT;
BEGIN
    -- 1) Try insert (initial version = 0)
    INSERT INTO objects (tenant, type, id, version, attributes)
         VALUES (p_tenant, p_type, p_id, 0, p_attrs)
    ON CONFLICT (tenant, type, id) DO NOTHING;

    GET DIAGNOSTICS _touched = ROW_COUNT;
    IF _touched > 0 THEN
        RETURN TRUE;      -- brand-new row
    END IF;

    -- 2) Try update, but only if versions match
    UPDATE objects
       SET attributes = p_attrs,
           updated_at = now(),
           version    = version + 1
     WHERE tenant  = p_tenant
       AND type    = p_type
       AND id      = p_id
       AND version = p_exp_ver;     -- optimistic check

    GET DIAGNOSTICS _touched = ROW_COUNT;
    RETURN _touched > 0;
END;
$$;

-----------------------------------------------------------------------
-- 4. Upsert helper  (associations – type TEXT)
-----------------------------------------------------------------------
CREATE OR REPLACE FUNCTION tao_upsert_association(
    p_tenant     BIGINT,
    p_type       TEXT,
    p_source     BIGINT,
    p_target     BIGINT,
    p_time       BIGINT,
    p_position   BIGINT,
    p_attrs      JSONB
) RETURNS VOID LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO associations (tenant, type, source_id, target_id, time, position, attributes)
         VALUES (p_tenant, p_type, p_source, p_target, p_time, p_position, p_attrs)
    ON CONFLICT (tenant, type, source_id, target_id) DO UPDATE
        SET time       = p_time,
            position   = p_position,
            attributes = p_attrs,
            created_at = associations.created_at;
END;
$$;

-----------------------------------------------------------------------
-- 5. Removal helpers  (soft-delete ready: just flip to DELETE)
-----------------------------------------------------------------------
CREATE OR REPLACE FUNCTION tao_delete_object(
    p_tenant BIGINT,
    p_type   INT,
    p_id     BIGINT
) RETURNS BOOLEAN LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM objects
     WHERE tenant = p_tenant
       AND type   = p_type
       AND id     = p_id;
    RETURN FOUND;
END;
$$;

-----------------------------------------------------------------------
-- 5. Delete helper  (associations – type TEXT)
-----------------------------------------------------------------------
CREATE OR REPLACE FUNCTION tao_delete_association(
    p_tenant BIGINT,
    p_type   TEXT,
    p_src    BIGINT,
    p_tgt    BIGINT
) RETURNS BOOLEAN LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM associations
     WHERE tenant    = p_tenant
       AND type      = p_type
       AND source_id = p_src
       AND target_id = p_tgt;
    RETURN FOUND;
END;
$$;

-----------------------------------------------------------------------
-- 6. Grant least-privilege roles (optional)
-----------------------------------------------------------------------
-- GRANT SELECT, INSERT, UPDATE ON objects TO brother_rw;
-- GRANT SELECT                    ON objects TO brother_ro;
-- GRANT EXECUTE ON FUNCTION tao_upsert_object      TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_delete_object      TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_upsert_association TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_delete_association TO brother_rw;

-- End of migration
