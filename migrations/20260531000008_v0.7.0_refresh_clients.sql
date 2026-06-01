-- ============================================================
-- Migration: 20260531000008_v0.7.0_refresh_clients.sql
-- 描述：v0.7.0 — Refresh tokens + OAuth2 clients
-- ============================================================

CREATE TABLE IF NOT EXISTS oauth2_clients (
    client_id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    redirect_uris TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    token_hash VARCHAR(64) PRIMARY KEY,
    user_id VARCHAR(21) NOT NULL REFERENCES users(id),
    client_id VARCHAR(21) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
