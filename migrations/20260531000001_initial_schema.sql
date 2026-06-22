-- ============================================================
-- Migration: 20260531000001_initial_schema.sql
-- 描述：创建所有版本需要的全部表。v0.2.0-v1.0.0 各版本逐步激活使用。
-- ============================================================

-- -----------------------------------------------------------
-- 枚举类型
-- -----------------------------------------------------------

-- 身份类型：区分人类用户和机器账号 (ADR-004)
create type identity_type as enum ('user', 'service_account');

-- 项目枚举：生态中的所有系统 (ADR-010)
--
-- History: Originally included 'tavern' as a separate ecosystem project.
-- As of 2026-06-21, Tavern has been merged into Pandaria as a subsystem.
-- The 'tavern' enum value is retained for backwards compatibility with
-- existing rows in service_tokens / api_keys / audit_logs; see migration
-- #15 (20260621000015_remove_tavern.sql) for details.
create type project as enum (
    'pandaria',
    'tavern',
    'emerald',
    'constell',
    'tokencamp',
    'heirloom'
);

-- 角色类型：约束 Role 可被赋予哪种身份 (ADR-004, ADR-005)
-- 参考 Logto 的 role_type 设计，扩展了 'both'
create type role_type as enum ('user', 'service_account', 'both');

-- 密码加密方法枚举 (参考 Logto)
-- v1.0.0 使用 argon2id，保留其他值为未来扩展
create type password_encryption_method as enum (
    'Argon2id'
);

-- -----------------------------------------------------------
-- v0.2.0 激活：核心业务表
-- -----------------------------------------------------------

-- 租户 (ADR-008: 单层多租户)
create table tenants (
    id          varchar(21) primary key,
    name        varchar(128) not null,
    quotas      jsonb not null default '{}',
    created_at  timestamptz not null default now()
);

-- 服务账号 (ADR-004)
create table service_accounts (
    id          varchar(21) primary key,
    tenant_id   varchar(21) not null references tenants(id)
                    on update cascade on delete cascade,
    label       varchar(128) not null,
    description text,
    expires_at  timestamptz,
    created_at  timestamptz not null default now()
);

create index service_accounts__tenant
    on service_accounts (tenant_id, id);

-- API Key (ADR-002)
create table api_keys (
    id              varchar(21) primary key,
    tenant_id       varchar(21) not null references tenants(id)
                        on update cascade on delete cascade,
    service_account_id varchar(21) not null references service_accounts(id)
                        on update cascade on delete cascade,
    project         project not null,
    key_hash        varchar(64) not null,
    key_prefix      varchar(32) not null,
    scopes          text[] not null default '{}',
    expires_at      timestamptz,
    revoked_at      timestamptz,
    created_at      timestamptz not null default now()
);

-- key_hash 唯一索引兼查找索引：覆盖 sha256(token) 等值查找
create unique index api_keys__key_hash
    on api_keys (key_hash);

create index api_keys__service_account
    on api_keys (tenant_id, service_account_id, project);

-- FK lookup 索引：CASCADE delete 时高效查找引用行
create index api_keys__service_account_id
    on api_keys (service_account_id);

-- 审计日志 (ADR-009)
create table audit_logs (
    id          varchar(21) primary key,
    tenant_id   varchar(21) not null references tenants(id)
                    on update cascade on delete cascade,
    actor_id    varchar(21) not null,
    actor_type  identity_type not null,
    action      varchar(64) not null,
    target_type varchar(32) not null,
    target_id   varchar(21) not null,
    metadata    jsonb not null default '{}',
    created_at  timestamptz not null default now()
);

create index audit_logs__tenant
    on audit_logs (tenant_id, created_at desc);

create index audit_logs__actor
    on audit_logs (tenant_id, actor_id, created_at desc);

create index audit_logs__action
    on audit_logs (tenant_id, action, created_at desc);

-- Service Token (ADR-011)
create table service_tokens (
    project     project not null primary key,
    token_hash  varchar(64) not null,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);

-- -----------------------------------------------------------
-- v0.3.0 激活：Scope 定义
-- -----------------------------------------------------------

-- Scope 定义 (ADR-006)
-- v0.3.0 写入种子数据，v0.2.0 此表为空
create table scopes (
    id      varchar(21) primary key,
    name    varchar(256) not null,
    description text,
    constraint scopes__name unique (name)
);

-- -----------------------------------------------------------
-- v1.0.0 激活：用户和角色
-- -----------------------------------------------------------

-- 用户 (ADR-004)
-- v1.0.0 开始使用，v0.1.0 建好避免后续 DDL 锁
create table users (
    id                          varchar(21) primary key,
    tenant_id                   varchar(21) not null references tenants(id)
                                    on update cascade on delete cascade,
    email                       varchar(256),
    password_hash               varchar(256),
    password_encryption_method  password_encryption_method default 'Argon2id',
    display_name                varchar(128),
    is_suspended                boolean not null default false,
    last_sign_in_at             timestamptz,
    created_at                  timestamptz not null default now(),
    updated_at                  timestamptz not null default now(),
    constraint users__email unique (tenant_id, email)
);

create index users__tenant
    on users (tenant_id, id);

-- 角色 (ADR-005)
create table roles (
    id          varchar(21) primary key,
    name        varchar(128) not null,
    description varchar(256),
    type        role_type not null default 'user',
    is_default  boolean not null default false,
    constraint roles__name unique (name)
);

create index roles__type
    on roles (type);

-- 角色-权限关联 (ADR-005)
create table roles_scopes (
    id          varchar(21) primary key,
    role_id     varchar(21) not null references roles(id)
                    on update cascade on delete cascade,
    scope_id    varchar(21) not null references scopes(id)
                    on update cascade on delete cascade,
    constraint roles_scopes__unique unique (role_id, scope_id)
);

-- 用户-角色关联 (ADR-004, ADR-005)
create table users_roles (
    id          varchar(21) primary key,
    user_id     varchar(21) not null references users(id)
                    on update cascade on delete cascade,
    role_id     varchar(21) not null references roles(id)
                    on update cascade on delete cascade,
    constraint users_roles__unique unique (user_id, role_id)
);

-- role_type 约束函数：确保赋予用户的 Role 类型匹配 (参考 Logto)
create function check_role_type(
    target_role_id  varchar(21),
    allowed_types   role_type[]
) returns boolean as $$
begin
    return (select type from roles where id = target_role_id) = any(allowed_types);
end;
$$ language plpgsql;

-- v1.0.0 激活时添加 users_roles 的 check constraint
-- alter table users_roles
--     add constraint users_roles__role_type
--     check (check_role_type(role_id, array['user', 'both']::role_type[]));
