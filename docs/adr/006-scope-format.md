# ADR-006: Scope 格式 — `project:resource:action`

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#scope--role)

---

## Context

Aspectus 需要一种 scope（权限标签）格式来表达「谁可以对哪个项目的什么资源做什么操作」。这个格式需要：
1. 人类可读、自解释
2. 支持通配符（「Pandaria 的所有权限」）
3. 易于在 `/introspect` 响应中传递（一个字段即可）
4. 不与 OAuth2 标准 scope（`openid profile email`）冲突

## Decision

**Scope 格式：`{project}:{resource}:{action}`，空格分隔，支持 `*` 通配符。**

```
pandaria:session:create
pandaria:session:read
pandaria:agent:execute
tavern:workflow:run
constell:agent:*
tokencamp:token:consume
```

**通配符语义**：
- `pandaria:session:*` — Pandaria session 资源的所有操作（create, read, delete...）
- `pandaria:*:*` — Pandaria 项目的所有资源和操作
- 不支持 `pandaria:session:cr*`（部分通配）——`*` 只能匹配完整段

**匹配算法**：按 `:` 分段后逐段比较，`*` 匹配任意一段。

## Alternatives Considered

### Alternative A：Logto 的 Resource → Scope 模型

Logto 的 scope 模型是：
```
Resource (audience: "https://api.pandaria.io")
  ├── scope: "session:create"
  ├── scope: "session:read"
  └── scope: "agent:execute"
```

scope 属于 resource，不携带 project 信息。project 信息通过 OAuth2 `client_id` 推断。

**拒绝理由**：
- 需要同时传递 `resource indicator`（audience）和 `scope` 列表才能完整表达权限——自省响应变复杂
- Project（pandaria、tavern）本身就是第一等的权限边界，应该放在 scope 中显式表达
- Aspectus 不实现完整的 OAuth2 resource indicator 机制——太重量级

### Alternative B：反向 DNS 格式

```
com.pandaria.session.create
com.pandaria.agent.execute
```

**拒绝理由**：冗余冗长。生态项目名已经足够唯一（pandaria、tavern），不需要反向 DNS 前缀。`project:resource:action` 更简洁。

### Alternative C：短 UUID 或数字 ID

```
scope:47a1b2c3
```

**拒绝理由**：完全不人类可读。管理员和开发者需要能看懂 scope 含义。自解释的字符串格式对调试和审计至关重要。

### Alternative D：分层 scope（含子 scope）

```
pandaria
  pandaria:session
    pandaria:session:create
    pandaria:session:read
  pandaria:agent
    pandaria:agent:execute
```

（层级式——持有 `pandaria` 自动拥有所有下级 scope）

**拒绝理由**：隐式层级在审计时难以追溯「这个用户为什么有 `pandaria:session:create`——是因为被分配了 `pandaria` 还是显式分配了 `pandaria:session:create`？」。显式 `*` 通配符更透明。

## Consequences

**正面**：
- 自解释：看 scope 就知道权限范围
- 紧凑：自省响应中一个空格分隔的字符串字段即可传递所有 scope
- 通配符灵活：Role 定义可以用 `pandaria:*:*` 一次性覆盖整个项目
- 与生态项目命名直接对应

**负面**：
- 没有 OAuth2 resource indicator 的概念，各项目需要自行将 scope 映射到内部权限模型
- `*` 通配符的语义需要文档明确说明（避免误解为 regex）
- scope 格式是约定而非标准，新加入生态的项目需要遵循格式规范

**缓解措施**：
- Aspectus client library 提供 scope 解析和匹配的 utility 函数
- 在 AGENTS.md 和开发者文档中明确 scope 格式规范
- Phase 2+ 如果生态扩大到需要 resource indicator，可以考虑 scope 扩展为 `resource_indicator scope1 scope2` 的映射，但当前不做
