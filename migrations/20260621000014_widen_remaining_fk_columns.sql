-- ============================================================
-- Migration: 20260621000014_widen_remaining_fk_columns.sql
-- 描述：把 remaining FK columns 从 varchar(21) widen 到 varchar(27)
--
-- Background:
-- Migration #13 (20260620000013_ksuid_widen_columns.sql) widened
-- users.id / tenants.id / service_accounts.id / oauth2_clients.id /
-- roles.id / scopes.id from varchar(21) to varchar(27), but
-- its note claimed:
--
--   "authorization_codes / refresh_tokens / oauth2_clients —
--    already varchar(64) or varchar(27)"
--
-- This was INACCURATE for the *_user_id and *_client_id columns
-- of authorization_codes, refresh_tokens, password_reset_tokens,
-- and api_keys (added in v0.5.0 by migration #6). Those columns
-- were created as VARCHAR(21) AFTER the initial migration and
-- missed by the widening sweep.
--
-- Symptom:
-- INSERT INTO authorization_codes (user_id, ...) VALUES (... 27 chars ...)
-- fails with: "value too long for type character varying(21)"
--
-- This breaks the entire OAuth2 flow:
--   - /oauth/token returns 401 "Invalid or expired code"
--   - The /authorize handler silently swallows the INSERT error
--     (it uses `let _ = ...`) and returns 200 to the client, who
--     then gets a code that does not exist in the DB.
--
-- Affected columns (all VARCHAR(21) → VARCHAR(27) or VARCHAR(64)):
--   - api_keys.user_id              → varchar(27)  (refs users.id)
--   - password_reset_tokens.user_id → varchar(27)  (refs users.id)
--   - authorization_codes.user_id   → varchar(27)  (refs users.id)
--   - authorization_codes.client_id → varchar(64)  (refs oauth2_clients.client_id)
--   - refresh_tokens.user_id        → varchar(27)  (refs users.id)
--   - refresh_tokens.client_id      → varchar(64)  (refs oauth2_clients.client_id)
--
-- Note: oauth2_clients.client_id is varchar(64) and the actual value is
-- "client_<21-char-ksuid>" = 28 chars total — does NOT fit in varchar(27).
-- Hence client_id columns widen to 64 to match the parent.
-- ============================================================

-- Phase 1: Drop FK constraints that reference the columns we're about to widen
ALTER TABLE api_keys              DROP CONSTRAINT IF EXISTS api_keys_user_id_fkey;
ALTER TABLE authorization_codes   DROP CONSTRAINT IF EXISTS authorization_codes_user_id_fkey;
ALTER TABLE refresh_tokens        DROP CONSTRAINT IF EXISTS refresh_tokens_user_id_fkey;
ALTER TABLE password_reset_tokens DROP CONSTRAINT IF EXISTS password_reset_tokens_user_id_fkey;

-- Phase 2: Widen columns
ALTER TABLE api_keys              ALTER COLUMN user_id     TYPE varchar(27);
ALTER TABLE authorization_codes   ALTER COLUMN user_id     TYPE varchar(27);
ALTER TABLE authorization_codes   ALTER COLUMN client_id   TYPE varchar(64);
ALTER TABLE refresh_tokens        ALTER COLUMN user_id     TYPE varchar(27);
ALTER TABLE refresh_tokens        ALTER COLUMN client_id   TYPE varchar(64);
ALTER TABLE password_reset_tokens ALTER COLUMN user_id     TYPE varchar(27);

-- Phase 3: Recreate FK constraints
ALTER TABLE api_keys              ADD CONSTRAINT api_keys_user_id_fkey              FOREIGN KEY (user_id)     REFERENCES users(id);
ALTER TABLE authorization_codes   ADD CONSTRAINT authorization_codes_user_id_fkey   FOREIGN KEY (user_id)     REFERENCES users(id);
ALTER TABLE refresh_tokens        ADD CONSTRAINT refresh_tokens_user_id_fkey        FOREIGN KEY (user_id)     REFERENCES users(id);
ALTER TABLE password_reset_tokens ADD CONSTRAINT password_reset_tokens_user_id_fkey FOREIGN KEY (user_id)     REFERENCES users(id) ON DELETE CASCADE;

-- Note: authorization_codes.client_id and refresh_tokens.client_id reference
-- oauth2_clients.client_id (varchar(64)) — no FK was declared in the initial
-- migration, so no constraint to recreate here. Same for api_keys.user_id
-- referencing users(id) which is the only FK we re-create.
