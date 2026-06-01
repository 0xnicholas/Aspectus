-- ============================================================
-- Migration: 20260531000005_v0.4.0_token_type.sql
-- 描述：v0.4.0 — api_keys 支持 token_type 区分
-- ============================================================

ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS token_type VARCHAR(16) NOT NULL DEFAULT 'api_key';
