-- ============================================================
-- Migration: 20260531000002_indexes_and_triggers.sql
-- 描述：补充分散在 001 之外的索引和自动更新触发器
-- ============================================================

-- api_keys 按 tenant 查询（管理 API：列出 tenant 的所有 Key）
-- 注意：api_keys__key_hash、api_keys__service_account、
--       api_keys__service_account_id 已在 001 中创建
create index api_keys__tenant
    on api_keys (tenant_id, created_at desc);

-- audit_logs 按 target 查询
create index audit_logs__target
    on audit_logs (target_type, target_id);

-- service_tokens 按 token_hash 查找（/introspect 认证热路径）
create index service_tokens__token_hash
    on service_tokens (token_hash);

-- 触发器：自动更新 updated_at
create or replace function set_updated_at()
returns trigger as $$
begin
    new.updated_at = now();
    return new;
end;
$$ language plpgsql;

create trigger set_users_updated_at
    before update on users
    for each row execute procedure set_updated_at();

create trigger set_service_tokens_updated_at
    before update on service_tokens
    for each row execute procedure set_updated_at();
