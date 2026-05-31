# ADR-003: 配额配置与执行分离

> 状态：Accepted
> 日期：2026-05-29（更新于 2026-05-31）
> 来源：[AGENTS.md](../../AGENTS.md#adr-003租户配额在-aspectus-管理各项目执行)
> 
> **注意**：本 ADR 反映了 2026-05-29 概念文档讨论后的更新设计。原始 AGENTS.md ADR-003 中自省响应包含 `used` 字段，现已改为仅返回 `limit`。详见 [概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#5-与-agentsmd-的差异)。

---

## Context

Pandaria 生态中，不同租户有不同的资源上限：LLM token 消耗量（Tokencamp）、并发 session 数（Pandaria）、workflow 执行频率（Tavern）等。需要一个统一的配额管理机制。

关键约束：Aspectus 是身份服务，不是资源计量服务。它不知道「租户 T 这个月已经消耗了多少 LLM token」，因为它不参与 LLM 调用的实际执行。

## Decision

**Aspectus 只存储配额的 limit，不追踪使用量（used）。各项目独立追踪使用量、独立执行限流判断。**

自省响应中携带配额配置（仅 limit）：

```json
{
  "active": true,
  "tenant_id": "org-acme",
  "quotas": {
    "pandaria": { "max_concurrent_sessions": 50 },
    "tokencamp": { "monthly_tokens": 10000000 }
  }
}
```

**职责边界**：

| 职责 | 谁管 | 说明 |
|------|------|------|
| 配额配置存储 | Aspectus | limit 的值存在 Aspectus |
| 配额配置查询 | 各项目 | 通过 `/introspect` 或定期同步获取 |
| 使用量追踪 | 各项目 | Tokencamp 计 token 消耗，Pandaria 计 session 数 |
| 限流执行 | 各项目 | 项目本地判断 `used < limit` |
| 配额更新 | Aspectus 管理 API | `PUT /tenants/{id}/quotas` |

## Alternatives Considered

### Alternative A：Aspectus 集中追踪使用量并执行限流

**拒绝理由**：Aspectus 会成为所有请求的热路径瓶颈。每次 LLM 调用、每个 session 创建都需要回调 Aspectus 更新计数器。这不仅引入额外延迟，还让 Aspectus 承担了不属于身份服务的职责。配额的语义是项目相关的（Tokencamp 知道什么是「一个 token」，Pandaria 知道什么是「一个 session」），Aspectus 不应理解这些语义。

### Alternative B：不使用 Aspectus，各项目自管配额

**拒绝理由**：回到当前生态的碎片化状态。如果每个项目各自存储租户配额，租户管理员需要在多个地方维护配置，且无法获得「租户 T 的配额全景」。

### Alternative C：Aspectus 返回 used + limit，但不做限流执行

**拒绝理由**：Aspectus 需要各项目频繁上报使用量，引入反向依赖（项目 → Aspectus 的写入路径），违背「Aspectus 是只读热路径」的设计原则。

### 与 Logto 的对比

Logto **没有配额概念**。Logto 关注的是认证和授权（谁可以访问什么），不关注用量限制（访问多少）。这是 Aspectus 与 Logto 的核心差异化能力——Aspectus 不仅是身份层，也是生态的租户治理层。

## Consequences

**正面**：
- 自省热路径极简：只需从缓存/DB 读取配额配置，无需写入
- 各项目独立闭环：项目有自己的计量逻辑（如 Tokencamp 的 tokenizer），Aspectus 不需要理解
- 与现有 `AGENTS.md` 原则 2（认证与授权分离）一致

**负面**：
- 配额执行的一致性依赖各项目正确实现限流逻辑
- 无法在 Aspectus 侧看到全局使用量仪表盘（需各项目自行暴露 metrics）
- 配额更新需要各项目刷新本地缓存（非实时）

**缓解措施**：
- Aspectus 提供 Rust client library 封装配额读取和缓存刷新逻辑
- 配额配置变更时通过 webhook 通知各项目刷新（Phase 2+）
- 各项目通过 OpenTelemetry metrics 暴露使用量，由可观测性平台聚合
