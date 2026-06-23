-- Service Token 生命周期管理 (v0.9.3)
--
-- 为 service_tokens 增加：
--   - token_prefix: 列表/详情展示用前缀（类似 api_keys.key_prefix）
--   - revoked_at: 软吊销时间戳，保留审计轨迹

ALTER TABLE service_tokens
    ADD COLUMN IF NOT EXISTS token_prefix varchar(16),
    ADD COLUMN IF NOT EXISTS revoked_at timestamptz;

COMMENT ON COLUMN service_tokens.token_prefix IS
    'Prefix of the service token used for display in management UI. NEVER stores the full token.';

COMMENT ON COLUMN service_tokens.revoked_at IS
    'Soft-revocation timestamp. NULL means the token is active.';
