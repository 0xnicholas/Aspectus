# Aspectus vs Logto — 全面对比

> 日期：2026-06-01
> Aspectus 当前版本：v0.8.0
> Logto 参考版本：latest (2026)

---

## 1. 定位与目标

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 定位 | 通用开源 IdP（对标 Auth0/Keycloak） | Pandaria 生态专用身份层 |
| 目标用户 | SaaS 和 AI 应用开发者 | Pandaria/Tavern/Constell 等内部项目 |
| 核心理念 | 全功能开箱即用 | 够用就好，生态优先 |
| 非目标 | — | SAML、LDAP、Social Login、MFA UI、组织架构 |

---

## 2. 架构对比

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 语言 | TypeScript | Rust |
| HTTP 框架 | Koa | axum |
| 数据库 | PostgreSQL | PostgreSQL |
| 缓存 | — (无内置 Redis) | Redis（自省缓存 + 吊销集） |
| 仓库 | pnpm monorepo（18+ packages） | Cargo workspace (4 crates) |
| 组件 | Core + Console (SPA) + Experience (SPA) + 30+ connectors | 单一 binary (aspectus-server) |
| 前端 | React Admin SPA + Sign-in SPA | 无（管理 UI 由 Daypaw 提供） |
| 部署 | Node.js 进程 + pnpm + PostgreSQL | 单一 Rust binary + PostgreSQL + Redis |
| Docker | 官方 image | 自建 Dockerfile（多阶段构建） |
| 表数量 | 80+ | 13 |
| 源文件 | 1000+ | 50+ |
| 代码量 | ~100K+ 行 TypeScript | ~3,500 行 Rust |

---

## 3. 数据模型对比

### Tenant 模型

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 层级 | **两层**：tenant + organization | **单层**：tenant |
| 租户内分组 | Organization（独立 RBAC、邀请、JIT provisioning） | 无 |
| 决定理由 | SaaS 多租户平台，企业内需要部门隔离 | 生态客户 = 企业，不需要二级分组 |

### 身份模型

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 人类用户 | `users` 表（username, email, phone, avatar, identities JSONB） | `users` 表（email, display_name, argon2id） |
| 机器身份 | `applications` type=`MachineToMachine` | `service_accounts` 表（独立实体） |
| 区分方式 | application type 枚举 | 独立表 + `identity_type` 枚举 |
| 借鉴点 | — | `role_type` 约束借鉴自 Logto，扩展了 `'both'` |

### 应用/Project 模型

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 概念 | `applications`（OAuth2 client） | `Project`（生态项目） |
| 注册方式 | 动态注册（运行时 CRUD） | 静态 enum（硬编码 6 个） |
| 类型 | Native, SPA, Traditional, M2M, Protected, SAML | pandaria, tavern, emerald, constell, tokencamp, heirloom |
| 决定理由 | 通用 IdP，无法预知应用类型 | 生态项目固定已知 |

---

## 4. Token 模型对比

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 标准 | 完整 OIDC Provider | RFC 7662 Token Introspection（子集） |
| Access Token | Opaque 或 JWT（全局配置） | **Hybrid**：API Key + JWT + Opaque，按场景选择 |
| API Key | Personal Access Token（用户级） | API Key per-tenant per-project scoped |
| JWT 签名 | OIDC 标准 | RS256（v0.4） |
| Token 吊销 | OIDC revocation endpoint | Redis SETEX（per-key）+ DB revoked_at |
| Service Token | 无（用 Client Credentials） | **独立概念**：每个 Project 一个，仅用于 /introspect 认证 |
| 自省端点 | `/introspect`（OIDC 标准一部分） | `/introspect`（核心热路径，p95 < 5ms） |

---

## 5. RBAC 对比

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 模型 | Resource → Scope → Role（三层） | Scope → Role（两层） |
| Scope 格式 | per-Resource scope name | `project:resource:action`（含 project 前缀） |
| Resource | API resource indicator（audience） | 无独立 Resource 概念（scope 自带 project） |
| Role 范围 | **per-tenant**（`roles.tenant_id`） | **全局定义**（所有 tenant 共享） |
| 组织级 Role | `organization_roles`（二级 RBAC） | 无 |
| role_type | `User` / `MachineToMachine` | `user` / `service_account` / `both` |
| 约束 | DB check constraint | DB check constraint（借鉴 Logto） |
| 默认 Role | `is_default` 字段 | `is_default` 字段（创建用户自动分配） |

---

## 6. 功能矩阵

