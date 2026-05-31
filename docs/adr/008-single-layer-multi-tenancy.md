# ADR-008: 单层多租户模型（vs Logto 的两层模型）

> 状态：Accepted
> 日期：2026-05-29
> 来源：[AGENTS.md](../../AGENTS.md#3-多租户是一等概念不是事后附加)

---

## Context

Aspectus 需要多租户隔离。`tenant_id` 是数据模型的顶层命名空间。

Logto 采用**两层多租户**模型：
- **第一层**：`tenant` — 顶层命名空间（对应企业/客户）
- **第二层**：`organization` — tenant 内的分组（对应部门/团队），有独立的 org-level RBAC

Aspectus 是否需要两层？

## Decision

**Aspectus 采用单层租户模型：只有 `tenant_id`，没有 `organization` 概念。**

```
Tenant (org-acme)
  ├── User alice
  ├── User bob
  ├── Service Account ci-pipeline
  └── API Keys...
```

所有资源（User、Service Account、API Key、AuditLog）通过 `tenant_id` 分区。

**不在 Aspectus 中做组织层级的原因**：

| 考量 | 结论 |
|------|------|
| Pandaria 生态的实际需求 | 当前客户是一家企业 = 一个 tenant。没有「企业内多个部门用同一个 Aspectus tenant 但需要隔离」的场景 |
| AGENTS.md 原则 2 | 认证与授权分离。组织级隔离是授权问题，不应在身份层解决 |
| AGENTS.md 非目标 | 「组织架构管理」明确列为非目标 |

## Alternatives Considered

### Alternative A：Logto 式两层模型（tenant + organization）

Logto 的 organization 提供：
- 独立的 org-level RBAC（`organization_roles`, `organization_scopes`）
- 成员邀请（`organization_invitations`）
- Just-in-time provisioning（SSO 登录时自动加入 org）
- 品牌定制（per-org sign-in 页面样式）

**拒绝理由**：
- Aspectus 的客户是 Pandaria 生态的运营方，不是 SaaS 多租户平台。不需要「tenant 的内部再分组」
- 如果 Pandaria 需要项目内多团队隔离，应该在 Pandaria 自己的数据模型中实现（如 Pandaria 的 workspace 概念），不应推到身份层
- 两层模型显著增加数据模型和 API 复杂度（6+ 张额外的 organization 表），与 MVP 范围冲突

### Alternative B：完全不做多租户（单 tenant）

**拒绝理由**：Pandaria 生态本身就是多租户的——不同的企业/客户使用同一个 Pandaria 实例，必须隔离。Aspectus 不使用外部的身份孤岛（Auth0/Keycloak），必须自己支持多租户。

### Alternative C：用 Role 模拟组织层级

```
Role: org-engineering
Role: org-marketing
```

**保留为未来选项**：如果未来确实需要组织级分组，可以通过 per-tenant Role（当前 ADR-005 决定 Role 为全局）或自定义数据字段（`custom_data JSONB`）实现轻量分组，不引入完整的 organization 数据模型。

## Consequences

**正面**：
- 数据模型简洁：所有表只需 `tenant_id` 一个分区键
- API 简洁：不需要 organization CRUD、org membership 管理、org-level RBAC
- 与 MVP 范围匹配：Phase 1 只需要隔离不同企业客户
- SQL 查询简单：`WHERE tenant_id = $1` 一条搞定

**负面**：
- 如果未来 Pandaria 需要「企业内的部门级隔离」（如：同一 tenant 下，A 部门的 API Key 不能访问 B 部门的 Pandaria session），Aspectus 无法原生支持
- 没有 organization 的 JIT provisioning 能力（Phase 3 User 场景下，SSO 登录后无法自动归属到子组）

**缓解措施**：
- 如果未来确实需要组织级分组：
  - **方案 1**：在 Aspectus 中添加轻量 `group` 概念（仅 group membership，不做 group-level RBAC）——比 Logto 的 organization 简单得多
  - **方案 2**：在 Heirloom 中做数据级隔离（更符合原则 2：认证与授权分离）
  - 当前不做，等到有明确需求时再做 ADR
