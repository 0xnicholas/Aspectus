# ADR-016: 跨租户登录路由与登录响应增强

> 状态：Proposed
> 日期：2026-06-19
> 来源：用户访谈（alice 跨项目注册场景）、v0.9.0 `/login` 与 `/register` 代码 review
> 关联 ADR：[ADR-008](./008-single-layer-multi-tenancy.md)（单层多租户）、[ADR-005](./005-role-global-definition.md)（Role 全局定义）
> 关联实现：`crates/aspectus-server/src/routes/auth.rs`、`crates/aspectus-server/src/routes/oauth.rs`

---

## Context

v0.9.0 上线用户认证后，`/login`、`/register` 暴露了若干**用户体验与数据正确性问题**，源于 Aspectus 的单层多租户模型（ADR-008）未在 UX 层得到完整呈现：

### 问题 1：用户登录后不知道自己在哪个租户

当前 `/login` 响应（`routes/oauth.rs:262`）：

```json
{
  "access_token": "eyJ...",
  "refresh_token": "rt_...",
  "expires_in": 900,
  "token_type": "Bearer",
  "token_format": "jwt"
}
```

**用户拿到 JWT 后，前端必须再调一次 `GET /tenants/{id}` 才能显示"你是 Acme Corp 的 alice"**。这导致：

- alice 登录 Pandaria 后看到的是裸 ID（如 `org_acme_8x7k9m`），体验差
- 多项目 UI 难以展示统一的"用户身份卡"
- 调试时难以快速识别 token 属于哪个组织

### 问题 2：跨租户同邮箱登录无法路由

`alice@acme.com` 同时在 `org-acme` 和 `org-foo`（外部顾问身份）注册。`/login` 接收 `(email, password)` 但不接收 `tenant_id`——Aspectus 内部：

```rust
// routes/auth.rs:79 — 当前实现
let (user_id, tenant_id) = sqlx::query_as(
    "SELECT id, tenant_id, password_hash FROM users WHERE email = $1"
).bind(&login_req.email)...
```

**返回哪条记录是数据库顺序决定的，结果不可预测**。alice 可能在不知情的情况下以错误身份登录。

### 问题 3：email 唯一性检查与 schema 不一致

Schema 是 `(tenant_id, email)` 复合唯一（`migrations/.../001_initial_schema.sql`），但 `/register` 代码查询：

```rust
// routes/auth.rs:227 — 当前实现
let exists: bool = sqlx::query_scalar(
    "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"  // ← 缺少 tenant_id
)
```

**跨租户同邮箱注册会被错误拦截**（schema 允许但代码不允许）。这是 code/schema 不一致的 bug，破坏了跨租户注册的可能性。

### 问题 4：注册后无默认 Role

`/register` 创建 user 后直接 `issue_tokens`，但**未分配任何 Role**：

```rust
// routes/auth.rs:295 — 当前实现
// Create user (no role assignment)
sqlx::query("INSERT INTO users ...").execute(...).await;

// 直接 issue tokens，scope 展开结果为空
crate::routes::oauth::issue_tokens(&state, &user_id, &tenant_id, &client_id).await;
```

**alice 注册后能登录但没有任何 scope**——她在 Pandaria 什么也做不了。

### 问题 5：`/register` 默认行为与 spec 冲突

Spec `v1.0.0-user-oauth2.md §10` 明确：

> v1.0.0 不做的事：用户自助注册 | 管理员创建

但 `/register` 代码默认行为是 `ASPECTUS_REGISTRATION_ENABLED` 未设置时返回 false（即**默认禁用**）——这是正确的，但：

- 文档（README）写的是"用户注册（需 `ASPECTUS_REGISTRATION_ENABLED=true`）"，**未明确这是 demo/dev only**
- `default` tenant 自动创建逻辑（`routes/auth.rs:267`）让任何不传 `tenant_id` 的请求都进入同一个 tenant——**生产环境隐患**

---

## Decision

### 决策 1：两步登录（邮箱 → 选租户 → 密码）

引入新端点 `POST /login/lookup`，第一步只收邮箱，返回该邮箱注册过的所有租户列表：

