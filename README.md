# Aspectus

Pandaria 生态的统一身份与多租户管理服务。

## 概述

Aspectus 是 Pandaria 生态的单一身份源，为所有项目提供统一的 `tenant_id`、用户认证、API Key 管理、Token 自省和租户配额配置。

当前生态中每个项目各自管理身份（Pandaria 用 HMAC token、Tavern 用 Bearer token、Emerald 用 API Key、Constell 用 NextAuth）。Aspectus 将这些分散的身份孤岛统一为一个可审计、可治理的单一身份层。

## 快速开始

```bash
# 1. 启动依赖
docker compose up -d

# 2. 设置环境变量
cp .env.example .env

# 3. 运行 migration
sqlx migrate run

# 4. 添加 Service Token
psql $DATABASE_URL -c "INSERT INTO service_tokens (project, token_hash) VALUES ('pandaria', '$(echo -n 'your-service-token' | sha256sum | cut -d' ' -f1)')"

# 5. 启动服务
cargo run -p aspectus-server

# 6. 验证
curl http://localhost:3100/health
```

## API 端点

| 端点 | 方法 | 认证 | 说明 |
|------|------|:--:|------|
| `/health` | GET | 无 | 健康检查 |
| `/metrics` | GET | 无 | Prometheus 指标 |
| `/introspect` | POST | Service Token | Token 自省 (RFC 7662) |
| `/login/lookup` | POST | 无 | 两步登录第一步：邮箱 → 返回关联租户列表（ADR-016） |
| `/login` | POST | 无 | 用户登录 (email+password+tenant_id → JWT，ADR-016 两步法第二步) |
| `/register` | POST | 无 | 用户注册（需 `ASPECTUS_REGISTRATION_ENABLED=true`，**demo/dev only**；生产用 `POST /users`） |
| `/logout` | POST | 无 | 吊销 refresh token |
| `/forgot-password` | POST | 无 | 生成密码重置 token |
| `/reset-password` | POST | 无 | 验证 token + 更新密码 |
| `/tenants` | POST/GET | Service Token | 租户管理 |
| `/tenants/{id}/quotas` | PUT | Service Token | 配额配置 |
| `/service-accounts` | POST/GET | Service Token | 服务账号 |
| `/users` | POST/GET | Service Token | 用户管理 |
| `/users/{id}/suspend` | PUT | Service Token | 挂起/恢复用户 |
| `/roles` | GET | Service Token | 角色列表 |
| `/users/{id}/roles` | POST/DELETE | Service Token | 角色分配 |
| `/api-keys` | POST/GET | Service Token | API Key 管理 |
| `/api-keys/{id}` | GET/DELETE | Service Token | 查询/吊销 Key |
| `/authorize` | POST | 无 | OAuth2 授权 |
| `/oauth/token` | POST | 无 | OAuth2 Token |
| `/clients` | POST/GET | Service Token | OAuth2 Client |
| `/.well-known/jwks.json` | GET | 无 | JWT 公钥 |

完整 API 文档见 [docs/openapi.yaml](docs/openapi.yaml)。

## 两步登录流程（ADR-016）

`alice@example.com` 同一个邮箱可以在多个租户下注册（schema `UNIQUE (tenant_id, email)`）。
为避免歧义、避免枚举，登录采用两步法：

```bash
# Step 1: 查询邮箱关联的租户
curl -X POST http://localhost:3100/login/lookup \
  -H 'Content-Type: application/json' \
  -d '{"email":"alice@example.com"}'

# Response 200 (无论邮箱是否存在都返回 200)
# {
#   "tenants": [
#     { "tenant_id": "org_acme", "tenant_name": "Acme Corp" },
#     { "tenant_id": "org_foo",  "tenant_name": "Foo Industries" }
#   ]
# }

# Step 2: 用户选择租户后，调用 /login 并传 tenant_id
curl -X POST http://localhost:3100/login \
  -H 'Content-Type: application/json' \
  -d '{
    "email": "alice@example.com",
    "password": "secret123",
    "tenant_id": "org_acme",
    "client_id": "pandaria"
  }'

# Response 200 — 含 JWT + user/tenant 上下文 + available_projects
```

如果邮箱只关联一个租户，前端可跳过选择步骤，直接将 lookup 返回的唯一
`tenant_id` 填入 `/login` 请求。

## 登录响应字段

`/login` 与 `/register` 的成功响应额外包含：

