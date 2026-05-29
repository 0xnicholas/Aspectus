# Aspectus 概念定义与架构设计

> 日期：2026-05-29
> 状态：已确认
> 基于：AGENTS.md 中的现有设计，经 brainstorming 讨论细化

---

## 1. 核心概念

### Tenant（租户）

生态的顶层命名空间。拥有独立的 User、Service Account、API Key、配额配置。对应一个企业/组织。

### User（人类用户）

属于某个 Tenant 的**人类**成员。

- **身份属性**：email（唯一标识）、password_hash（argon2id）、display_name、avatar_url
- **认证方式**：OAuth2 Authorization Code flow（Phase 3+）
- **授权模型**：通过 Role（全局定义的 scope 命名集合）获得 scopes
- **生命周期**：管理员手动创建/禁用，无自动过期
- **审计语义**：actor = 具体的人，可追责

### Service Account（机器身份）

属于某个 Tenant 的**程序化**身份，代表自动化系统。

- **身份属性**：label、description（无 email，无 password）
- **认证方式**：API Key（唯一方式），OAuth2 Client Credentials（Phase 3+）
- **授权模型**：scopes 直接绑定（不使用 Role）
- **生命周期**：通过 API 或管理员创建，可设 expires_at
- **审计语义**：actor = 系统/流水线，非个人

**关键区别：User 和 Service Account 是两个独立概念，不合并。** 合并会导致语义歧义（audit log 不可追责）、数据污染（大量 null 字段）、安全边界模糊（密码重置 vs API Key 轮转流程不同）、授权模型混乱（Role 对人类有意义，机器只需精确 scope）。

### API Key

长期凭证，由 User 或 Service Account 持有。

- **owner**：指向 User 或 Service Account（`identity_type` + `identity_id`）
- **project**：绑定到特定生态项目（enum：pandaria、tavern、emerald、constell、tokencamp、heirloom）
- **scopes**：必须 ⊆ owner 的有效 scopes（API Key 权限不能超出持有者）
- **存储**：`key_hash = sha256(key)`，原文仅创建时返回一次，丢失 = 必须重新创建
- **显示**：`key_prefix` 用于 UI 展示（如 `pk_live_abc123`）
- **状态**：expires_at（可选过期）、revoked_at（吊销时间）

### Scope & Role

- **Scope 格式**：`{project}:{resource}:{action}`，支持 `*` 通配符
  - 例：`pandaria:session:*`（Pandaria session 的所有操作）、`tavern:workflow:run`
- **Role**：全局定义（非 per-tenant）的 scope 命名集合
  - 例：`agent-developer = [pandaria:*, tavern:*]`
- **User** 通过 Role 获得 scopes；**Service Account** 直接绑定 scopes

### Project

生态中的一个项目，以 enum 形式定义：

- pandaria
- tavern
- emerald
- constell
- tokencamp
- heirloom

每个 Project 持有**恰好一个 Service Token**，用于调用 Aspectus 的 `/introspect` 端点。

### Service Token

各项目调用 `/introspect` 时使用的内部认证 token。与 subject token（用户/服务账号的 token）是**两个独立概念**。Service Token 仅用于认证调用方身份，不被自省。

### Quota

Per-tenant 的资源上限**配置**。

- Aspectus 存储 limit，不追踪使用量
- `/introspect` 响应中返回 limit（仅上限，不含 used）
- 各项目独立追踪使用量、独立执行限流判断
- 配额配置通过管理 API 更新（`PUT /tenants/{id}/quotas`）

**理由**：配额执行需要项目上下文（Tokencamp 知道 token 消耗速度，Pandaria 知道当前活跃 session 数）。Aspectus 只管配置，不管执行——避免成为所有请求的热路径瓶颈。

---

## 2. Token 模型

### Hybrid 方案

