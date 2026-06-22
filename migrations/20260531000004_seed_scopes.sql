-- ============================================================
-- Migration: 20260531000004_seed_scopes.sql
-- 描述：为所有 6 个 Project 写入 scope 种子数据 (v0.3.0)
-- ============================================================

-- Pandaria
INSERT INTO scopes (id, name, description) VALUES
('sc_pa_session_create', 'pandaria:session:create', 'Create a new agent session'),
('sc_pa_session_read',   'pandaria:session:read',   'Read session details'),
('sc_pa_session_delete', 'pandaria:session:delete', 'Delete a session'),
('sc_pa_session_manage', 'pandaria:session:manage', 'Manage session lifecycle'),
('sc_pa_agent_execute',  'pandaria:agent:execute',  'Execute an agent task'),
('sc_pa_agent_manage',   'pandaria:agent:manage',   'Register/configure agents')
ON CONFLICT (name) DO NOTHING;

-- Tavern removed 2026-06-21 (merged into Pandaria). See migration
-- #15 (20260621000015_remove_tavern.sql) which DELETEs any existing
-- tavern:* scope rows.
-- Constell
INSERT INTO scopes (id, name, description) VALUES
('sc_co_agent_publish', 'constell:agent:publish', 'Publish an agent to the marketplace'),
('sc_co_agent_install', 'constell:agent:install', 'Install an agent from marketplace'),
('sc_co_agent_read',    'constell:agent:read',    'Browse/read agent details')
ON CONFLICT (name) DO NOTHING;

-- Tokencamp
INSERT INTO scopes (id, name, description) VALUES
('sc_tk_token_consume', 'tokencamp:token:consume', 'Consume LLM tokens'),
('sc_tk_token_meter',   'tokencamp:token:meter',   'Read token usage metrics'),
('sc_tk_token_manage',  'tokencamp:token:manage',  'Configure token limits')
ON CONFLICT (name) DO NOTHING;

-- Heirloom
INSERT INTO scopes (id, name, description) VALUES
('sc_he_resource_read',  'heirloom:resource:read',  'Read resource metadata'),
('sc_he_policy_read',    'heirloom:policy:read',    'Read access policies'),
('sc_he_policy_manage',  'heirloom:policy:manage',  'Create/update/delete policies')
ON CONFLICT (name) DO NOTHING;
