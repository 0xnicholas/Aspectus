# AGENTS.md — Aspectus

> 本文件是 Aspectus 项目的首要上下文文档。所有参与开发的 agent（包括 AI coding agent 和人类工程师）在开始任何任务前必须阅读本文件。

---

## 项目定义

**Aspectus 是 Pandaria 生态的统一身份与多租户管理服务。它为生态所有项目提供单一身份源——统一 `tenant_id`、用户认证、API Key 管理、Token 自省和租户配额配置。**

当前 Pandaria 生态中，每个项目各自管理身份——Pandaria 用 HMAC token、Tavern 用 Bearer token、Emerald 用 API Key、Constell 用 NextAuth。同一个租户在不同系统中无法关联。Aspectus 将这些分散的身份孤岛统一为一个可审计、可治理的单一身份层。

---

## 架构原则

以下为不可变更的设计约束。每个功能、每个 PR、每次重构都必须遵循。

### 1. 单点身份，多点消费

Aspectus 是生态中**唯一**签发和验证身份的服务。任何其他项目不得自行签发 token、管理用户或维护独立的租户表。所有项目通过 Aspectus 的 token introspection 端点验证身份。

**这不是微服务之间共享一个 user 表——Aspectus 本身是身份的唯一权威源。**

### 2. 认证与授权分离——Aspectus 只管到项目门禁

Aspectus 的授权边界止于**项目级访问控制**：「用户 U 属于租户 T，可以访问 Pandaria session、Tavern workflow」。数据级的细粒度授权（「用户 U 可以对 Customer 表 read 但不能 drop」）属于 Heirloom 的范畴。

| 层 | 谁管 | 粒度 | 例子 |
|----|------|------|------|
| 身份认证 | Aspectus | 「你是谁」 | 用户 U 属于租户 T，持有 API Key K |
| 项目访问 | Aspectus | 「你能进哪个系统」 | 用户 U 可以访问 Pandaria、Tavern |
| 数据操作 | Heirloom | 「你能对这个 Resource 做什么」 | 用户 U 可以从 Customer read，不能 drop |

**Aspectus = 进门。Heirloom = 进房间后能碰什么。两者不重叠，不在同一项目中实现。**

### 3. 多租户是一等概念，不是事后附加

`tenant_id` 不是 `user` 表的一个可选字段。租户是整个数据模型的顶层命名空间。每个 API Key、每个 OAuth2 client、每个 user session 都必须限定在一个明确的 `tenant_id` 下。跨租户操作在设计上不可表达。

### 4. Token 自省优先于 Token 签发

Aspectus 的主要消费者不是人类用户——是其他服务（Pandaria、Tavern、Emerald、Constell）。这意味着：

- **Token 自省端点**（`POST /introspect`）是最高优先级的 API。它必须低延迟（p95 < 5ms）、高可用（每个服务每个请求都调它）
- Token 签发（OAuth2 / API Key 创建）是次要优先级——只在管理操作时调用，频率低
- 自省响应格式必须简洁：`{ active: bool, tenant_id, user_id, scopes[] }`——不过度设计

### 5. 生态优先，非通用

Aspectus 不是通用 Identity Provider（不要对标 Auth0 / Keycloak）。它专为 Pandaria 生态设计：

- 不需要支持 SAML、LDAP、Social Login（除非生态项目明确需要）
- 不需要多因素认证的 UI 流程（MVP 阶段）
- 不需要组织架构管理（HR 系统集成）

优先实现生态需要的，不提前设计「可能会用到」的功能。

---

## 关键设计决策

### ADR-001：Token 自省用 OAuth2 Token Introspection (RFC 7662) 语义

**决策**：自省端点遵循 RFC 7662，但简化。不实现完整的 OAuth2 Authorization Server——仅实现 token introspection 所需的子集。

**请求**：
```json
POST /introspect
Authorization: Bearer {service_token}
Content-Type: application/x-www-form-urlencoded

token={subject_token}
```

