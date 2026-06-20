-- ============================================================
-- Migration: 20260620000013_ksuid_widen_columns.sql
-- 描述：将所有 varchar(21) ID 列扩展为 varchar(27)，支持 KSUID base62 格式
--       KSUID base62 = 27 chars。现有 hex ID (21 chars) 完全兼容。
-- 策略：先卸所有 FK → 扩展所有列 → 重建所有 FK
-- ============================================================

-- Phase 1: Drop all foreign key constraints
ALTER TABLE api_keys         DROP CONSTRAINT IF EXISTS api_keys_tenant_id_fkey;
ALTER TABLE api_keys         DROP CONSTRAINT IF EXISTS api_keys_service_account_id_fkey;
ALTER TABLE service_accounts DROP CONSTRAINT IF EXISTS service_accounts_tenant_id_fkey;
ALTER TABLE users            DROP CONSTRAINT IF EXISTS users_tenant_id_fkey;
ALTER TABLE users_roles      DROP CONSTRAINT IF EXISTS users_roles_user_id_fkey;
ALTER TABLE users_roles      DROP CONSTRAINT IF EXISTS users_roles_role_id_fkey;
ALTER TABLE roles_scopes     DROP CONSTRAINT IF EXISTS roles_scopes_role_id_fkey;
ALTER TABLE roles_scopes     DROP CONSTRAINT IF EXISTS roles_scopes_scope_id_fkey;
ALTER TABLE audit_logs       DROP CONSTRAINT IF EXISTS audit_logs_tenant_id_fkey;

-- Note: password_reset_tokens.user_id REFERENCES users(id) — varchar(64), no change needed
-- Note: authorization_codes / refresh_tokens / oauth2_clients — already varchar(64) or varchar(27)

-- Phase 2: Widen all varchar(21) columns to varchar(27)
-- Child tables first, then PK tables (cosmetic — FKs already dropped)

-- leaf: api_keys
ALTER TABLE api_keys         ALTER COLUMN id                 TYPE varchar(27);
ALTER TABLE api_keys         ALTER COLUMN tenant_id          TYPE varchar(27);
ALTER TABLE api_keys         ALTER COLUMN service_account_id TYPE varchar(27);

-- leaf: service_accounts
ALTER TABLE service_accounts ALTER COLUMN id        TYPE varchar(27);
ALTER TABLE service_accounts ALTER COLUMN tenant_id TYPE varchar(27);

-- leaf: users
ALTER TABLE users            ALTER COLUMN id        TYPE varchar(27);
ALTER TABLE users            ALTER COLUMN tenant_id TYPE varchar(27);

-- leaf: users_roles
ALTER TABLE users_roles      ALTER COLUMN id      TYPE varchar(27);
ALTER TABLE users_roles      ALTER COLUMN user_id TYPE varchar(27);
ALTER TABLE users_roles      ALTER COLUMN role_id TYPE varchar(27);

-- leaf: roles_scopes
ALTER TABLE roles_scopes     ALTER COLUMN id       TYPE varchar(27);
ALTER TABLE roles_scopes     ALTER COLUMN role_id  TYPE varchar(27);
ALTER TABLE roles_scopes     ALTER COLUMN scope_id TYPE varchar(27);

-- leaf: audit_logs
ALTER TABLE audit_logs       ALTER COLUMN id        TYPE varchar(27);
ALTER TABLE audit_logs       ALTER COLUMN tenant_id TYPE varchar(27);
ALTER TABLE audit_logs       ALTER COLUMN actor_id  TYPE varchar(27);
ALTER TABLE audit_logs       ALTER COLUMN target_id TYPE varchar(27);

-- PK tables (no incoming FKs remaining)
ALTER TABLE roles            ALTER COLUMN id TYPE varchar(27);
ALTER TABLE scopes           ALTER COLUMN id TYPE varchar(27);
ALTER TABLE tenants          ALTER COLUMN id TYPE varchar(27);

-- misc
ALTER TABLE service_tokens   ALTER COLUMN project TYPE varchar(27);

-- Phase 3: Recreate foreign key constraints
ALTER TABLE api_keys         ADD CONSTRAINT api_keys_tenant_id_fkey          FOREIGN KEY (tenant_id)          REFERENCES tenants(id);
ALTER TABLE api_keys         ADD CONSTRAINT api_keys_service_account_id_fkey FOREIGN KEY (service_account_id) REFERENCES service_accounts(id);
ALTER TABLE service_accounts ADD CONSTRAINT service_accounts_tenant_id_fkey  FOREIGN KEY (tenant_id)          REFERENCES tenants(id);
ALTER TABLE users            ADD CONSTRAINT users_tenant_id_fkey             FOREIGN KEY (tenant_id)          REFERENCES tenants(id);
ALTER TABLE users_roles      ADD CONSTRAINT users_roles_user_id_fkey         FOREIGN KEY (user_id)            REFERENCES users(id);
ALTER TABLE users_roles      ADD CONSTRAINT users_roles_role_id_fkey         FOREIGN KEY (role_id)            REFERENCES roles(id);
ALTER TABLE roles_scopes     ADD CONSTRAINT roles_scopes_role_id_fkey        FOREIGN KEY (role_id)            REFERENCES roles(id);
ALTER TABLE roles_scopes     ADD CONSTRAINT roles_scopes_scope_id_fkey       FOREIGN KEY (scope_id)           REFERENCES scopes(id);
ALTER TABLE audit_logs       ADD CONSTRAINT audit_logs_tenant_id_fkey        FOREIGN KEY (tenant_id)          REFERENCES tenants(id);
