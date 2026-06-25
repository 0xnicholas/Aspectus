-- ============================================================
-- Migration: 20260624000001_role_system_flag.sql
-- 描述：为 roles 表增加 is_system 标记，用于区分系统预置角色
--       与自定义角色。系统角色不可删除/修改。
-- ============================================================

ALTER TABLE roles ADD COLUMN IF NOT EXISTS is_system boolean NOT NULL DEFAULT false;

-- 将现有系统预置角色标记为 is_system。
-- 当前 is_default = true 的角色均为系统角色。
UPDATE roles SET is_system = true WHERE is_default = true;