**响应**：
```json
{
  "active": true,
  "tenant_id": "org-acme",
  "user_id": "user-123",
  "client_id": "pandaria-api-gateway",
  "scope": "pandaria:session:create pandaria:session:read tavern:workflow:run",
  "token_type": "bearer",
  "exp": 1717000000
}
```

**理由**：RFC 7662 是工业标准，现有 OAuth2 库原生支持。各项目可以用标准 OAuth2 client 库调自省端点，无需自定义 HTTP client。

### ADR-002：API Key 是 per-tenant、per-project scoped

**决策**：API Key 不是全局的——每个 Key 绑定到一个 `(tenant_id, project, scopes)` 三元组。

```
API Key: pk_live_abc123
  tenant_id: org-acme
  project: pandaria
  scopes: [session:create, session:read]
  expires_at: 2027-01-01
  created_by: user-123
```

**理由**：细粒度 API Key 允许租户为不同项目签发不同权限的 Key。如果 Pandaria 的 Key 泄露，不影响 Tavern 和 Emerald。

### ADR-003：租户配额在 Aspectus 管理，各项目执行

**决策**：Aspectus 存储配额配置（如「租户 T 每月最多 10000 次 LLM 调用」），但配额执行在各项目侧——Tokencamp 执行 token 配额，Pandaria 执行 session 并发限制。Aspectus 的自省响应中携带配额元数据。

**自省响应扩展**：
```json
{
  "active": true,
  "tenant_id": "org-acme",
  "quotas": {
    "tokencamp": { "monthly_tokens": 10000000, "used": 4200000 },
    "pandaria": { "max_concurrent_sessions": 50 }
  }
}
```

**理由**：配额执行需要项目上下文（Tokencamp 知道 token 消耗速度，Pandaria 知道当前活跃 session 数）。Aspectus 只管配置，不管执行——避免成为所有请求的热路径瓶颈。

### ADR-004：Emerald 的 entity_id 使用 `tenant_id:user_id` 复合映射

**决策**：当 Aspectus 引入 per-user 身份后，Pandaria 的 `EmeraldMemoryStore` adapter 将 `entity_id` 从 `tenant_id` 升级为 `tenant_id:user_id`。

**迁移路径**：
```
Phase 1（当前）：entity_id = tenant_id
Phase 2（Aspectus 上线后）：entity_id = tenant_id:user_id
  → 新 session 使用新格式
  → 旧 Emerald 数据保留原 entity_id，不迁移（记忆有 natural decay）
```

**理由**：Emerald 的记忆是按 entity 隔离的。如果同一租户下有多个独立用户，他们应该有独立的记忆画像。分阶段迁移避免破坏已有记忆数据。

---

## 核心概念

| 术语 | 定义 |
|------|------|
| **Tenant（租户）** | 生态的顶层命名空间。拥有独立的用户集、API Key、配额配置。对应企业/组织。 |
| **User（用户）** | 属于某个 Tenant 的人类或服务账号。通过 OAuth2 或 API Key 认证。 |
| **API Key** | 长期凭证，per-tenant + per-project scoped。用于服务间调用和 Agent SDK。 |
| **Service Token** | 各项目调用 Aspectus 自省端点时使用的内部 token。与用户 token 区分。 |
| **Scope** | 权限标签，格式 `{project}:{resource}:{action}`。如 `pandaria:session:create`。 |
| **Role** | Scope 的命名集合。如 `agent-developer` = `[pandaria:session:*, tavern:workflow:*]`。 |
| **Client / Project** | 生态中的一个项目（Pandaria、Tavern、Emerald、Constell）。自省时作为 `client_id` 传入。 |
| **Token Introspection** |  RFC 7662 端点，验证 token 有效性并返回关联的 tenant/user/scopes。 |
| **Quota（配额）** | per-tenant 的资源限制配置。Aspectus 只存配置，不执行。 |

---

## API 设计

### 自省端点（P0——最高优先级）

```
POST /introspect
```

生态所有项目在接收每个请求时调用此端点。

### 管理 API（P1——Phase 2 后）

