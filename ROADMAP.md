# Aspectus Roadmap

> 最后更新：2026-06-23
> 当前：**v0.9.0 完成 — 用户认证就绪；控制台与集成测试已补齐**

```
✅ 后端：Rust/axum | 28+ API | 179 tests | 21 migrations | 14 tables
✅ 前端：React/TypeScript | 10 pages | 12+ 组件 | Vite
✅ 文档：OpenAPI 3.0 | ADR×15 | Spec×7 | 对比×2
✅ 部署：Dockerfile | docker-compose | Helm chart | env config
✅ JWT 本地验证 | JWKS 公钥 | 注册/登录/登出/密码重置
✅ Service Token / Audit Log 后端 + UI
✅ Tenant / Service Account 详情页
✅ cargo test --workspace 全绿（含 testcontainers 集成测试）
```

---

## 版本全景

```
v0.1 ──→ v0.2 ──→ v0.3 ──→ v0.4 ──→ v0.5 ──→ v0.6 ──→ v0.7 ──→ v0.8 ──→ v0.9
骨架    MVP    多项目   JWT     User    OAuth2  Refresh  Metrics  用户认证
                +配额   Opaque   Role    Code    Token   +Docs    就绪
```

| 版本 | 状态 | 内容 |
|------|:--:|------|
| [v0.1.0](#v010--项目骨架) | ✅ | 项目骨架 + DB schema |
| [v0.2.0](#v020--mvp服务账号--api-key--introspect) | ✅ | MVP: /introspect + 管理 API |
| [v0.3.0](#v030--多项目接入--配额) | ✅ | 多项目 scope + 配额 + 性能 |
| v0.4.0 | ✅ | JWT + Opaque Token |
| v0.5.0 | ✅ | User + Role 管理 |
| v0.6.0 | ✅ | OAuth2 Authorization Code |
| v0.7.0 | ✅ | Refresh Token + OAuth2 Clients |
| v0.8.0 | ✅ | Metrics + OpenAPI 文档 |
| v0.9.0 | ✅ | 用户认证就绪: /login /register /logout /forgot-password /reset-password, JWKS 真实公钥, JWT 本地验证 (aspectus-client), identity_type in JWT, 审计日志 |
| v0.9.1 | ⏳ | 控制台体验打磨：统一 loading/错误/空状态、搜索筛选、分页、Dashboard 统计卡、用户角色管理 UI |
| v1.0.0 | ⬜ | API 稳定承诺 |

---

## 版本依赖链

```
v0.1.0 ──→ v0.2.0 ──→ v0.3.0 ──→ v1.0.0
  │            │            │            │
  │            │            │            └── 依赖 v0.3.0 的 /introspect 格式
  │            │            │               + 新增 users/roles/oauth2 表
  │            │            │               + v0.9.0 用户认证端点
  │            │            │
  │            │            └── 依赖 v0.2.0 的 /introspect 端点和数据模型
  │            │               + 新增 quotas 列、scopes 数据行
  │            │
  │            └── 依赖 v0.1.0 的 crate 结构和 DB schema
  │               + 实现业务逻辑
  │
  └── 无依赖（纯骨架）
```

**每个版本是前一个版本的增量，不可跳过。** v0.2.0 的 DB schema 是 v0.3.0 的基础，v0.3.0 的 `/introspect` 响应格式是 v1.0.0 所有消费者依赖的契约。

---

## 版本兼容性约定

| 约定 | 说明 |
|------|------|
| **v0.x.0 → v0.x.0** | 跨 minor 版本可能引入 breaking change，需 migration |
| **v0.x.y → v0.x.z** | patch 版本向后兼容（bug fix、性能优化），无需 migration |
| **`/introspect` 响应** | v0.2.0 定义基础字段。v0.3.0 只增 `quotas` 字段，不删不改已有字段。v1.0.0 只增 `identity_type=user` 路径，不删不改已有路径 |
| **管理 API** | v0.2.0 的管理 API 在 v0.3.0 中保持兼容（新增端点但不改已有端点签名） |
| **v1.0.0** | 长期稳定承诺。此后 `/introspect` 格式和管理 API 签名进入 semver 严格管理，breaking change 需要 v2.0.0 |

---

## v0.1.0 — 项目骨架

| 属性 | 值 |
|------|-----|
| **目标** | Rust 项目可编译、可运行、可测试，DB schema 就绪 |
| **产出** | Cargo workspace + docker-compose + CI |
| **消费者** | 无（内部开发基础设施） |
| **前置依赖** | 无 |
| **被依赖** | v0.2.0, v0.3.0, v1.0.0 |
| **API 稳定性** | —（无业务 API） |
| **周期** | 2-3 天 |

### 边界：做什么 / 不做什么

| ✅ 做 | ❌ 不做 |
|------|--------|
| Cargo workspace 4 crates | 业务逻辑（Tenant CRUD 等） |
| docker-compose (PG17 + Redis7) | HTTP endpoint（除 /health） |
| 初始 DB migration（全部表建好） | API Key hash/verify |
| GitHub Actions CI | `/introspect` 实现 |
| `/health` 返回 200 | JWT／OAuth2 任何代码 |

### 功能清单

#### 0.1.1 Cargo workspace

- [ ] `Cargo.toml` workspace root

| Crate | 职责 | 对外依赖 |
|-------|------|---------|
| `aspectus-core` | 域模型：enum、struct、trait 定义 | 无 |
| `aspectus-server` | axum HTTP 服务 | `aspectus-core`, `aspectus-auth` |
| `aspectus-auth` | 认证逻辑（hash/verify stub） | `aspectus-core` |
| `aspectus-client` | 供其他 Rust 项目引用的 client | `aspectus-core` |

#### 0.1.2 开发环境

- [ ] `docker-compose.yml`：PostgreSQL 17 + Redis 7
- [ ] `.env.example`：`DATABASE_URL`, `REDIS_URL`, `SERVICE_TOKENS`
- [ ] `justfile`：`dev`, `test`, `migrate`, `lint`, `clean`

#### 0.1.3 DB schema（所有表，完整迁移）

> 一次性建好四个版本需要的全部表，后续版本只做增量 migration（加列、加约束、加索引）。

| 表 | 覆盖版本 | 关键列 |
|----|:--:|------|
| `tenants` | v0.2 | `id`, `name`, `quotas JSONB`, `created_at` |
| `service_accounts` | v0.2 | `id`, `tenant_id`, `label`, `description`, `expires_at` |
| `api_keys` | v0.2 | `id`, `tenant_id`, `service_account_id`, `project`, `key_hash`, `key_prefix`, `scopes TEXT[]`, `expires_at`, `revoked_at` |
| `audit_logs` | v0.2 | `id`, `tenant_id`, `actor_id`, `actor_type`, `action`, `target_type`, `target_id`, `metadata JSONB`, `created_at` |
| `scopes` | v0.3 | `id`, `name` |
| `service_tokens` | v0.2 | `project`, `token_hash`, `created_at` |
| `users` | v1.0 | 见 v1.0.0（表在 v0.1.0 建好，列在 v1.0.0 的 migration 中激活） |
| `roles` | v1.0 | 同上 |
| `roles_scopes` | v1.0 | 同上 |
| `users_roles` | v1.0 | 同上 |

- [ ] PostgreSQL enum：`identity_type ('user','service_account')`, `project ('pandaria','tavern','emerald','constell','tokencamp','heirloom')`

> **为什么在 v0.1.0 建好 v1.0.0 的表？** 避免跨版本 migration 时的大表 DDL 锁问题。v0.1.0 尚无生产数据，建表成本为零。v0.2.0/v0.3.0 时这些空表无任何读写开销。

#### 0.1.4 CI/CD

- [ ] GitHub Actions：`build`, `test`, `clippy`, `fmt`
- [ ] testcontainers 集成测试框架就绪（尚无测试用例）

### 验收

```bash
cargo build --workspace         # 0 errors
cargo test --workspace           # 框架可运行
docker compose up -d             # PG + Redis 启动
cargo run -p aspectus-server     # GET /health → 200
```

### 参考资料

- [ADR-004](./docs/adr/004-user-vs-service-account-role-type.md) — IdentityType enum
- [ADR-010](./docs/adr/010-project-static-enum.md) — Project enum
- [ADR-012](./docs/adr/012-technology-stack.md) — 技术选型
- [ADR-015](./docs/adr/015-id-format-short-id.md) — ID 格式

---

## v0.2.0 — MVP：服务账号 + API Key + `/introspect`

| 属性 | 值 |
|------|-----|
| **目标** | Pandaria api-gateway 不再用 HMAC token，改为调 Aspectus `/introspect` 验证 API Key |
| **产出** | `/introspect` 端点 + 管理 API（Tenant/SA/APIKey CRUD）+ 审计日志 |
| **消费者** | Pandaria api-gateway |
| **前置依赖** | v0.1.0（crate 结构 + DB schema） |
| **被依赖** | v0.3.0, v1.0.0 |
| **API 稳定性** | ⚠️ 不稳定——`/introspect` 响应格式在 v0.3.0 会新增 `quotas` 字段 |
| **周期** | 3-4 周 |

### 边界：做什么 / 不做什么

| ✅ 做 | ❌ 不做 |
|------|--------|
| `/introspect`（API Key 验证） | JWT / Opaque Token 签发和验证 |
| Tenant / SA / APIKey CRUD | User 管理 / OAuth2 |
| Redis 自省结果缓存 | 配额配置 API（quota 字段预留但不生效） |
| 审计日志（append-only） | Role 定义 / Scope 通配符匹配 |
| Service Token 认证调用方 | 多 Project scope 校验（scope 按自由文本接受） |
| Pandaria 接入 | Constell / Tokencamp 接入 |

### 版本内部迭代

```
v0.2.0 ──→ v0.2.1 (bug fix) ──→ v0.2.2 (性能调优) ──→ v0.2.x (稳定)
  │
  └── 之后 v0.3.0 分支
```

### 功能清单

#### 2.1 `aspectus-core` — 域模型

- [ ] **Tenant** struct + `TenantStore` trait
- [ ] **ServiceAccount** struct + `ServiceAccountStore` trait
- [ ] **ApiKey** struct + `ApiKeyStore` trait
  - `key_hash = sha256(key)` 生成
  - `key_prefix` 提取（`pk_live_` + KSUID 前 8 字符）
  - scope 解析（空格分隔 `String` → `Vec<String>`；暂不做通配符匹配）
- [ ] **Scope** 基础 struct（Phase 1 只做字符串存储，不做匹配）
- [ ] **Project** enum + `Display`/`FromStr`
- [ ] **IdentityType** enum
- [ ] **AuditLog** struct + `AuditLogStore` trait（仅 INSERT，无 UPDATE/DELETE 路径）
- [ ] **IntrospectResponse** struct（v0.2.0 版本，不含 quotas）
- [ ] 错误类型：`AuthError`, `ValidationError`, `NotFoundError`

#### 2.2 `aspectus-auth` — 认证逻辑

- [ ] **ApiKeyCreator**：生成 32-byte 随机 key + KSUID 前缀 → 返回原文（仅一次）→ 存储 sha256
- [ ] **ApiKeyVerifier**：`sha256(token)` → Redis（TTL=`min(剩余有效期/10, 300s)`）→ PostgreSQL fallback → 返回 IntrospectResponse
- [ ] **ServiceTokenVerifier**：`sha256(service_token)` → Redis（TTL=60s）→ PostgreSQL fallback → 返回 Project 身份
- [ ] 吊销：更新 `api_keys.revoked_at` + 删除 Redis 缓存

#### 2.3 `aspectus-server` — HTTP 服务

**P0 — 自省端点**

- [ ] `POST /introspect`
  - Service Token 认证 middleware（`Authorization: Bearer {service_token}`）
  - 提取 `token` 参数（`application/x-www-form-urlencoded`）
  - 调度 `ApiKeyVerifier`
  - 有效：`200 { active: true, tenant_id, user_id, identity_type:"service_account", client_id, scope, token_type:"Bearer", exp }`
  - 无效/吊销/过期：`200 { active: false }`
  - Service Token 无效：`401 application/problem+json`

**P1 — 管理 API**

| 端点 | 方法 | 说明 |
|------|------|------|
| `/tenants` | POST | 创建租户 |
| `/tenants/{id}` | GET | 查询租户（含 quotas，但 v0.2.0 始终返回空对象） |
| `/service-accounts` | POST | 创建 Service Account |
| `/service-accounts` | GET | 列出 tenant 下的 SA（`?tenant_id=...`） |
| `/api-keys` | POST | 创建 API Key → 返回 `{ id, key_prefix, key: "pk_live_..." }`（原文仅此一次） |
| `/api-keys` | GET | 列出 API Key（`?service_account_id=...`；不返回 key_hash 或原文） |
| `/api-keys/{id}` | DELETE | 吊销 API Key |

**基础设施**

- [ ] RFC 7807 错误处理 middleware
- [ ] `tracing` crate + trace_id 全链路
- [ ] CORS（管理 API 供 Daypaw 调用）
- [ ] `GET /health`

#### 2.4 审计日志

- [ ] 记录以下事件（全部 `INSERT`，无 `UPDATE`/`DELETE`）：
  - `tenant.created`
  - `service_account.created`
  - `api_key.created`
  - `api_key.revoked`
  - `token.introspected`（采样：每 1000 次记录 1 次，或仅记录 `active=false`）
- [ ] `metadata` 字段中绝不出现 `key_hash`, `password`, `secret`, `signature`
- [ ] 编译期保证：audit struct 的 `Serialize` 不导出敏感字段

#### 2.5 测试

| 类型 | 覆盖 |
|------|------|
| 单元测试 | API Key hash/verify、Service Token hash/verify、ID 格式、错误类型转换 |
| 集成测试 (testcontainers) | 完整链路：创建 Tenant → 创建 SA → 创建 API Key → `/introspect` 返回 active:true → 吊销 → `/introspect` 返回 active:false → 无效 Service Token → 401 → 缓存 hit/miss 路径 |
| 契约测试 | `/introspect` 响应 JSON schema 与 Pandaria api-gateway 期望一致 |
| 安全测试 | 密钥不出现日志（grep tracing output + 集成测试 snapshot） |

#### 2.6 Pandaria 接入

- [ ] Pandaria api-gateway 引入 `aspectus-client`
- [ ] 废弃现有 HMAC token 验证逻辑
- [ ] 端到端：Pandaria HTTP 请求 → api-gateway → `POST /introspect` → 解析响应 → 注入 `tenant_id`/`scopes` → 放行或 401

### 验收

```
✅ POST /introspect p95 < 5ms（Redis 命中）、< 10ms（Redis miss）
✅ 创建 API Key → /introspect → active: true
✅ 吊销 API Key → /introspect → active: false
✅ Service Token 无效 → 401 RFC 7807
✅ Pandaria 完全切换到 Aspectus，HMAC token 逻辑已移除
✅ 审计日志记录 api_key.created / api_key.revoked
✅ grep -r "key_hash\|password\|secret" tracing-log/ → 0 结果
```

### v0.2.x patch 版本（预期）

| 版本 | 内容 |
|------|------|
| v0.2.1 | Bug fix：Redis 连接断开时的 graceful degradation、边界条件处理 |
| v0.2.2 | 性能：连接池参数调优、缓存 TTL 调整、DB 索引优化 |
| v0.2.3+ | Pandaria 集成反馈驱动的小修复 |

### 参考资料

- [ADR-001](./docs/adr/001-token-introspection-rfc7662.md) — `/introspect` 设计
- [ADR-002](./docs/adr/002-api-key-per-tenant-per-project.md) — API Key 模型
- [ADR-003](./docs/adr/003-quota-config-vs-enforcement.md) — 配额分离（v0.2.0 只预留字段）
- [ADR-009](./docs/adr/009-audit-log-structured-vs-jsonb.md) — 审计日志
- [ADR-011](./docs/adr/011-service-token-separate-auth.md) — Service Token
- [ADR-014](./docs/adr/014-error-handling-rfc7807.md) — 错误格式

---

## v0.3.0 — 多项目接入 + 配额

| 属性 | 值 |
|------|-----|
| **目标** | Constell、Tokencamp 接入 `/introspect`。配额配置 API 可用 |
| **产出** | 多 Project scope 定义、配额管理 API、Daypaw 管理 UI |
| **消费者** | Pandaria（不变）, Constell, Tokencamp |
| **前置依赖** | v0.2.0（`/introspect` + 管理 API） |
| **被依赖** | v1.0.0 |
| **API 稳定性** | ⚠️ 不稳定——`/introspect` 响应新增 `quotas` 字段，v0.2.x 消费者不受影响（新字段被忽略）。管理 API 新增端点，不影响已有端点 |
| **周期** | 2-3 周 |

### 边界：做什么 / 不做什么

| ✅ 做 | ❌ 不做 |
|------|--------|
| 所有 6 个 Project 的 scope 定义 + 校验 | User 管理 |
| `PUT /tenants/{id}/quotas` | OAuth2 / JWT |
| `/introspect` 响应新增 `quotas` 字段 | Role 定义（role_type 约束在 DB schema 已存在，但 Role 数据不填充） |
| Constell / Tokencamp 接入 | Service Account 通过 Role 获得 scope（Phase 2 SA 仍直接绑 scope） |
| Daypaw API Key 管理 UI | Emerald entity_id 迁移 |

### 与 v0.2.0 的关系

```
v0.2.0 消费者（Pandaria）
  │
  │  v0.2.0 /introspect 响应：{ active, tenant_id, user_id, identity_type, client_id, scope, exp }
  │  v0.3.0 /introspect 响应：{ ..., quotas: { pandaria: { max_concurrent_sessions: 50 } } }
  │
  │  → Pandaria 无需修改代码，忽略 quotas 字段即可正常工作
  │
  v0.3.0 新消费者（Constell, Tokencamp）
  │
  │  直接对接 v0.3.0 的 /introspect，可选使用 quotas 字段
```

**向后兼容策略**：只增字段，不删不改已有字段。v0.2.0 的 HTTP client 对 v0.3.0 新增字段按 JSON 反序列化默认值处理即可。

### 功能清单

#### 3.1 Scope 定义

- [ ] 每个 Project 的 scope 种子数据（写入 `scopes` 表）：

| Project | Scope 示例 |
|---------|-----------|
| `pandaria` | `pandaria:session:create`, `pandaria:session:read`, `pandaria:session:delete`, `pandaria:agent:execute`, `pandaria:agent:manage` |
| `constell` | `constell:agent:publish`, `constell:agent:install`, `constell:agent:read` |
| `tokencamp` | `tokencamp:token:consume`, `tokencamp:token:meter`, `tokencamp:token:manage` |
| `heirloom` | `heirloom:resource:read`, `heirloom:policy:read`, `heirloom:policy:manage` |
| `emerald` | 无直接 scope（通过 Pandaria 间接使用），保留枚举但 scope 集为空 |

- [ ] API Key 创建时校验 scope 值 ∈ `scopes` 表中对应 Project 的合法集合
- [ ] v0.2.0 创建的 API Key（scope 为自由文本）在 v0.3.0 仍可用，不被校验拦截

#### 3.2 配额管理

- [ ] `PUT /tenants/{id}/quotas`：接受 per-project 的 limit 配置
  ```json
  { "pandaria": { "max_concurrent_sessions": 50 }, "tokencamp": { "monthly_tokens": 10000000 } }
  ```
- [ ] 配额写入选填（未配置的 project 无限制）
- [ ] `/introspect` 响应新增 `quotas` 字段（仅当前 token 所属 project 的配额）
- [ ] `GET /tenants/{id}` 响应返回全量配额配置
- [ ] 配额变更审计日志：`quota.updated`（`metadata` 记录 before/after）

#### 3.3 多项目接入

每个项目接入的步骤相同：

1. Aspectus 侧：配置该 Project 的 Service Token → 写入 `service_tokens` 表
2. 项目侧：引入 `aspectus-client`（或 HTTP client）→ 在请求处理链中调 `/introspect`
3. 验证：端到端测试通过

- [ ] **Constell**：web + worker 调 `/introspect` 验证 API Key
- [ ] **Tokencamp**：调 `/introspect` + 读取 `quotas.tokencamp` 字段执行限流
- [ ] Heirloom：v0.3.0 不接入（数据级授权需要 v1.0.0 的 User 身份），但 scope 预定义

#### 3.4 Daypaw API Key 管理 UI

- [ ] API Key 列表（按 Project 分组，显示 `key_prefix`、scopes、过期时间、状态）
- [ ] 创建 API Key 对话框（选择 Service Account + Project + scopes → 展示原文一次）
- [ ] 吊销按钮（确认后吊销）

### 验收

```
✅ 6 个 Project 的 scope 种子数据完整且正确
✅ 创建 API Key 时非法 scope 被拒绝（422）
✅ v0.2.0 创建的 API Key 仍可正常自省（不因 scope 校验失败而中断）
✅ PUT /tenants/{id}/quotas → 写入成功 → GET 返回正确值
✅ /introspect 响应含 quotas 字段（限当前 project 的配额）
✅ Constell / Tokencamp 正常调用 /introspect
✅ Daypaw 可完成「查看列表 → 创建 → 查看 → 吊销」完整流程
```

### v0.3.x patch 版本（预期）

| 版本 | 内容 |
|------|------|
| v0.3.1 | Bug fix：配额边界条件、scope 校验误报 |
| v0.3.2 | Daypaw UI 体验优化 |
| v0.3.3+ | Constell/Tokencamp 集成反馈 |

### 参考资料

- [ADR-003](./docs/adr/003-quota-config-vs-enforcement.md) — 配额
- [ADR-005](./docs/adr/005-role-global-definition.md) — Role 定义（v0.3.0 表已建、数据不填）
- [ADR-006](./docs/adr/006-scope-format.md) — Scope 格式

---

## v1.0.0 — 用户 + OAuth2 + Role

| 属性 | 值 |
|------|-----|
| **目标** | 人类用户可登录，完整身份平台上线。API 进入长期稳定 |
| **产出** | User CRUD + OAuth2 Authorization Code flow + JWT + Role 分配 + Emerald 迁移 |
| **消费者** | 全部生态项目（Pandaria, Constell, Tokencamp, Heirloom） |
| **前置依赖** | v0.3.0（`/introspect` + 配额 + scope 定义） |
| **API 稳定性** | ✅ **长期稳定**。此后 `/introspect` 格式和管理 API 签名受 semver 保护 |
| **周期** | 3-4 周 |

### 边界：做什么 / 不做什么

| ✅ 做 | ❌ 不做 |
|------|--------|
| User 注册/禁用、argon2id 密码 | SAML / LDAP / Social Login |
| OAuth2 Authorization Code flow | MFA（多因素认证） |
| Client Credentials flow（SA 可选） | 用户自助注册 / 密码重置流程 |
| JWT Access Token + Refresh Token | 组织架构管理（HR 集成） |
| Role 管理 + `role_type` 约束 | Web UI（登录页面由各项目自建，非 Aspectus 负责） |
| Emerald entity_id 迁移 | 数据级细粒度授权（Heirloom 范畴） |
| Heirloom 接入（通过 User identity） | |

### 与 v0.3.0 的关系

```
v0.3.0 消费者
  │
  │  /introspect 对 API Key (SA) 的响应格式不变
  │  /introspect 新增 User token 路径，新增 identity_type: "user"
  │
  │  → SA 路径完全兼容，已有消费者无需任何修改
  │
  v1.0.0 新能力
  │
  │  User + OAuth2：全新认证方式，不替代 API Key，两者共存
  │  Role：User 通过 Role 获得 scope；SA 继续保持直接绑定 scope
  │  Heirloom：首次接入 Aspectus，依赖 User 身份做数据级授权
```

**关键承诺**：v1.0.0 **不改变** Service Account + API Key 的任何行为。v0.2.0/v0.3.0 消费者 **零修改** 即可在 v1.0.0 上运行。User + OAuth2 是**新增能力**，不是替代品。

### 功能清单

#### 4.1 User 模型

- [ ] `users` 表数据访问层（表已在 v0.1.0 建好）
  - `id`, `tenant_id`, `email`, `password_hash` (argon2id), `display_name`, `is_suspended`, `created_at`
- [ ] `POST /users` — 创建用户（管理员操作，含初始密码）
- [ ] `GET /users/{id}` — 查询用户（不含 password_hash）
- [ ] `GET /users` — 列出 tenant 用户（`?tenant_id=...`）
- [ ] `PUT /users/{id}/disable` — 禁用用户
- [ ] `/introspect` 对 User token 返回 `identity_type: "user"`
- [ ] User 不可直接创建 API Key（Phase 3 用户只能通过 OAuth2 登录，API Key 由管理员代为创建）

#### 4.2 OAuth2 Authorization Code Flow

- [ ] `GET /authorize` — 授权端点
  - 参数：`client_id`, `redirect_uri`, `response_type=code`, `scope`, `state`
  - 返回：重定向到 login 页面（由各项目自建，Aspectus 提供 API）
- [ ] `POST /token` — Token 端点
  - `grant_type=authorization_code` → `{ access_token (JWT), refresh_token (opaque), expires_in }`
  - `grant_type=refresh_token` → 新 access_token + 新 refresh_token（旧 refresh_token 失效）
  - `grant_type=client_credentials` → SA 可选使用（替代直接 API Key 的场景）
- [ ] JWT Access Token 签发（RS256，含 `sub`, `tenant_id`, `scope`, `exp`, `jti`）
- [ ] JWT 验签（各项目可用 public key 本地验签，跳过 `/introspect` 网络调用）
- [ ] JWT 吊销（jti 加入 Redis Set，验签时检查）
- [ ] Refresh token 存储在 DB，支持轮转和吊销

#### 4.3 Role 管理

- [ ] `roles` 表种子数据（全局 Role 定义，v0.1.0 已建表）：

| Role | type | scopes |
|------|------|--------|
| `tenant-admin` | both | 管理 API 全权限（不映射到 scope，通过管理 API 认证单独控制） |
| `agent-developer` | user | `pandaria:session:*, pandaria:agent:*, constell:agent:publish` |
| `agent-operator` | user | `pandaria:session:read, pandaria:agent:execute` |
| `project-admin` | user | 单个 project 的全部 scope（创建 API Key 时裁剪） |
| `ci-deployer` | service_account | `pandaria:session:create` |

- [ ] `users_roles` 关联表：将 Role 分配给 User
- [ ] `role_type` DB check constraint（User 只能赋予 type=`user`/`both` 的 Role；SA 同理）
- [ ] 用户有效 scope = 所有分配 Role 的 scope 并集（展开后去重）
- [ ] `/introspect` 返回的 `scope` = Role 展开结果

#### 4.4 Service Account Role（可选 Phase）

- [ ] v1.0.0 SA **默认仍直接绑定 scope**（与 v0.2.0 行为一致）
- [ ] 可选：SA 可通过 API 绑定 Role（`role_type = service_account | both`），scope 由 Role 展开
- [ ] SA 同时支持两种模式：
  - 直接 scope（精确控制，适合 CI pipeline）
  - Role scope（模板化管理，适合标准化部署）

#### 4.5 Emerald entity_id 迁移

- [ ] Pandaria `EmeraldMemoryStore` adapter 更新
  - `entity_id` 从 `tenant_id` 改为 `tenant_id:user_id`
  - 对 SA token：`entity_id` 仍为 `tenant_id`（SA 无 user 关联）
- [ ] 旧 Emerald 数据保留，不迁移
- [ ] 部署文档：Pandaria 先在 staging 验证，再滚动生产

#### 4.6 Heirloom 接入

- [ ] Heirloom 调 `/introspect` 获取 `tenant_id` + `user_id` + `scopes`
- [ ] Heirloom 基于 `user_id` 解析数据级 Role（具体在 Heirloom 侧实现，非 Aspectus 范围）

### 验收

```
✅ User 通过 OAuth2 Authorization Code 登录 → 获取 JWT
✅ JWT /introspect → active: true, identity_type: "user"
✅ JWT 吊销 → /introspect → active: false
✅ Role 分配 → scope 正确展开（单元测试覆盖所有 Role）
✅ role_type 约束生效：user-type Role 分配给 SA → DB 拒绝
✅ 已有 SA + API Key 路径完全不受影响
✅ Emerald entity_id = tenant_id:user_id（User session），= tenant_id（SA session）
✅ Heirloom 可正常调用 /introspect
✅ v0.2.0 / v0.3.0 消费者（Pandaria、Constell、Tokencamp）零修改运行
```

### v1.0.x patch 版本（预期）

| 版本 | 内容 |
|------|------|
| v1.0.1 | Bug fix：OAuth2 边界条件、JWT 验签 edge case |
| v1.0.2 | 安全加固：rate limiting on /token、refresh token rotation 增强 |
| v1.0.3+ | 集成反馈 + 性能优化 |

### 参考资料

- [ADR-004](./docs/adr/004-user-vs-service-account-role-type.md) — User vs SA + role_type
- [ADR-005](./docs/adr/005-role-global-definition.md) — Role 定义
- [ADR-007](./docs/adr/007-hybrid-token-model.md) — JWT + Opaque Token
- [ADR-013](./docs/adr/013-emerald-entity-id-mapping.md) — Emerald 迁移

---

## 跨版本关注项

### API 演进矩阵

| `/introspect` 响应字段 | v0.2.0 | v0.3.0 | v1.0.0 |
|----------------------|:--:|:--:|:--:|
| `active` | ✅ | ✅ | ✅ |
| `tenant_id` | ✅ | ✅ | ✅ |
| `user_id` | ✅ (SA ID) | ✅ (SA ID) | ✅ (SA ID or User ID) |
| `identity_type` | ✅ (`"service_account"`) | ✅ | ✅ (`"user"` 新增) |
| `client_id` | ✅ | ✅ | ✅ |
| `scope` | ✅ | ✅ | ✅ |
| `token_type` | ✅ | ✅ | ✅ |
| `exp` | ✅ | ✅ | ✅ |
| `quotas` | ❌ | ✅ | ✅ |
| `sub` (OAuth2) | ❌ | ❌ | ✅ (JWT only) |

### 数据迁移路径

```
v0.1.0: 所有表建好（空）
  │
  v0.2.0: tenants, service_accounts, api_keys, audit_logs 开始有数据
  │       scopes 表为空（v0.3.0 填充）
  │       users, roles, etc. 为空（v1.0.0 激活）
  │
  v0.3.0: + migration: scopes 种子数据 INSERT
  │       tenants.quotas 列已有 → 开始被 PUT 写入
  │
  v1.0.0: + migration: users 激活，roles 种子数据 INSERT
  │       无表结构 DDL（v0.1.0 已建），只有 INSERT + 约束激活
  │
  v1.0.x: 只有 patch（bug fix / 索引优化 / 参数调优），无 schema change
```

### 消费者升级指南

| 消费者 | v0.2.0 | v0.3.0 | v1.0.0 |
|------|:--:|:--:|:--:|
| **Pandaria** | 首次接入 | 无修改 | 无修改（可选：Emerald entity_id 升级） |
| **Constell** | — | 首次接入 | 无修改 |
| **Tokencamp** | — | 首次接入 | 无修改 |
| **Heirloom** | — | — | 首次接入 |
| **Daypaw** | — | API Key 管理 UI 接入 | 无修改 |

---

## 当前状态

```
v0.1.0  ████████████████  100%  项目骨架 + DB schema
v0.2.0  ████████████████  100%  /introspect + 管理 API + Service Token
v0.3.0  ████████████████  100%  多项目 scope + 配额 + 性能基准框架
v0.4.0  ████████████████  100%  JWT + Opaque Token
v0.5.0  ████████████████  100%  User + Role 管理
v0.6.0  ████████████████  100%  OAuth2 Authorization Code
v0.7.0  ████████████████  100%  Refresh Token + OAuth2 Clients
v0.8.0  ████████████████  100%  Metrics + OpenAPI 文档
v0.9.0  ████████████████  100%  用户认证就绪
v1.0.0  ░░░░░░░░░░░░░░░░   0%  API 稳定承诺（待发布）

设计     ████████████████  100%
```

## 近期补齐（v0.9.1 方向）

在 v1.0.0 稳定 API 之前，优先完成控制台与工程化收尾：

- 管理控制台 UX 统一：loading、错误/空状态、表单校验、搜索与分页。
- Dashboard 统计卡：展示租户数、用户数、API Key 数、Service Token 状态。
- 用户角色管理 UI：在 Users 页面直接分配/移除角色。
- 持续提升集成测试稳定性与覆盖率。
- 文档同步：README、AGENTS.md、ROADMAP、生产部署注释。

> 注：v0.1.0–v0.9.0 的功能已在代码中实现。下方各版本的详细功能清单保留作为历史规划参考，实际完成情况以 `AGENTS.md` 和代码为准。

---

## 相关文档

- [AGENTS.md](./AGENTS.md) — 项目总纲与架构原则
- [docs/adr/](./docs/adr/) — 15 篇架构决策记录
- [docs/superpowers/specs/2026-05-29-concepts-and-architecture-design.md](./docs/superpowers/specs/2026-05-29-concepts-and-architecture-design.md) — 概念细化