```rust
// 新端点：POST /login/lookup
#[derive(Deserialize)]
pub struct LoginLookupRequest {
    email: String,
}

#[derive(Serialize)]
pub struct TenantOption {
    tenant_id: String,
    tenant_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo_url: Option<String>,
}

pub async fn login_lookup(...) -> impl IntoResponse {
    let accounts: Vec<TenantOption> = sqlx::query_as(
        "SELECT t.id, t.name, t.logo_url 
         FROM users u JOIN tenants t ON u.tenant_id = t.id 
         WHERE u.email = $1 AND u.is_suspended = false"
    ).bind(&req.email).fetch_all(...).await?;
    
    if accounts.is_empty() {
        return Json(json!({ "tenants": [] }));  // 不泄漏邮箱是否存在
    }
    Json(json!({ "tenants": accounts }))
}
```

**前端流程**：

```
┌─ Step 1: 邮箱 ──────────────────────────────┐
│  alice@acme.com → [下一步]                    │
└──────────────────────────────────────────────┘
              ↓
┌─ Step 2: 选租户（如果只有一个，自动跳过）─┐
│  ● Acme Corp                                │
│  ○ Foo Industries                           │
│  [下一步]                                    │
└──────────────────────────────────────────────┘
              ↓
┌─ Step 3: 密码 ──────────────────────────────┐
│  ●●●●●●●●  → [登录]                         │
└──────────────────────────────────────────────┘
              ↓
返回完整 JWT + user/tenant 信息
```

`/login` 入参增加 `tenant_id`（必填，但当 `lookup` 只返回一个租户时可省略）：

```rust
pub struct LoginRequest {
    email: String,
    password: String,
    tenant_id: String,                    // ← 新增必填
    #[serde(default = "default_client_id")]
    client_id: String,
}
```

### 决策 2：登录响应增加 user/tenant 上下文

`/login` 与 `/register` 响应增强：

```json
{
  "access_token": "eyJ...",
  "refresh_token": "rt_...",
  "expires_in": 900,
  "token_type": "Bearer",
  "token_format": "jwt",
  "user": {
    "id": "user_abc123",
    "email": "alice@acme.com",
    "display_name": "Alice"
  },
  "tenant": {
    "id": "org_acme_8x7k9m",
    "name": "Acme Corp",
    "logo_url": "https://cdn.acme.com/logo.png"
  },
  "available_projects": ["pandaria", "tavern"]
}
```

`available_projects` 来源：用户 Role 展开后 scope 中的 distinct project 集合——告诉前端"这个账号当前有权访问的生态项目"。

### 决策 3：JWT 增加 `tenant_name` claim

修改 `JwtSigner::sign`：

```rust
// 当前
sign(user_id, tenant_id, project, &scopes, IdentityType::User, ttl)

// 改为
sign_with_tenant_info(
    user_id, tenant_id, tenant_name, project, &scopes, IdentityType::User, ttl
)
```

JWT payload：

```json
{
  "sub": "user_abc123",
  "tenant_id": "org_acme_8x7k9m",
  "tenant_name": "Acme Corp",
  "project": "pandaria",
  "scope": "pandaria:session:read ...",
  "identity_type": "user",
  "exp": 1717000900
}
```

客户端拿到 JWT 后**无需再调 API** 即可显示"Acme Corp 的 alice"。`tenant_name` 是声明时快照，不动态同步（避免 token 体积膨胀）。

### 决策 4：修 email 唯一性 bug

`/register` 与 `/login` 的 email 查询改为按 `(tenant_id, email)` 过滤：

```rust
// /register
let exists = sqlx::query_scalar(
    "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id = $1 AND email = $2)"
).bind(&reg.tenant_id).bind(&reg.email).fetch_one(...).await?;

// /login
let row = sqlx::query_as(
    "SELECT id, tenant_id, password_hash FROM users 
     WHERE tenant_id = $1 AND email = $2"  // ← 必须用 tenant_id 限定
).bind(&login_req.tenant_id).bind(&login_req.email).fetch_optional(...).await?;
```

这让**跨租户同邮箱注册真正工作**——`alice@acme.com` 可以在 `org-acme` 和 `org-foo` 各自注册一次。

### 决策 5：注册时分配默认 Role

在 `roles` 表中标识一个 `is_default=true` 的 Role（如 `agent-operator` 或新建 `member`）：

```sql
-- 新 migration
UPDATE roles SET is_default = true WHERE name = 'agent-operator';
```

`/register` 流程修改：

