-- ============================================================
-- Migration: 20260619000012_tenant_logo_url.sql
-- 描述：ADR-016 — tenants 表增加 logo_url 列
--       /login/lookup 的 TenantOption 预留字段现在可被填充
-- ============================================================

ALTER TABLE tenants ADD COLUMN IF NOT EXISTS logo_url VARCHAR(512);
