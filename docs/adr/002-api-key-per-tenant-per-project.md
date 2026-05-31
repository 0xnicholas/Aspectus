# ADR-002: API Key — per-tenant、per-project scoped

> 状态：Accepted
> 日期：2026-05-29
> 来源：[AGENTS.md](../../AGENTS.md#adr-002api-key-是-per-tenantper-project-scoped)

---

## Context

Aspectus 需要支持长期凭证，用于服务间调用和 Agent SDK 集成。API Key 是生态项目（Pandaria、Tavern、Constell 等）访问 Aspectus 管控资源的主要凭证方式。

核心约束：一个租户可能使用多个生态项目。如果 API Key 是全局的，一旦泄露，所有项目全部暴露。

## Decision

API Key 不是全局的。每个 Key 绑定到一个 **`(tenant_id, project, scopes)` 三元组**。

```
API Key: pk_live_abc123
  tenant_id: org-acme
  project: pandaria
  scopes: [pandaria:session:create, pandaria:session:read]
  expires_at: 2027-01-01
  created_by: user-123
```

**设计要点**：

| 属性 | 说明 |
|------|------|
| 租户绑定 | Key 属于且仅属于一个 tenant |
| 项目绑定 | Key 被签发到单一生态项目（pandaria、tavern、constell 等） |
| scope 限制 | scopes 必须 ⊆ owner 的有效 scopes（Key 权限不能超出持有者） |
| 单向哈希 | 数据库只存 `sha256(key)`，原文仅在创建时返回一次 |
| 可吊销 | `revoked_at` 字段支持即时吊销 |
| key_prefix | UI 展示用（如 `pk_live_abc123`） |

## Alternatives Considered

### Alternative A：全局 API Key（不分项目）

**拒绝理由**：如果一个 Key 可以访问所有生态项目，Key 泄露的 blast radius 是整个生态。Per-project Key 将 blast radius 限制到单个项目。这与最小权限原则一致。

### Alternative B：per-user API Key（不绑定 project）

**拒绝理由**：同一个用户可能在多个项目中有不同权限。「Pandaria 管理员」和「Tavern 只读用户」应该是两个 Key，而不是一个万能 Key。

### Alternative C：用 OAuth2 Client Credentials 替代 API Key

**拒绝理由**：Client Credentials 需要先获取 access token（多一次网络往返），对于 Agent SDK 等场景不够便利。API Key 是自包含的长期凭证，一次创建即可直接使用。Phase 3 可以补充 Client Credentials 作为可选方案，但 API Key 仍然是 MVP 的首选。

### 与 Logto 的对比

Logto 使用 `personal_access_tokens` 表存储用户级 PAT（Personal Access Token），通过 `applications` 表（type = MachineToMachine）管理 M2M 凭证。Logto 的 PAT 不绑定到特定 resource/project——它是用户级的，scopes 由角色决定。

Aspectus 的 API Key 更细粒度：同时绑定 tenant、project 和 scopes 三个维度。这是因为生态项目的边界比 OAuth2 resource 更明确——Pandaria 和 Tavern 是不同的系统，有独立的安全域。

## Consequences

**正面**：
- Blast radius 最小化：泄露一个 Key 只影响一个项目的特定 scopes
- 权限粒度可控：不同项目使用不同 Key，不同 Key 有不同 scope
- 吊销精准：吊销 Key 不影响同一 identity 的其他 Key

**负面**：
- 管理复杂度增加：一个 Service Account 可能在 6 个项目各有 Key，总共 6 个 Key
- 创建 Key 时需要额外指定 project 参数

**缓解措施**：
- 管理 UI（Daypaw）按 project 分组展示 Key
- 批量操作 API 支持按 project 批量创建/吊销