```rust
// 创建 user 后
sqlx::query("INSERT INTO users ...").execute(...).await?;

// 分配默认 role
let default_role_id: Option<String> = sqlx::query_scalar(
    "SELECT id FROM roles WHERE is_default = true AND type IN ('user', 'both') LIMIT 1"
).fetch_optional(...).await?;

if let Some(role_id) = default_role_id {
    sqlx::query(
        "INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3)"
    ).bind(&generate_id()).bind(&user_id).bind(&role_id).execute(...).await?;
}
```

**新用户自动获得基础 Role**——至少有 read 类 scope，能在新项目里"看见东西"。

### 决策 6：`/register` 默认关闭 + `default` tenant 限制

- `ASPECTUS_REGISTRATION_ENABLED` 未设置时返回 `403 Public registration is disabled`（当前已实现）
- 文档明确：此端点仅用于 demo/dev，**生产环境必须通过 `POST /users`（Service Token 认证）由管理员创建用户**
- 删除 `default` tenant 自动创建逻辑——强制 `/register` 必须传 `tenant_id`：

```rust
// 当前（routes/auth.rs:267）
if !tenant_exists {
    // Auto-create the tenant if it doesn't exist  ← 删除
    sqlx::query("INSERT INTO tenants ...").execute(...).await?;
}

// 改为
if !tenant_exists {
    return ProblemDetails::not_found(
        "Tenant not found",
        format!("/tenants/{tenant_id}")
    ).into_response();
}
```

### 决策 7：每个项目继续实现自己的 login UI

**不引入统一的 SSO 门户**（如 `accounts.pandaria.io`）——每个生态项目（Pandaria、Tavern、Constell 等）继续维护自己的 login UI，但**必须**支持两步法（调用 `/login/lookup` → 调用 `/login`）。

理由：

- 不增加新的前端项目部署
- 各项目对自己品牌 UI 有控制权（Daypaw 是 Pandaria 风格，Tavern 是 Tavern 风格）
- 后端 API 标准化即可，前端实现各项目自治

代价：alice 在不同项目间切换时仍需重新登录。但只要后端统一，体验是一致的（同样的两步法、同样的错误处理）。

---

## Alternatives Considered

### Alternative A：单步登录 + 邮箱全局唯一

```rust
// 强制全局 email 唯一
constraint users__email unique (email)
```

**拒绝理由**：

- 破坏 ADR-008「单层多租户」——跨租户身份天然允许同邮箱（alice 在 ACME 和 Foo 都有账号）
- 与 Logto 等成熟 IdP 的设计不符（Logto 也是 `(tenant_id, email)` 复合唯一）
- 强迫用户使用 `alice+acme@gmail.com` 这种别名——用户体验差

### Alternative B：SSO 统一门户（`accounts.pandaria.io`）

独立的前端项目，alice 永远去同一个地方登录，登录后跳回请求方项目。

**拒绝理由**：

- 增加新的前端部署、新的域名、新的品牌维护
- 与现有各项目的 login UI 重复
- 对 Pandaria 生态来说**当前不是瓶颈**——只要后端 API 标准化，前端重复实现的成本可控

**保留为未来选项**：如果未来生态需要跨项目 session 共享（不只是跨项目身份），可重新评估。

### Alternative C：登录响应保持精简，让前端自己查

**拒绝理由**：

- 增加前端 1 次额外 API 调用
- 增加前端逻辑复杂度
- 与 Logto 等 IdP 的"登录响应一次性返回完整上下文"惯例不符

### Alternative D：JWT 不携带 `tenant_name`

**拒绝理由**：

- 客户端必须每次都调 `GET /tenants/{id}` 才能展示用户身份
- 增加 `/introspect` 调用频率（虽然有缓存，但仍是额外路径）
- 审计时难以从 JWT 直接看出 token 属于哪个组织

### Alternative E：`/register` 完全删除

**拒绝理由**：

- 失去 demo / 集成测试的便利
- 强制所有用户通过 `POST /users` 创建——对单租户 demo 场景过于繁重

---

## Consequences

### 正面

1. **用户困惑解决**——登录后立即看到"Acme Corp 的 alice"
2. **跨租户身份路由正确**——`alice@acme.com` 在 ACME 和 Foo 都有账号时，登录流程明确让她选择
3. **新用户可用**——注册即获得默认 Role，能在新项目里"看见东西"
4. **后端 API 标准化**——`/login/lookup` 是 OAuth2 IdP 标准做法（Google、GitHub 都用）
5. **代码与 schema 一致**——email 唯一性检查与 `(tenant_id, email)` 复合唯一约束一致

