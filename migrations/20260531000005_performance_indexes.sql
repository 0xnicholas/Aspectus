-- ============================================================
-- Migration: 20260531000005_performance_indexes.sql
-- 描述：v0.3.2 性能索引补充
-- ============================================================

-- api_keys: 管理 API 按 tenant 列出所有 Keys（常见管理操作）
-- (已有 api_keys__tenant on tenant_id, created_at desc — 够用)

-- service_accounts: 管理 API 按 tenant + 时间排序
create index if not exists service_accounts__tenant_created
    on service_accounts (tenant_id, created_at desc);

-- audit_logs: 按时间范围查询审计日志（dashboard 常用）
create index if not exists audit_logs__created_at
    on audit_logs (created_at desc);

-- api_keys: 覆盖索引加速 introspect 查询（最热路径）
-- 包含 revoked_at 和 expires_at 避免回表
-- (已有关键的 key_hash 唯一索引，够用)
