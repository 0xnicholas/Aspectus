# Aspectus

Pandaria 生态的统一身份与多租户管理服务。

## 概述

Aspectus 是 Pandaria 生态的单一身份源，为所有项目提供统一的 `tenant_id`、用户认证、API Key 管理、Token 自省和租户配额配置。

当前生态中每个项目各自管理身份（Pandaria 用 HMAC token、Emerald 用 API Key、Constell 用 NextAuth）。Aspectus 将这些分散的身份孤岛统一为一个可审计、可治理的单一身份层。

> **2026-06-21 更新**：Tavern 已合并入 Pandaria 作为子系统（位于 `pandaria/crates/tavern-*`），不再作为独立生态消费者。Aspectus 侧不再为 Tavern 维护独立的 Project 枚举值、scope、Service Token。

## 快速开始

```bash
# 1. 启动 PostgreSQL + Redis（端口映射为 5433/6380）
docker compose up -d

# 2. 设置环境变量
cp .env.example .env
# 编辑 .env，填入 DATABASE_URL/REDIS_URL 与 ASPECTUS_ADMIN_SERVICE_TOKEN
# 示例：
#   DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus
#   REDIS_URL=redis://localhost:6389
#   ASPECTUS_ADMIN_SERVICE_TOKEN=change-me-in-dev

# 3. 运行 migration
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus sqlx migrate run

# 4. 生成 JWT 密钥（可选；代码内置 dev test key，仅本地开发可跳过）
./scripts/generate-jwt-keys.sh
# 将 jwt_private.pem / jwt_public.pem 路径或内容写入 .env

# 5. 启动后端服务
cargo run -p aspectus-server

# 6. 启动管理控制台（新终端）
cd console
cp .env.example .env
# 填入 VITE_API_BASE=http://localhost:3100 与 VITE_SERVICE_TOKEN（同 admin service token）
npm install
npm run dev
# 控制台默认在 http://localhost:5180/

# 7. 验证后端健康检查
curl http://localhost:3100/health
```

## API 端点

| 端点 | 方法 | 认证 | 说明 |
|------|------|:--:|------|
| `/health` | GET | 无 | 健康检查 |
| `/metrics` | GET | 无 | Prometheus 指标 |
| `/openapi.yaml` | GET | 无 | OpenAPI 3.0 文档（无需认证，集成方可自服务发现） |
| `/docs` | GET | 无 | Swagger UI 交互式文档（同上） |
| `/introspect` | POST | Service Token | Token 自省 (RFC 7662) |
| `/token` | POST | Service Token | 签发 JWT/Opaque access token |
| `/token/revoke` | POST | Service Token | 吊销 API Key / Opaque token |
| `/login/lookup` | POST | 无 | 两步登录第一步：邮箱 → 返回关联租户列表（ADR-016） |
| `/login` | POST | 无 | 用户登录 (email+password+tenant_id → JWT，ADR-016 两步法第二步) |
| `/register` | POST | 无 | 用户注册（需 `ASPECTUS_REGISTRATION_ENABLED=true`，**demo/dev only**；生产用 `POST /users`） |
| `/logout` | POST | 无 | 吊销 refresh token |
| `/forgot-password` | POST | 无 | 生成密码重置 token |
| `/reset-password` | POST | 无 | 验证 token + 更新密码 |
| `/tenants` | POST/GET | **Admin** Service Token | 租户管理 |
| `/tenants/{id}` | GET | **Admin** Service Token | 租户详情 |
| `/tenants/{id}/quotas` | PUT | **Admin** Service Token | 配额配置 |
| `/service-accounts` | POST/GET | **Admin** Service Token | 服务账号 |
| `/service-accounts/{id}` | GET | **Admin** Service Token | 服务账号详情 |
| `/users` | POST/GET | **Admin** Service Token | 用户管理 |
| `/users/{id}` | GET | **Admin** Service Token | 用户详情 |
| `/users/{id}/suspend` | PUT | **Admin** Service Token | 挂起/恢复用户 |
| `/users/{id}/scopes` | GET | **Admin** Service Token | 用户有效 scope 列表 |
| `/users/{id}/roles` | GET/POST/DELETE | **Admin** Service Token | 查询/分配/移除用户角色 |
| `/users/{id}/change-password` | POST | 无 | 用户自助修改密码（需当前密码，受 rate limit） |
| `/roles` | GET/POST | **Admin** Service Token | 角色列表 / 创建自定义角色 |
| `/roles/{id}` | GET/PUT/DELETE | **Admin** Service Token | 角色详情 / 更新 / 删除自定义角色 |
| `/api-keys` | POST/GET | **Admin** Service Token | API Key 管理 |
| `/api-keys/{id}` | GET/DELETE | **Admin** Service Token | 查询/吊销 Key |
| `/service-tokens` | POST/GET | Admin Service Token | 生态项目 Service Token 管理 |
| `/service-tokens/{project}` | GET/DELETE | Admin Service Token | 查询/吊销 Service Token |
| `/service-tokens/{project}/rotate` | POST | Admin Service Token | 轮换 Service Token |
| `/audit-logs` | GET | Admin Service Token | 审计日志查询 |
| `/authorize` | POST | 无 | OAuth2 授权 |
| `/oauth/token` | POST | 无 | OAuth2 Token |
| `/clients` | POST/GET | **Admin** Service Token | OAuth2 Client |
| `/.well-known/jwks.json` | GET | 无 | JWT 公钥 |