| Token 类型 | 格式 | 验证路径 | 吊销能力 |
|-----------|------|---------|---------|
| JWT | Self-contained | 验签 + exp 检查 + Redis 吊销列表 | 通过 jti 加入 Redis 吊销集合 |
| Opaque Token | 随机字符串 | sha256 → Redis 缓存 → PostgreSQL | DB 设 revoked_at，删 Redis 缓存 |
| API Key | 随机字符串 | sha256 → Redis 缓存 → PostgreSQL | 同上 |

- **JWT**：微秒级验证（纯 CPU 验签），适合高频调用的服务间场景
- **Opaque / API Key**：缓存命中 ~1ms，miss ~5-10ms，需要吊销能力时使用

### Service Token（内部）

与 subject token 完全独立。每项目一个，静态配置。用于认证谁在调用 `/introspect`。

---

## 3. /introspect 端点设计

### 请求

```
POST /introspect
Authorization: Bearer {service_token}
Content-Type: application/x-www-form-urlencoded

token={subject_token}
```

### 响应（有效 token）

```json
{
  "active": true,
  "tenant_id": "org-acme",
  "user_id": "user-123",
  "identity_type": "user",
  "client_id": "pandaria",
  "scope": "pandaria:session:* tavern:workflow:run",
  "token_type": "Bearer",
  "exp": 1717000000,
  "quotas": {
    "pandaria": { "max_concurrent_sessions": 50 },
    "tokencamp": { "monthly_tokens": 10000000 }
  }
}
```

### 字段说明

| 字段 | 说明 |
|------|------|
| `active` | token 是否有效 |
| `tenant_id` | 租户 ID |
| `user_id` | User 或 Service Account 的 ID |
| `identity_type` | `"user"` 或 `"service_account"` |
| `client_id` | token 被签发到的目标 Project |
| `scope` | 空格分隔的 scope 列表 |
| `token_type` | 固定 `"Bearer"` |
| `exp` | Unix 时间戳，token 过期时间 |
| `quotas` | per-project 配额上限（仅 limit，不含 used） |

### 无效 token

```json
{ "active": false }
```

遵循 RFC 7662——无效/过期 token 返回 `active: false` 而非 4xx，避免信息泄漏。

---

## 4. 缓存策略

| 缓存对象 | 存储 | TTL | 说明 |
|---------|------|-----|------|
| Opaque/API Key 自省结果 | Redis | min(token 剩余有效期/10, 300s) | 热路径缓存，大幅降低 PostgreSQL 压力 |
| JWT 吊销集合 | Redis Set | JWT 原始过期时间 | key = jti，验证时检查是否在集合中 |
| Quota 配置 | 各项目本地 | 启动时拉取，定期刷新 | 不通过自省热路径传递 |
| Service Token 验证 | Redis | 60s | 避免每次调 /introspect 都查 DB |

---

## 5. 与 AGENTS.md 的差异

| 主题 | AGENTS.md 原设计 | 更新后 |
|------|-----------------|--------|
| User 定义 | 包含「服务账号」 | 拆分为 User + Service Account |
| API Key owner | `created_by: user-123` | owner 可为 User 或 Service Account |
| identity_type | 无 | 自省响应新增 `identity_type` 字段 |
| Quota 返回 | 含 used 数据 | 仅返回 limit |
| JWT 吊销 | 未细说 | Redis 吊销集合 + 短有效期 |
| client_id 语义 | 未明确定义 | = token 被签发到的目标 Project |
| Role | 未明确 per-tenant vs 全局 | 全局定义 |
| Project | 未明确 enum vs 动态 | enum（硬编码） |

---

## 6. 待后续细化

以下内容在本次设计中未完全确定，留待实现计划阶段细化：

- 具体数据库 schema（表结构、索引）
- API Key 格式细节（前缀、长度、编码）
- 具体 Role 定义（有哪些 role，各自包含哪些 scopes）
- Rust crate 结构
- 迁移脚本
- Phase 1 MVP 的具体 API 列表
