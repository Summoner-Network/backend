/*======================================================================
  TAO core schema  –  multi-tenant objects + associations
  ----------------------------------------------------------------------
  • tenant (BIGINT) is part of every PK
  • BIGSERIAL auto-generates object IDs (caller may pass NULL/0)
  • Optimistic concurrency via the `version` column
  • JSONB attrs + GIN indexes for schemaless search
  • Compatible with PostgreSQL ≥ 12 and YugabyteDB ≥ 2.17 (YSQL)
  • Idempotent: every DDL uses IF NOT EXISTS / OR REPLACE
======================================================================*/

-----------------------------------------------------------------------
-- 1. Namespaces
-----------------------------------------------------------------------
CREATE SCHEMA IF NOT EXISTS tao;
SET search_path TO tao, public;

-----------------------------------------------------------------------
-- 2. Objects
--    PK = (tenant, type, id)  where id is BIGSERIAL
-----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS objects (
    tenant      BIGINT      NOT NULL,
    type        INT         NOT NULL,
    id          BIGSERIAL   NOT NULL,
    version     INT         NOT NULL DEFAULT 0,
    attributes  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT objects_pk PRIMARY KEY (tenant, type, id)
);

-- Touch trigger: bump updated_at + version on every UPDATE
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
--    Secondary = (tenant, type, source_id, position) for fast paging
-----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS associations (
    tenant      BIGINT      NOT NULL,
    type        TEXT        NOT NULL,
    source_id   BIGINT      NOT NULL,
    target_id   BIGINT      NOT NULL,
    time        BIGINT      NOT NULL,          -- epoch-ms
    position    BIGINT      NOT NULL,          -- monotonic
    attributes  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT associations_pk PRIMARY KEY (tenant, type, source_id, target_id)
);

CREATE INDEX IF NOT EXISTS associations_srcpos_idx
    ON associations (tenant, type, source_id, position DESC);

CREATE INDEX IF NOT EXISTS associations_attrs_gin
    ON associations USING gin (attributes);

-----------------------------------------------------------------------
-- 4. Upsert helpers
--------------------------------------------------------------------
/*--------------------------------------------------------------
  tao_upsert_object
  • Pass p_id = 0 or NULL to create (id auto-generated)
  • On success returns (id, created)
  • On version clash raises SQLSTATE 40001
--------------------------------------------------------------*/
CREATE OR REPLACE FUNCTION tao_upsert_object(
    p_tenant   BIGINT,
    p_type     INT,
    p_id       BIGINT,     -- 0 / NULL ⇒ insert
    p_exp_ver  INT,        -- expected version
    p_attrs    JSONB
) RETURNS TABLE (id BIGINT, created BOOLEAN) LANGUAGE plpgsql AS $$
DECLARE
    _created BOOLEAN := FALSE;
BEGIN
    /* ---------- INSERT path (no id supplied) ----------------------- */
    IF p_id IS NULL OR p_id = 0 THEN
        INSERT INTO objects (tenant, type, version, attributes)
             VALUES (p_tenant, p_type, 0, p_attrs)
          RETURNING objects.id INTO p_id;
        _created := TRUE;

    ELSE
        /* ---------- ensure row exists or create at explicit id ----- */
        INSERT INTO objects (tenant, type, id, version, attributes)
             VALUES (p_tenant, p_type, p_id, 0, p_attrs)
        ON CONFLICT (tenant, type, id) DO NOTHING;

        /* ---------- UPDATE path with optimistic check -------------- */
        UPDATE objects
           SET attributes = p_attrs,
               updated_at = now(),
               version    = version + 1
         WHERE tenant  = p_tenant
           AND type    = p_type
           AND id      = p_id
           AND version = p_exp_ver;

        IF NOT FOUND THEN
            RAISE EXCEPTION
              'tao_upsert_object: version clash (tenant %, type %, id %)',
              p_tenant, p_type, p_id
              USING ERRCODE = '40001';
        END IF;
    END IF;

    RETURN QUERY SELECT p_id, _created;
END;
$$;

/*--------------------------------------------------------------
  tao_upsert_association  (type = TEXT)
--------------------------------------------------------------*/
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
    INSERT INTO associations (tenant, type, source_id, target_id, time,
                               position, attributes)
         VALUES (p_tenant, p_type, p_source, p_target,
                 p_time,   p_position, p_attrs)
    ON CONFLICT (tenant, type, source_id, target_id) DO UPDATE
        SET time       = p_time,
            position   = p_position,
            attributes = p_attrs,
            created_at = associations.created_at;
END;
$$;

-----------------------------------------------------------------------
-- 5. Delete helpers (soft-delete ready)
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
-- 6. Least-privilege grants (optional)
-----------------------------------------------------------------------
-- GRANT SELECT, INSERT, UPDATE ON objects TO brother_rw;
-- GRANT SELECT                    ON objects TO brother_ro;
-- GRANT EXECUTE ON FUNCTION tao_upsert_object      TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_delete_object      TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_upsert_association TO brother_rw;
-- GRANT EXECUTE ON FUNCTION tao_delete_association TO brother_rw;

-- End of migration