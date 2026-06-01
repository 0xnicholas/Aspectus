-- ============================================================
-- Migration: 20260531000006_v0.5.0_user_role.sql
-- 描述：v0.5.0 — api_keys 扩展 + role_type 约束
-- ============================================================

-- api_keys: support both user_id and service_account_id owners
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS user_id VARCHAR(21) REFERENCES users(id);
ALTER TABLE api_keys ALTER COLUMN service_account_id DROP NOT NULL;

DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'api_keys__one_owner') THEN
    ALTER TABLE api_keys ADD CONSTRAINT api_keys__one_owner
      CHECK ((user_id IS NOT NULL AND service_account_id IS NULL) OR
             (user_id IS NULL AND service_account_id IS NOT NULL));
  END IF;
END $$;

-- Activate role_type constraint (function created in 001)
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_roles__role_type') THEN
    ALTER TABLE users_roles ADD CONSTRAINT users_roles__role_type
      CHECK (check_role_type(role_id, ARRAY['user','both']::role_type[]));
  END IF;
END $$;
