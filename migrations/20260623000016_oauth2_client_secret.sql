-- ============================================================
-- Migration: 20260623000016_oauth2_client_secret.sql
-- 描述：为 OAuth2 clients 增加 client_secret_hash，用于 /oauth/token 校验
-- ============================================================

ALTER TABLE oauth2_clients
    ADD COLUMN IF NOT EXISTS client_secret_hash VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_oauth2_clients_secret_hash
    ON oauth2_clients(client_id, client_secret_hash);