| 功能 | Logto | Aspectus | 备注 |
|------|:--:|:--:|------|
| OIDC / OAuth 2.1 | ✅ 完整 | ⚠️ Authorize Code 子集 | Aspectus 不实现完整 OIDC |
| SAML | ✅ | ❌ | 非目标 |
| Social Login | ✅ 30+ | ❌ | 非目标 |
| SSO | ✅ | ❌ | 非目标 |
| MFA | ✅ | ❌ | 非 MVP |
| Multi-tenancy | ✅ 两层 | ✅ 单层 | |
| RBAC | ✅ 三层 | ✅ 两层 | |
| API Key | ✅ PAT | ✅ per-tenant/project | Aspectus 更细粒度 |
| Token Introspection | ✅ | ✅ 核心热路径 | |
| JWT | ✅ OIDC | ✅ RS256 | |
| 配额管理 | ❌ | ✅ | Aspectus 独有 |
| 审计日志 | ✅ JSONB | ✅ 结构化列 | |
| 管理 UI | ✅ Console (React SPA) | ❌（由 Daypaw 提供） | |
| Sign-in UI | ✅ Experience (SPA) | ❌（由各项目自建） | |
| 密码认证 | ✅ | ✅ argon2id | |
| OAuth2 Client | ✅ 动态注册 | ✅ CRUD + redirect_uri 校验 | |
| Refresh Token | ✅ | ✅ 轮转 | |
| JWKS 端点 | ✅ | ✅ | |
| Metrics | 外部（APM） | ✅ Prometheus 端点 | |
| OpenAPI 文档 | ✅ Swagger | ✅ OpenAPI 3.0 YAML | |

---

## 7. API 端点对比

| Logto 端点 | Aspectus 端点 | 对比 |
|-----------|-------------|------|
| `POST /oidc/token` | `POST /oauth/token` | Aspectus 支持 authorization_code + refresh_token |
| `POST /oidc/introspect` | `POST /introspect` | 相同 RFC 7662 语义 |
| `GET /.well-known/openid-configuration` | — | Aspectus 不做 OIDC discovery |
| `POST /api/applications` | `POST /clients` | 相似 |
| `POST /api/users` | `POST /users` | 相似 |
| `POST /api/roles` | — (seed only) | Aspectus Role 全局定义，不运行时创建 |
| `POST /api/resources` | — | Aspectus 无 Resource 概念 |
| `GET /api/logs` | — (SQL 直查 audit_logs) | |
| — | `POST /tenants` | Logto 有 tenant 管理但 API 不同 |
| — | `PUT /tenants/{id}/quotas` | Aspectus 独有 |
| — | `POST /token/revoke` | Aspectus 统一吊销端点 |

---

## 8. 性能特性

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 自省延迟 | ~5-15ms（Node.js + OIDC 中间件链） | **p95 < 5ms**（Redis 命中），实测 avg 1.2ms |
| 冷启动 | Node.js 进程启动 + 连接池 | Rust binary 启动 + PG pool (min=10) |
| 内存 | ~100-300MB（Node.js heap） | ~10-30MB（Rust binary） |
| 并发模型 | 事件循环（单线程 + worker） | tokio 多线程 async |
| 缓存 | 应用层可选 | Redis（自省结果 + 吊销集 + Service Token） |

---

## 9. 开发体验

| 维度 | Logto | Aspectus |
|------|-------|----------|
| 类型系统 | TypeScript（编译期 + 运行时） | Rust（编译期保证，无运行时开销） |
| 编译时间 | tsc: ~10s | cargo build: ~10s（增量），~2min（全量） |
| 测试 | Jest（单元 + 集成） | cargo test（单元 + testcontainers 集成） |
| 数据库迁移 | 自定义 migration 系统 | sqlx migrate |
| API 文档 | Swagger 自动生成 | OpenAPI 3.0 YAML 手写 |
| 热重载 | tsup dev（HMR） | cargo watch（重新编译） |

---

## 10. 设计取舍总结

| Aspectus 的选择 | 借鉴 Logto | 不同于 Logto | 原因 |
|----------------|:--:|:--:|------|
| `role_type` 约束 | ✅ | 扩展 `both` | 采纳成熟设计，适配 SA 场景 |
| `varchar(21)` 短 ID | ✅ | — | 相同方案 |
| 结构化审计日志 | — | ✅ | DB 约束 > JSONB 灵活性 |
| 单层 tenant | — | ✅ | 无 organization 需求 |
| 全局 Role | — | ✅ | 生态 scope 统一 |
| Project enum | — | ✅ | 生态项目已知固定 |
| Service Token | — | ✅ | 专属认证层，无 OIDC 包袱 |
| 配额管理 | — | ❌ 独有 | 生态核心需求 |
| Hybrid Token | — | ✅ | 自省优先，按场景选择格式 |
| Rust/axum | — | ✅ | 生态一致性 + 极致性能 |
| 无 UI | — | ✅ | Daypaw 统一管理界面 |

---

## 11. 适用场景

| 场景 | 推荐 |
|------|------|
| 需要完整 OIDC Provider 的 SaaS 产品 | **Logto** |
| 需要 SAML/SSO 的企业环境 | **Logto** |
| 需要 Social Login 的 C 端应用 | **Logto** |
| 需要内置 Sign-in UI | **Logto** |
| Pandaria 生态内部服务间认证 | **Aspectus** |
| 需要极低延迟 Token 验证（<5ms） | **Aspectus** |
| 需要配额管理 | **Aspectus** |
| 需要与 Rust 技术栈深度集成 | **Aspectus** |
| 需要最小化部署依赖 | **Aspectus** |