| 字段 | 说明 |
|------|------|
| `user` | `{id, email, display_name}` — 避免前端再调一次 `GET /users/{id}` |
| `tenant` | `{id, name}` — `name` 嵌入 JWT payload 的 `tenant_name` claim |
| `available_projects` | 用户 Role 展开后 distinct 出的生态项目列表 |

JWT payload 现在包含 `tenant_name` 声明，客户端可直接读取展示"Acme Corp 的 alice"，
无需额外 API 调用。

## Token 类型

| 前缀 | 类型 | 场景 |
|------|------|------|
| `pk_live_*` | API Key | 长期 Agent SDK |
| `eyJ*` | JWT | 高频服务间调用 |
| `ot_*` | Opaque Token | 需吊销的短期凭证 |
| `rt_*` | Refresh Token | OAuth2 刷新 |

## 创建用户：Demo vs 生产

**Demo/dev 路径**（`ASPECTUS_REGISTRATION_ENABLED=true`）：

```bash
# 1. 管理员手动创建 tenant（SQL）
psql -c "INSERT INTO tenants (id, name) VALUES ('org_acme', 'Acme Corp')"

# 2. 用户自助注册
curl -X POST http://localhost:3100/register \
  -H 'Content-Type: application/json' \
  -d '{
    "email": "alice@example.com",
    "password": "secret123",
    "tenant_id": "org_acme"
  }'
```

**生产路径**（`ASPECTUS_REGISTRATION_ENABLED` 未设置或 = false）：

```bash
# 1. 管理员用 Service Token 创建 tenant
curl -X POST http://localhost:3100/tenants \
  -H "Authorization: Bearer $SERVICE_TOKEN" \
  -d '{"id": "org_acme", "name": "Acme Corp"}'

# 2. 管理员创建 user（包含初始密码）
curl -X POST http://localhost:3100/users \
  -H "Authorization: Bearer $SERVICE_TOKEN" \
  -d '{
    "email": "alice@example.com",
    "password": "secret123",
    "tenant_id": "org_acme"
  }'

# 3. alice 使用两步法登录
# POST /login/lookup → POST /login（见下节）
```

**ADR-016 决策 6**：`/register` 不再自动创建 tenant。传入不存在的 `tenant_id`
会返回 404 并提示使用 `POST /tenants` 创建。这是生产环境的安全护栏，
防止任意用户创建任意 tenant。

## 技术栈

| 维度 | 选型 |
|------|------|
| 语言 | Rust |
| 框架 | axum |
| 数据库 | PostgreSQL |
| 缓存 | Redis |
| Token 签名 | JWT RS256 |
| 密码哈希 | argon2id |
| API 文档 | OpenAPI 3.0 |

## 项目结构

```
Aspectus/
├── crates/
│   ├── aspectus-core/       # 域模型、trait 定义
│   ├── aspectus-auth/       # 认证逻辑、JWT、密码哈希
│   ├── aspectus-server/     # HTTP 服务、路由、Store 实现
│   └── aspectus-client/     # Rust client library (stub)
├── migrations/              # PostgreSQL migration
├── docs/
│   ├── adr/                 # 架构决策记录 (×15)
│   ├── specs/               # 技术规格 (×7)
│   ├── plans/               # 实现计划
│   └── openapi.yaml         # API 文档
└── tests/                   # 集成测试
```

## 开发

```bash
# 运行测试
DATABASE_URL="..." REDIS_URL="..." cargo test --workspace

# 集成测试
DATABASE_URL="..." REDIS_URL="..." cargo test -p aspectus-server --test integration_test

# 代码检查
cargo clippy --all-targets
cargo fmt --all -- --check
```

## 版本

| 版本 | 内容 |
|------|------|
| v0.1.0 | 项目骨架 + DB schema |
| v0.2.x | MVP: `/introspect` + 管理 API |
| v0.3.x | 多项目 scope + 配额 + 性能 |
| v0.4.0 | JWT + Opaque Token |
| v0.5.0 | User + Role 管理 |
| v0.6.0 | OAuth2 Authorization Code |
| v0.7.0 | Refresh Token + OAuth2 Clients |
| v0.8.0 | Metrics + OpenAPI 文档 |

## 架构决策

见 [docs/adr/](docs/adr/) — 15 篇架构决策记录，涵盖 Token 自省、多租户模型、Token 类型、审计日志等关键设计。

## License

MIT
