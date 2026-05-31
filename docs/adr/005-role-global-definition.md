# ADR-005: Role 为全局定义，含身份类型约束

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#scope--role)
> 参考：Logto 的 Role 模型（全局定义 + tenant 隔离）

---

## Context

Aspectus 需要 Role 机制来简化权限分配——将一组 scope 打包成命名集合（如 `agent-developer`），而不是每次手动选择数十个 scope。

核心问题：Role 是全局定义（跨所有 tenant 共享 Role 模板）还是 per-tenant 定义（每个 tenant 自己创建 Role）？

## Decision

**Role 全局定义，非 per-tenant。所有 tenant 共享同一套 Role 模板。**

```
Role: agent-developer
  role_type: user
  scopes: [pandaria:session:*, tavern:workflow:*, constell:agent:*]

Role: ci-deployer  
  role_type: service_account
  scopes: [pandaria:session:create, tavern:workflow:deploy]
```

**设计要点**：

| 属性 | 说明 |
|------|------|
| 全局定义 | Role 由 Aspectus 管理员/开发者定义，不是由 tenant 管理员自定义 |
| `role_type` 约束 | `user` / `service_account` / `both`，DB 层强制 |
| User 通过 Role 授权 | `users_roles` 表关联 User 与 Role |
| Service Account 授权 | **Phase 1**：scopes 直接绑定在 API Key 上，不使用 Role<br>**Phase 2+**：可选用 Role（`role_type = service_account \| both`），也可继续直接绑 scopes——两路并行 |
| tenant 无关 | Role 定义中不含 `tenant_id`，分配时通过 join 表限定 tenant |

## Alternatives Considered

### Alternative A：per-tenant Role（每个租户自定义）

**拒绝理由**：
- 生态项目（Pandaria、Tavern 等）的 scope 集合是固定的——`pandaria:session:*` 对所有 tenant 语义相同
- 允许 tenant 自定义 Role 意味着 scope 的「打包方式」可以不同，导致跨租户审计困难
- 引入 tenant 管理员维护 Role 的负担

### Alternative B：完全不用 Role，只分配 scope

**拒绝理由**：Scope 粒度过细（预计每个项目 5-20 个 scope，6 个项目 = 30-120 个 scope）。管理员无法每次手动选择。Role 是必要的聚合层。

### Alternative C：Hierarchical Role（Role 继承）

```
developer → pandaria-developer + tavern-reader
pandaria-developer → pandaria:session:* + pandaria:agent:*
```

**拒绝理由**：增加复杂度，MVP 阶段不需要。Role 的 scope 集合直接展开存储，简单清晰。如果未来 Role 爆炸（20+ 个），再考虑继承。

### 与 Logto 的对比

Logto 的 Role 模型：

| 维度 | Logto | Aspectus |
|------|-------|----------|
| Role 范围 | per-tenant（`tenant_id` 列在 roles 表上） | 全局定义 |
| `role_type` 枚举 | `User` / `MachineToMachine` | `user` / `service_account` / `both` |
| Scope 模型 | Scope 属于 Resource（API resource indicator） | Scope 格式为 `project:resource:action` |
| 组织级 Role | 有 `organization_roles` 表（二级 Role） | 无（单层 tenant，无 organization 概念） |

Logto 的 Role 是 per-tenant 的——每个租户可以在自己的 tenant 内创建自定义 Role。这是因为 Logto 是通用 IdP，不同租户的应用完全不同。

Aspectus 不同：所有 tenant 面对的是同一套生态项目（Pandaria、Tavern 等），scope 语义是全局一致的。因此全局 Role 定义更合适，减少 tenant 管理员的心智负担。

## Consequences

**正面**：
- Role 语义统一：「agent-developer」在所有 tenant 中含义相同，便于审计和治理
- 减少 tenant 管理员负担：不需要理解 scope 细节，直接选预定义 Role
- 简化 API：不需要 per-tenant Role CRUD

**负面**：
- 无法满足「租户 T1 的 agent-developer 应该有特殊限制」的定制需求
- 新增生态项目时（如新增一个 `chimera` 项目），需要全局更新所有相关 Role 的 scope

**缓解措施**：
- Service Account 可直接绑定 scope（绕过 Role），为需要精确控制的场景留了出口
- User 的 scope 也可在 Role 基础上叠加/裁剪（Phase 3 设计）
- 新增项目属于低频操作（生态项目 6 个，短期不会暴增）
- 如果未来确实需要 per-tenant Role，可以在 roles 表上添加 `tenant_id` 列（nullable），`NULL` = 全局 Role，非空 = tenant 自定义——但当前不做