> **认证术语**：
> - **Service Token**：消费项目（如 Pandaria）的 service token，用于 `/introspect`、`/token`、`/token/revoke`
> - **Admin Service Token**：`ASPECTUS_ADMIN_SERVICE_TOKEN` 环境变量配置，专门用于所有 `/tenants`、`/users`、`/api-keys` 等管理端点。**消费项目的 service token 不能调用管理端点**（见 AGENTS.md 安全约束 7）

完整 API 文档见 [docs/openapi.yaml](docs/openapi.yaml)。

## 管理控制台

项目内置 React + Vite 管理控制台，源码在 `console/`。构建产物会被服务端嵌套在 `/admin` 路径下。

```bash
cd console
cp .env.example .env
# 必须配置：
#   VITE_API_BASE=http://localhost:3100
#   VITE_SERVICE_TOKEN=你的 admin service token
npm install
npm run dev   # http://localhost:5180/
```

> ⚠️ **安全警告**：`VITE_SERVICE_TOKEN` 会被打包进前端 bundle。仅允许在内部网络或已有身份验证的反向代理后使用。若需公网暴露，请改用 BFF 反向代理或在服务端注入 Authorization header。

> **生态项目接入**（Pandaria / Constell / Tokencamp / Heirloom / Emerald）：如何把 Aspectus 接入你的服务网关，参见 [消费者接入指南](docs/consumer-integration.md)。涵盖完整中间件实现、错误处理矩阵、本地 JWT 验签优化、灰度与回滚。

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
    "password": "Secret123",
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

## 安全配置

密码策略与登录锁定通过环境变量配置，所有变量均有默认值：

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `ASPECTUS_PASSWORD_MIN_LENGTH` | `8` | 最小长度 |
| `ASPECTUS_PASSWORD_REQUIRE_UPPERCASE` | `true` | 要求大写字母 |
| `ASPECTUS_PASSWORD_REQUIRE_LOWERCASE` | `true` | 要求小写字母 |
| `ASPECTUS_PASSWORD_REQUIRE_DIGIT` | `true` | 要求数字 |
| `ASPECTUS_PASSWORD_REQUIRE_SPECIAL` | `false` | 要求特殊字符 |
| `ASPECTUS_LOGIN_LOCKOUT_THRESHOLD` | `5` | 连续失败几次后锁定账户 |
| `ASPECTUS_LOGIN_LOCKOUT_DURATION_SECS` | `1800` | 锁定持续时间（秒） |

被锁定的账户可通过 `POST /users/{id}/unlock`（admin service token）手动解锁，或在锁定时间到期后自动解锁。

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
    "password": "Secret123",
    "tenant_id": "org_acme"
  }'
```

**生产路径**（`ASPECTUS_REGISTRATION_ENABLED` 未设置或 = false）：

```bash
# 1. 管理员用 Admin Service Token 创建 tenant
#    注：tenant id 由服务端自动生成 KSUID，响应中返回。
#    name 需匹配 [a-zA-Z0-9_-]{1,128}，不含空格。
curl -X POST http://localhost:3100/tenants \
  -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "acme-corp"}'
# → 201 { "id": "3FoUhlWdNrv5PQODgLCJvnFhmkY", "name": "acme-corp", ... }

# 2. 管理员创建 service-account（API Key 的所有者）
TENANT_ID=3FoUhlWdNrv5PQODgLCJvnFhmkY
SA_RESP=$(curl -s -X POST http://localhost:3100/service-accounts \
  -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"tenant_id\":\"$TENANT_ID\",\"label\":\"alice-bot\",\"project\":\"pandaria\"}")
SA_ID=$(echo "$SA_RESP" | python3 -c "import json,sys; print(json.load(sys.stdin)['id'])")

# 3. 管理员创建 user（包含初始密码）
curl -X POST http://localhost:3100/users \
  -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"email\": \"alice@example.com\",
    \"password\": \"Secret123\",
    \"tenant_id\": \"$TENANT_ID\"
  }"

# 4. 管理员为 service-account 签发 API Key（scopes 必填）
curl -X POST http://localhost:3100/api-keys \
  -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"tenant_id\": \"$TENANT_ID\",
    \"project\": \"pandaria\",
    \"service_account_id\": \"$SA_ID\",
    \"scopes\": [\"pandaria:session:create\", \"pandaria:session:read\"]
  }"
# → 201 { "key": "pk_live_xxxxx", ... } （key 仅此一次返回）

# 5. alice 使用两步法登录
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
# 1. 启动依赖
docker compose up -d

# 2. 运行 migration
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus sqlx migrate run

# 3. 运行全部测试（包含集成测试）
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus \
  REDIS_URL=redis://localhost:6380 \
  cargo test --workspace

# 4. 代码检查
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 5. 前端检查
cd console && npm run lint && npm run build
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
