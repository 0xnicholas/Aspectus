-- ============================================================
-- Migration: 20260624000020_password_policy_and_lockout.sql
-- 描述：为 users 表增加登录失败计数与账户锁定字段，
--       支持账户级防爆破保护。
-- ============================================================

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS failed_login_attempts integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS locked_until timestamptz;
