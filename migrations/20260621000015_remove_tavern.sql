-- ============================================================
-- Migration: 20260621000015_remove_tavern.sql
-- 描述：Tavern 停止存在 (2026-06-21)。从系统中移除 Tavern 痕迹。
--
-- Background:
-- Tavern was originally a separate ecosystem project. As of 2026-06-21,
-- Tavern's functionality has been merged into Pandaria as a subsystem
-- (lives under `pandaria/crates/tavern-*`). Tavern is no longer a
-- separate consumer of Aspectus.
--
-- What this migration does:
-- 1. Delete all `scopes` rows whose name starts with `tavern:`
-- 2. (Best-effort) Mark the `service_tokens.project = 'tavern'` row
--    as revoked-by-removal. The row itself is left in place because
--    PostgreSQL enums cannot drop a value without recreating the type,
--    which is a destructive operation we'd rather defer.
-- 3. Document the deprecation in a comment.
--
-- The Rust `Project` enum (aspectus-core/src/project.rs) no longer
-- has a `Tavern` variant; this migration aligns the database with
-- the code. From 2026-06-21 onward, no new code can reference Tavern.
-- ============================================================

-- Phase 1: Delete Tavern scope rows
DELETE FROM scopes WHERE name LIKE 'tavern:%';

-- Phase 2: Document Tavern service token status
--
-- We do NOT DELETE the row or drop the enum value because:
--   - PostgreSQL enum values cannot be removed without recreating the
--     entire type (ALTER TYPE project DROP VALUE 'tavern' is rejected
--     by Postgres).
--   - Any existing api_keys.user_id / api_keys.tenant_id referencing
--     Tavern's data will keep working since the column is varchar.
--   - Recreating the enum would require a multi-step table rewrite
--     that we defer to a future maintenance window.
--
-- The Rust FromStr impl rejects 'tavern' as a valid Project, so even
-- if the DB row exists, new code paths cannot interact with it.
COMMENT ON COLUMN service_tokens.project IS
    'Per-project service token. Note: the ''tavern'' value (if present) is deprecated as of 2026-06-21 — Tavern has been merged into Pandaria. Existing rows are kept for backwards compatibility; new code cannot create tavern service tokens (the Rust Project enum no longer has a Tavern variant).';

-- Phase 3: Document scope removal
COMMENT ON TABLE scopes IS
    'Permission labels in project:resource:action format (ADR-006). Note: tavern:* scopes were removed 2026-06-21 (Tavern merged into Pandaria).';
