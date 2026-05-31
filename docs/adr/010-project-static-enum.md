# ADR-010: Project 为静态枚举（非动态注册）

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#project)

---

## Context

Aspectus 需要知道「这个 API Key 是给 Pandaria 用的还是 Tavern 用的」。生态中的项目集合是固定的还是动态的？

## Decision

**Project 是静态 enum，硬编码在代码中。新项目加入生态需要代码变更 + migration + 部署。**

```rust
// Rust enum
pub enum Project {
    Pandaria,
    Tavern,
    Emerald,
    Constell,
    Tokencamp,
    Heirloom,
}
```

当前定义的生态项目：
- `pandaria` — 多 Agent 协作平台
- `tavern` — Workflow 编排引擎
- `emerald` — 记忆系统
- `constell` — Agent 市场
- `tokencamp` — LLM Token 网关
- `heirloom` — 数据权限与治理

每个 Project 持有**恰好一个 Service Token**，用于调用 `/introspect`。

## Alternatives Considered

### Alternative A：动态注册（类似 Logto 的 applications 表）

Logto 的 applications 表允许运行时创建新应用（SPA、Native、M2M 等），有完整的 CRUD。

**拒绝理由**：
- Aspectus 的 Project 代表生态中的**系统**，不是 OAuth2 应用。生态项目的数量是低频变化的——6 个系统，不会每周新增一个
- 如果允许动态注册，任何人都可以注册一个新 Project 并创建 API Key，破坏「生态中只有这些系统是合法的」假设
- 每个 Project 需要定制的 scope 前缀（如 `pandaria:session:create`）。动态注册意味着 scope 前缀也是动态的——scope 失去语义一致性

### Alternative B：per-project Service Token（动态但受限）

每个 Project 在初始化时调用 Aspectus 注册自己，获得 Service Token。项目数有限但不硬编码。

**拒绝理由**：仍然是动态注册，只是加了个审批步骤。语义上，Project 是生态的一部分——当 Pandaria 加入生态时，它需要的是 Aspectus 的代码变更（添加 scope 定义、添加 Service Token）而不是运行时注册。运行时注册适合「未知的第三方应用」，而 Aspectus 面向的是已知的生态项目。

### Alternative C：不用 Project 概念，只用 scope 前缀

```
pandaria:session:create — 隐含了 project = pandaria
```

**拒绝理由**：API Key 需要显式绑定 project，用于 blast radius 控制（ADR-002）。仅靠 scope 前缀推断 project 不可靠——如果一个 Key 有 `pandaria:session:create tavern:workflow:run`，它属于哪个 project？应该是两个不同的 Key。

### 与 Logto 的对比

Logto 的 `applications` 表是动态的——因为 Logto 是通用 IdP，无法预测用户会注册什么类型的应用（SPA、Native 等）。Aspectus 不同——生态中的项目是 Aspectus 开发者已知的、固定的集合。这是一个「生态内 vs 对第三方开放」的根本差异。

## Consequences

**正面**：
- 类型安全：编译器保证只有合法 project 值存在
- scope 前缀与 project enum 一致，自动验证
- 简化 API Key 创建 API：project 字段可以用 enum 而非 free-text
- 新 project 加入是经过审查的代码变更（code review），保证了生态的一体性

**负面**：
- 新增生态项目需要 code change + migration + deploy，速度比运行时注册慢
- 如果未来需要支持「第三方开发者创建自己的 project」，架构不支持

**缓解措施**：
- 新增生态项目是极低频操作。如果未来频率增加（生态快速扩展），可以考虑将 enum 从硬编码改为配置文件/DB 表
- 但当前 6 个项目，短期无扩展计划。YAGNI（You Ain't Gonna Need It）