### 负面

1. **登录多一步**——用户多一次点击（选租户）。当 email 只对应一个 tenant 时，前端可自动跳过此步
2. **`/login/lookup` 端点暴露邮箱存在性**——攻击者可枚举哪些邮箱在 Aspectus 注册过
   - **缓解**：返回空列表与"未找到"使用相同响应（已纳入决策 1 的代码示例）
3. **JWT 体积略增**——`tenant_name` 增加 ~30 bytes
   - **缓解**：15 分钟 TTL 的 access token 影响微乎其微
4. **删除 `default` tenant 自动创建**——破坏 demo 便利
   - **缓解**：提供 `aspectus setup demo` 脚本预先创建 demo tenant

### 中性

1. **前端实现责任落到各项目**——Pandaria/Tavern 各自实现 login UI
2. **`tenant_name` 不动态同步**——如果租户改名，alice 旧的 JWT 仍显示旧名（15 分钟后刷新）

---

## Migration 计划

### Phase 1：后端（无 breaking change）

```bash
# 1. 修改 schema migration —— 加 tenant_name 来源
#    tenants 表已有 name 字段，无 schema 改动

# 2. 修代码
git checkout -b adr-016-login-ux
# - 修 routes/auth.rs：login/lookup + register + login
# - 修 routes/oauth.rs：issue_tokens 增加 user/tenant 信息
# - 修 aspectus-auth/jwt.rs：sign_with_tenant_info
# - 修 routes/auth.rs：删除 default tenant 自动创建
# - 加 migration：roles 表 is_default=true

# 3. 测试
DATABASE_URL=... REDIS_URL=... cargo test --workspace

# 4. 集成测试（testcontainers）
cargo test -p aspectus-server --test integration_test -- login_lookup
```

### Phase 2：前端（各项目自建）

```markdown
Pandaria:
- src/pages/Login.tsx → 实现两步法
- src/api/aspectus.ts → 新增 lookup()
- 测试：`alice@acme.com` 在 ACME/Foo 双注册 → 登录时正确路由

Tavern:
- src/pages/Login.tsx → 复用 Pandaria 的 Aspectus client 实现
- ...

Constell / Tokencamp / Heirloom:
- 同上
```

### Phase 3：文档更新

- `README.md`：更新 `/register` 描述，明确 demo-only
- `AGENTS.md`：在"安全约束"加 1 条"跨租户登录路由"原则
- `docs/openapi.yaml`：增加 `/login/lookup` 端点定义
- `docs/specs/v1.0.0-user-oauth2.md`：补充 §3 登录响应格式

---

## Open Questions

1. **`is_default=true` 的 Role 选哪个？**
   - 现有 seed migration 有 5 个 Role（`tenant-admin`、`agent-developer`、`agent-operator`、`ci-deployer`、`readonly`）
   - 建议：新建 `member` Role（read 权限 + 基础 session:create），避免 `agent-developer` 权限过大
   - **待 v1.0.0 spec 确认**

2. **`/login/lookup` 是否返回 display_name？**
   - 若返回：用户体验好，但暴露 PII（邮箱对应的真实姓名）
   - 若不返回：用户必须选完租户才能看到自己是谁
   - **建议**：不返回，等第二步输密码登录后再返回

3. **两步法是否对所有项目强制？**
   - 严格强制：破坏老客户端（如果有的话）
   - 向后兼容：`tenant_id` 可选，缺省时退化为单步（按 email 查首条）
   - **建议**：v1.0.0 之前兼容老 API，v1.0.0 之后强制

---

## 参考

| 文档 | 说明 |
|------|------|
| [AGENTS.md](../../AGENTS.md) | 总体架构原则 |
| [ADR-008](./008-single-layer-multi-tenancy.md) | 单层多租户模型 |
| [ADR-005](./005-role-global-definition.md) | Role 全局定义 |
| [specs/v1.0.0-user-oauth2.md](../specs/v1.0.0-user-oauth2.md) | User + OAuth2 spec |
| `crates/aspectus-server/src/routes/auth.rs` | 当前 /login、/register 实现 |
| `crates/aspectus-server/src/routes/oauth.rs` | 当前 issue_tokens 实现 |
| `migrations/20260531000001_initial_schema.sql` | users 表 schema |
