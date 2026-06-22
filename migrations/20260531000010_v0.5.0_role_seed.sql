-- ============================================================
-- Migration: 20260531000010_v0.5.0_role_seed.sql
-- 描述：v0.5.0 — Role 种子数据 + roles_scopes 映射
-- ============================================================

-- Roles
INSERT INTO roles (id, name, description, type, is_default) VALUES
('role_tenant_admin', 'tenant-admin', 'Full tenant management', 'both', false),
('role_agent_dev', 'agent-developer', 'Agent development access', 'user', true),
('role_agent_op', 'agent-operator', 'Agent operation access', 'user', false),
('role_ci_deployer', 'ci-deployer', 'CI/CD deployment', 'service_account', false)
ON CONFLICT (name) DO NOTHING;

-- tenant-admin: all scopes
INSERT INTO roles_scopes (id, role_id, scope_id)
SELECT LEFT('rsa_' || s.id, 21), 'role_tenant_admin', s.id FROM scopes s
ON CONFLICT (role_id, scope_id) DO NOTHING;

-- agent-developer
INSERT INTO roles_scopes (id, role_id, scope_id) VALUES
('rsdev1','role_agent_dev','sc_pa_session_create'),('rsdev2','role_agent_dev','sc_pa_session_read'),
('rsdev3','role_agent_dev','sc_pa_session_delete'),('rsdev4','role_agent_dev','sc_pa_session_manage'),
('rsdev5','role_agent_dev','sc_pa_agent_execute'),('rsdev6','role_agent_dev','sc_pa_agent_manage'),
('rsdev9','role_agent_dev','sc_co_agent_publish')
ON CONFLICT (role_id, scope_id) DO NOTHING;

-- agent-operator
INSERT INTO roles_scopes (id, role_id, scope_id) VALUES
('rsop1','role_agent_op','sc_pa_session_read'),
('rsop2','role_agent_op','sc_pa_agent_execute')
ON CONFLICT (role_id, scope_id) DO NOTHING;

-- ci-deployer
INSERT INTO roles_scopes (id, role_id, scope_id) VALUES
('rsci1','role_ci_deployer','sc_pa_session_create')
ON CONFLICT (role_id, scope_id) DO NOTHING;
