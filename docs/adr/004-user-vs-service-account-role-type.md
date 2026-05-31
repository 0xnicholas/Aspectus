# ADR-004: User 与 Service Account 分离 + `role_type` 约束

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#user人类用户)
> 参考：Logto 的 `role_type` enum 设计

---

## Context

Aspectus 需要管理两种不同的身份：
1. **人类用户**：通过 OAuth2 Authorization Code 登录，有 email/password，通过 Role 获得权限
2. **机器/服务账号**：通过 API Key 认证，无 email/password，直接绑定 scopes

这两种身份在认证方式、授权模型、生命周期、审计语义上完全不同。如果放在同一张 `users` 表中会导致：
- 大量 null 字段（服务账号无 email、无 password）
- 审计日志角色模糊（「谁创建了 API Key」——是人还是 CI pipeline？）
- 安全边界模糊（密码重置流程 vs API Key 轮转流程）

## Decision

**User 和 Service Account 是两个独立概念、独立表、独立 API。**

| 维度 | User | Service Account |
|------|------|-----------------|
| 身份属性 | email, password_hash, display_name, avatar_url | label, description |
| 认证方式 | OAuth2 Authorization Code (Phase 3) | API Key (Phase 1) |
| 授权模型 | 通过 Role 获得 scopes | **Phase 1**：直接绑定 scopes（不使用 Role）<br>**Phase 2+**：可选用 Role（`role_type = service_account \| both`），也可继续直接绑 scopes |
| 生命周期 | 手动创建/禁用 | 可设 expires_at，过期后关联 Key 自动失效 |
| 审计语义 | actor = 具体的人，可追责 | actor = 系统/流水线 |

**DB 设计**：两张独立表，共用 `identity_type` 枚举区分。

```sql
create type identity_type as enum ('user', 'service_account');
```

自省响应中通过 `identity_type` 字段告知调用方当前 token 属于哪种身份。

## Role Type 约束（参考 Logto）

借鉴 Logto 的 `role_type` enum 设计：

```sql
create type role_type as enum ('user', 'service_account', 'both');
```

- Roles 标记适用的身份类型
- DB 层通过 check constraint 强制：
  - User 只能被赋予 `role_type = 'user'` 或 `'both'` 的 Role
  - Service Account 同理
- 防止运维错误（把「agent-developer」角色赋给 CI pipeline）

**Logto 的实现**：

```sql
-- Logto 的 role_type 定义（我们采纳的设计）
create type role_type as enum ('User', 'MachineToMachine');

-- Logto 的 check constraint（我们采纳的模式）
create function check_role_type(role_id varchar, target_type role_type) 
  returns boolean as $$
begin
  return (select type from roles where id = role_id) = target_type;
end; $$ language plpgsql;

constraint users_roles__role_type
  check (public.check_role_type(role_id, 'User'))
```

Aspectus 在 Logto 基础上扩展了 `'both'` 类型，允许某些 Role（如 `tenant-admin`）同时适用于人类用户和服务账号。

> **Phase 1 说明**：Phase 1 MVP 仅包含 Service Account + API Key。`role_type = 'service_account' | 'both'` 的 Role 定义在 DB schema 中已存在，但 Service Account 在 Phase 1 不使用 Role——scopes 直接绑定在 API Key 上。Role 对 SA 的支持在 Phase 2+ 启用，届时 SA 可选用 Role 获得 scopes 或继续直接绑定 scopes（两路并行，灵活切换）。

## Alternatives Considered

### Alternative A：单表 + `is_service_account` flag

**拒绝理由**：由 null 字段、语义歧义、安全边界模糊导致——如 Context 中所述。

### Alternative B：单表 + 继承/多态（Single Table Inheritance）

**拒绝理由**：PostgreSQL 无原生继承支持的表查询。用 JSONB 做多态字段失去类型安全。不如两张独立表清晰。

### Alternative C：完全不区分，所有身份统一

**拒绝理由**：与 Phase 1 MVP 范围冲突——Phase 1 只需要 API Key（服务账号场景），不需要 OAuth2 和 password。强行统一会导致 Phase 1 过度设计。

## Consequences

**正面**：
- 语义清晰：审计日志中一眼可辨是「人做的」还是「系统做的」
- 安全隔离：Service Account 的 password 字段根本不存在，不会误用
- Phase 1 MVP 简单：只需实现 Service Account + API Key，不涉及 OAuth2

**负面**：
- 两张表有重复模式（id、tenant_id、created_at 等）
- `/introspect` 响应需要 `identity_type` 字段，调用方需要条件分支
**缓解措施**：
- 共享 base trait / interface（Rust trait: `Identity`），减少代码重复
- 自省响应统一为 `user_id` 字段（无论 User 还是 Service Account），`identity_type` 只是提示性质
- Phase 2+ 的 Service Account Role 支持已通过 `role_type = 'service_account' | 'both'` 在 DB schema 中预留