| 端点 | 说明 |
|------|------|
| `POST /tenants` | 创建租户 |
| `GET /tenants/{id}` | 查询租户配置与配额 |
| `PUT /tenants/{id}/quotas` | 更新租户配额 |
| `POST /api-keys` | 创建 API Key |
| `GET /api-keys` | 列出租户的所有 API Key |
| `DELETE /api-keys/{id}` | 吊销 API Key |
| `POST /users` | 创建用户（含角色分配） |
| `GET /users/{id}/scopes` | 查询用户有效 scope |

### OAuth2 端点（P2——Phase 3 后）

标准 OAuth2 Authorization Code flow + Client Credentials flow（用于 service account）。

---

## 与 Pandaria 生态的集成

| 项目 | 集成方式 | Aspectus 提供什么 |
|------|---------|------------------|
| **Pandaria** | api-gateway 在接收请求时调 `/introspect` | `tenant_id` + `user_id` + scopes + 配额 |
| **Tavern** | tavern-server 在接收请求时调 `/introspect` | 同上 |
| **Emerald** | 不直接调 Aspectus。Pandaria 在 `EmeraldMemoryStore` adapter 中传入 `tenant_id:user_id` | 间接——通过 Pandaria 的 entity_id 映射 |
| **Constell** | web + worker 调 `/introspect` 验证 API key | `tenant_id` + project scopes |
| **Tokencamp** | 调用 `/introspect` 验证 API key + 读取租户配额 | `tenant_id` + token 配额配置 |
| **Heirloom** | Phase 2+ 时 Heirloom 的 Auth 步骤调 `/introspect` | `tenant_id` + `user_id`（Heirloom 据此解析 Role） |

---

## 数据模型（逻辑）

```
Tenant
  ├── id: string (PK)
  ├── name: string
  ├── quotas: JSON
  └── created_at: timestamp

User
  ├── id: string (PK)
  ├── tenant_id: string (FK → Tenant)
  ├── email: string
  ├── roles: string[]
  └── created_at: timestamp

APIKey
  ├── id: string (PK)
  ├── tenant_id: string (FK → Tenant)
  ├── project: string          // "pandaria" | "tavern" | "constell" | ...
  ├── key_hash: string         // sha256(key)
  ├── key_prefix: string       // "pk_live_abc123" → prefix for UI display
  ├── scopes: string[]
  ├── created_by: string (FK → User)
  ├── expires_at: timestamp?
  └── revoked_at: timestamp?

AuditLog
  ├── id: string (PK)
  ├── tenant_id: string
  ├── actor_id: string         // user or service
  ├── action: string           // "api_key.created" | "token.introspected" | "tenant.quota.updated"
  ├── target: string           // affected resource id
  ├── metadata: JSON
  └── timestamp: timestamp
```

---

## 技术选型

| 维度 | 推荐 | 理由 |
|------|------|------|
| 语言 | Rust | 与 Pandaria/Tavern/Pawbun 一致，零成本自省性能 |
| 框架 | axum | 与 Tavern 一致，生态内统一 |
| 数据库 | PostgreSQL | 租户/用户/API Key 是关系型数据 |
| 缓存 | Redis | 自省结果缓存（TTL = token 剩余有效期的 1/10） |
| Token 格式 | JWT (self-contained) + Opaque (by reference) | JWT 用于服务间零网络调用场景；Opaque 用于需要吊销能力的场景 |
| 哈希 | argon2id（用户密码）、sha256（API Key） | 用户密码需要抗暴力破解，API Key 只需要不可逆 |

**为什么不用 external OSS Identity Provider（Keycloak / Zitadel / Ory）？**

这些工具面向通用企业场景，引入了一整套 Aspectus 不需要的复杂度（SAML、LDAP、Social Login、管理 UI、User Federation）。Aspectus 需要的是一个**轻量的、生态定制的、与 Pandaria 租户模型深度耦合的**身份层——不是一个通用 IdP 的配置实例。

---

## 项目结构（规划）

