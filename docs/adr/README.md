# Architecture Decision Records (ADR)

Aspectus 项目的架构决策记录。每条 ADR 记录一个重要的架构决策，包含 context、decision、alternatives considered 和 consequences。

## ADR 索引

| 编号 | 标题 | 状态 | 参考/对比 Logto |
|------|------|------|:--:|
| [001](./001-token-introspection-rfc7662.md) | Token 自省采用 RFC 7662 语义 | Accepted | ✅ Logto 也实现 RFC 7662，但作为 OIDC 能力的一部分 |
| [002](./002-api-key-per-tenant-per-project.md) | API Key — per-tenant、per-project scoped | Accepted | ✅ Logto 的 PAT 是用户级，不绑定 project |
| [003](./003-quota-config-vs-enforcement.md) | 配额配置与执行分离 | Accepted | ❌ Logto 无配额概念 |
| [004](./004-user-vs-service-account-role-type.md) | User 与 Service Account 分离 + `role_type` 约束 | Accepted | ✅ 借鉴 Logto 的 `role_type` enum，扩展了 `'both'` |
| [005](./005-role-global-definition.md) | Role 为全局定义，含身份类型约束 | Accepted | ✅ Logto 的 Role 是 per-tenant，Aspectus 不同 |
| [006](./006-scope-format.md) | Scope 格式 — `project:resource:action` | Accepted | ✅ Logto 用 Resource→Scope 模型，Aspectus 不同 |
| [007](./007-hybrid-token-model.md) | Hybrid Token 模型 — JWT + Opaque + API Key | Accepted | ✅ Logto 统一 OAuth2 token，不区分高频/低频场景 |
| [008](./008-single-layer-multi-tenancy.md) | 单层多租户模型（vs Logto 的两层） | Accepted | ✅ Logto 有 tenant + organization 两层 |
| [009](./009-audit-log-structured-vs-jsonb.md) | 审计日志 — 结构化列 vs JSONB | Accepted | ✅ Logto 用 JSONB，Aspectus 选结构化列 |
| [010](./010-project-static-enum.md) | Project 为静态枚举（非动态注册） | Accepted | ✅ Logto 的 applications 是动态的 |
| [011](./011-service-token-separate-auth.md) | Service Token — 独立的内部认证层 | Accepted | ❌ Logto 无 Service Token 概念（用 Client Credentials） |
| [012](./012-technology-stack.md) | 技术选型 — Rust/axum/PostgreSQL/Redis | Accepted | ✅ Logto 是 TypeScript/Koa/PostgreSQL |
| [013](./013-emerald-entity-id-mapping.md) | Emerald entity_id 使用 `tenant_id:user_id` 复合映射 | Accepted | — |
| [014](./014-error-handling-rfc7807.md) | API 错误响应采用 RFC 7807 Problem Details | Accepted | — |
| [015](./015-id-format-short-id.md) | 实体 ID 格式 — 短 ID (varchar/21) | Accepted | ✅ Logto 也使用 varchar(21) 短 ID |
| [016](./016-unified-login-ux.md) | 跨租户登录路由与登录响应增强 | Accepted | ✅ 类似 Google/GitHub 的两步登录（邮箱 → 选租户 → 密码） |

## 设计参考源

- **主要参考**：[Logto](https://github.com/logto-io/logto) — 开源 IdP（OIDC + OAuth 2.1 + SAML + RBAC + 多租户）
- **项目上下文**：[AGENTS.md](../../AGENTS.md) — Aspectus 总体架构和原则
- **概念细化**：[concepts-and-architecture-design.md](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md) — brainstorming 后的概念细化

## 实施参考

- **[消费者接入指南](../consumer-integration.md)** — 面向 Pandaria / Constell / Tokencamp / Heirloom / Emerald 等生态项目的工程师，说明如何把 Aspectus 接入项目网关。涵盖完整中间件代码、错误处理矩阵、本地 JWT 验签优化、灰度与回滚。引用了 ADR-001 / ADR-002 / ADR-003 / ADR-011。

> **注意 2026-06-21**：Tavern 已合并入 Pandaria 作为子系统，本指南及生态消费者列表中已移除 Tavern。历史 ADR 文件（001–016）中提及 Tavern 的内容是决策时刻的记录，未作修改（历史决策不动）。

## ADR 格式

每条 ADR 遵循以下结构：

```
# ADR-NNN: 决策标题
> 状态 | 日期 | 来源

## Context
## Decision
## Alternatives Considered
## Consequences
```

## 状态说明

| 状态 | 含义 |
|------|------|
| Proposed | 提案中，尚未确定 |
| Accepted | 已采纳，当前有效 |
| Deprecated | 已废弃（但未完全移除） |
| Superseded | 被新的 ADR 取代（标注取代它的 ADR 编号） |
