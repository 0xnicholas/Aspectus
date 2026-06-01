-- ============================================================
-- Migration: 20260531000007_v0.6.0_oauth2.sql
-- 描述：v0.6.0 — OAuth2 authorization codes
-- ============================================================

CREATE TABLE IF NOT EXISTS authorization_codes (
    code VARCHAR(64) PRIMARY KEY,
    user_id VARCHAR(21) NOT NULL REFERENCES users(id),
    client_id VARCHAR(21) NOT NULL,
    redirect_uri TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false
);
