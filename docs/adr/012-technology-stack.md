# ADR-012: 技术选型 — Rust/axum/PostgreSQL/Redis

> 状态：Accepted
> 日期：2026-05-29
> 来源：[AGENTS.md](../../AGENTS.md#技术选型)

---

## Context

Aspectus 需要选择技术栈。核心约束：
1. 自省端点 p95 < 5ms（含 Redis 缓存命中）
2. 生态一致性（Pandaria、Tavern 都是 Rust + axum）
3. 关系型数据模型（tenant、user、api_key 等）
4. 自省结果需要缓存
5. 团队维护成本

## Decision

| 维度 | 选型 | 理由 |
|------|------|------|
| 语言 | **Rust** | 零成本抽象 + 无 GC，p95 延迟可控；与 Pandaria/Tavern/Pawbun 一致 |
| HTTP 框架 | **axum** | 与 Tavern 一致，tokio 生态，类型安全的路由和 extractors |
| 数据库 | **PostgreSQL** | 租户/用户/API Key 是关系型数据，需要 ACID、FK 约束、行级安全 |
| 缓存 | **Redis** | 自省结果缓存、JWT 吊销集合、Service Token 验证缓存 |
| Token 签名 | **RSA-256 (RS256)** | 非对称签名，public key 可分发各项目本地验签 |
| 密码哈希 | **argon2id** | 抗 GPU/ASIC 暴力破解，OWASP 推荐（Phase 3 User 场景） |
| API Key 哈希 | **SHA-256** | 不可逆，仅用于查找匹配，无需抗暴力破解（128-bit entropy key） |
| 迁移 | **sqlx** 或 **refinery** | Rust 原生 migration 工具，类型安全 |

## Alternatives Considered

### 语言对比

| 语言 | 理由拒绝 |
|------|---------|
| TypeScript/Node.js (Logto 的选择) | GC pause 影响 p95 延迟；与生态其他 Rust 项目不一致；自省热路径需要稳定低延迟 |
| Go | 与生态不一致（学习和维护两套语言）；GC 虽然比 Node 好但仍逊于 Rust |
| Python | GIL 限制并发；不适合高性能自省端点 |

### 数据库对比

| 数据库 | 理由拒绝 |
|------|---------|
| SQLite | 不支持高并发写入（API Key 创建、审计日志）；无原生连接池适合单机场景 |
| CockroachDB | 过度设计——Aspectus 不需要全球分布式部署 |
| DynamoDB | schema-less，无法表达 FK 约束；租户隔离靠应用层而非 DB 层 |

### 为什么不用 Logto 作为依赖？

Logto 是完整 IdP（Node.js + OIDC + SAML + SSO），如果直接使用：
- 引入 80+ 张表、30+ 个 package 的复杂度——但 Aspectus 只需要其中 ~5 张表
- 自省端点必须经过 Logto 的完整 OAuth2 中间件链，p95 延迟难以优化到 < 5ms
- Logto 不做配额管理——这是 Aspectus 的核心差异化能力，必须自己实现
- 部署和维护一个 Logto 实例 vs 部署 Aspectus——前者需要完整 Node.js 生态（pnpm、ESLint、Jest、30+ connectors），后者只需一个 Rust binary + PostgreSQL + Redis

**Logto 是优秀的参考设计**（我们采纳了其 role_type 约束），但作为运行时依赖则不合适。

## Consequences

**正面**：
- 与 Pandaria 生态技术栈一致：共享 Rust 类型定义（如 `scope` 解析库）、部署镜像、CI/CD 流程
- 性能可预测：Rust 无 GC，p95 延迟稳定可控
- 编译期安全：API Key 哈希、密码值等敏感数据通过类型系统防止误用（如 `#[derive(Serialize)]` 排除敏感字段）

**负面**：
- Rust 开发效率低于 TypeScript——但 Aspectus 体量小（Phase 1 ~5 个，最终 ~15 个 API endpoint），开发速度不是瓶颈
- 运行时调试不如 Node.js 方便（没有 REPL、热重载有限）
- Rust 招聘难度高于 TypeScript——但 Pandaria 生态团队已经是 Rust 团队

**缓解措施**：
- 使用 cargo-watch 或 bacon 提供自动重编译
- 丰富的 tracing/logging（tokio-rs/tracing crate）
- Phase 1 MVP 范围小（~5 个 API），开发周期可控