```
Aspectus/
├── Cargo.toml
├── crates/
│   ├── aspectus-core/          # 核心域模型：Tenant, User, APIKey, Scope, Role
│   ├── aspectus-server/        # axum HTTP 服务：introspect + 管理 API
│   ├── aspectus-auth/          # 认证逻辑：JWT 签发/验证、API Key 哈希、argon2id
│   └── aspectus-client/        # Rust client library（其他 Rust 项目用）
├── migrations/                 # PostgreSQL 迁移
├── tests/                      # 集成测试（testcontainers）
├── AGENTS.md                   # 本文件
└── README.md
```

---

## 实施阶段

### Phase 1 — 最小可用（MVP）

**目标**：Token 自省端点可用，Pandaria 可以调它验证 token。

- [ ] Tenant CRUD（API only，无 UI）
- [ ] API Key 创建与吊销
- [ ] `POST /introspect` 端点（RFC 7662 子集）
- [ ] 自省结果 Redis 缓存
- [ ] 审计日志
- [ ] Pandaria api-gateway 接入

**验收**：Pandaria 不再使用 HMAC token，改为调 Aspectus `/introspect` 验证 Bearer token。

### Phase 2 — 多项目接入 + 配额

- [ ] 多 project scope 支持
- [ ] 租户配额配置 API
- [ ] Tavern、Constell 接入
- [ ] API Key 管理 UI（可以是 Daypaw 中的应用）

### Phase 3 — 用户 + OAuth2

- [ ] User 模型 + 密码认证
- [ ] OAuth2 Authorization Code flow
- [ ] Role 管理
- [ ] Emerald entity_id 迁移到 `tenant_id:user_id`

---

## 安全约束

1. **API Key 永远不可逆**：数据库只存 `sha256(key)`。Key 原文仅在创建时返回一次。丢失 = 必须重新创建。
2. **自省端点本身需要认证**：调用 `/introspect` 的服务必须先通过 service token 认证（避免匿名扫描）。
3. **租户隔离不可违反**：任何查询/操作必须限定在调用者的 `tenant_id` 范围内。SQL 查询必须有 `WHERE tenant_id = $1`。
4. **审计日志不可变**：`AuditLog` 表 append-only，无 UPDATE/DELETE。敏感的 token 操作（创建、吊销）必须记录。
5. **密钥不得出现在日志中**：JWT signature、API Key 原文、用户密码——禁止出现在任何 tracing span、日志、错误消息中。
6. **跨租户登录路由**：`/login` 必须按 `(tenant_id, email)` 复合查找用户（ADR-016 决策 1），不得仅按 email 查询（跨租户同邮箱会路由到错误身份）。`/login/lookup` 端点对未知邮箱必须返回空列表（非 4xx），避免邮箱枚举攻击。

---

## 错误处理

- 所有 API 错误返回 RFC 7807 Problem Details 格式
- 自省端点对无效/过期 token 返回 `{ active: false }`（非 4xx）——遵循 RFC 7662，避免信息泄漏
- 管理 API 的授权失败返回 403，认证失败返回 401

---

## 测试要求

- 单元测试：Token 签发/验证、API Key 哈希、scope 匹配逻辑
- 集成测试：`testcontainers` 启动 PostgreSQL + Redis，测试完整自省流程
- 契约测试：确保自省响应格式与 Pandaria/Tavern 的期望一致
- 性能测试：`POST /introspect` ——目标 p95 < 5ms（含 Redis 缓存命中）

---

## 非目标（不做的事）

- ❌ SAML / LDAP / Social Login 集成
- ❌ 多因素认证 UI
- ❌ 用户自助注册 / 密码重置流程（MVP 阶段）
- ❌ 组织架构管理（部门、汇报链）
- ❌ 数据级授权（属于 Heirloom）
- ❌ Web UI（管理界面在 Daypaw 中实现，不在 Aspectus 中）

---

*本文件随项目演进持续更新。每次架构决策变更时，同步更新 ADR 部分并在 git commit message 中注明 `docs: update AGENTS.md`。*
