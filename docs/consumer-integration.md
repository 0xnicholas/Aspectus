# 消费者接入指南（Aspectus v0.9.0）

> 本文档面向 **Pandaria 生态消费者项目**（Pandaria / Constell / Tokencamp / Heirloom / Emerald 等）的工程师，
> 描述如何把 Aspectus 接入到你项目的 HTTP 网关/服务间调用中。
>
> 当前参考实现：Pandaria `api-gateway`（`crates/api-gateway/src/middleware/auth.rs`）。
>
> 文档版本：v0.9.0 · 2026-06-21
>
> **2026-06-21 更新**：Tavern 已合并入 Pandaria 作为子系统（位于 `pandaria/crates/tavern-*`），不再作为独立生态消费者。本指南不再涉及 Tavern 接入。

---

## 目录

1. [架构与角色](#1-架构与角色)
2. [前置准备](#2-前置准备)
3. [依赖与 Cargo 集成](#3-依赖与-cargo-集成)
4. [环境变量](#4-环境变量)
5. [接入核心：Bearer → /introspect → TenantContext](#5-接入核心bearer--introspect--tenantcontext)
6. [错误处理矩阵](#6-错误处理矩阵)
7. [性能优化：本地 JWT 验签](#7-性能优化本地-jwt-验签)
8. [可观测性](#8-可观测性)
9. [灰度与回滚](#9-灰度与回滚)
10. [验证清单](#10-验证清单)

---

## 1. 架构与角色

Aspectus 在每次外部请求到达消费者时的位置：

```
┌──────────┐    Bearer token     ┌──────────────────┐   POST /introspect   ┌─────────┐
│  Client  │ ──────────────────► │ Consumer Gateway │ ──────────────────► │Aspectus │
│ (CLI/UI) │ ◄────────────────── │ (Pandaria/Constell)│ ◄────────────────── │ /:3100  │
└──────────┘   response          └──────────────────┘  IntrospectResponse └─────────┘
```

**三层认证**（ADR-011）：

| 层 | 凭证 | 谁持有 | 用途 |
|----|------|--------|------|
| L1 | Service Token | 消费者进程（环境变量） | 证明「我是 Pandaria」——调用 `/introspect` 时出示 |
| L2 | Subject Token（API Key / JWT / Opaque） | 终端用户/Agent | 证明「我是用户 U / Service Account S」——每次请求出示 |
| L3 | Quota metadata | Aspectus | 在自省响应中携带，由消费者读取后强制执行 |

**关键不变量**：
- L1 与 L2 是**完全独立**的两个 token（ADR-011 拒绝理由）。L2 泄露不会让攻击者能调 `/introspect`。
- 消费者拿到 IntrospectResponse 后必须**自行缓存**——Aspectus 自带 Redis 缓存用于自身性能，但消费者侧的缓存减少**跨进程**的网络往返。
- 配额（quotas）是**配置而非执行**（ADR-003）。消费者读取 `quotas.pandaria.*` 后自行限流。

---

## 2. 前置准备

接入前你需要从 Aspectus 管理员那里拿到：

| 项 | 说明 | 例子 |
|----|------|------|
| **Service Token 原文** | 一次性出示，**只此一次**。立即存到 secrets manager | `aspectus-dev-pandaria-service-token` |
| **Service Token 对应的 project 枚举值** | 必须严格匹配 OpenAPI `Project` enum 之一 | `pandaria` |
| **Aspectus base URL** | 网络可达的 Aspectus 服务地址 | `http://aspectus.aspectus.svc.cluster.local:3100` |
| **Tenant 配额配置** | 你负责读取的 quota 子树 key | `pandaria` |

获取 Service Token 的 SQL（管理员执行）：

```sql
-- 创建：插入 sha256(token)
INSERT INTO service_tokens (project, token_hash)
VALUES (
  'pandaria',
  encode(sha256('aspectus-dev-pandaria-service-token'::bytea), 'hex')
);
```

**轮转**：建议同时持有旧/新两个 token，先插新、再让消费者切换、再删旧：

```sql
-- 1. 写入新 token
INSERT INTO service_tokens (project, token_hash) VALUES ('pandaria', '<new_hash>');
-- 2. 消费者切换 ASPECTUS_SERVICE_TOKEN → 新值
-- 3. 删除旧 token
DELETE FROM service_tokens WHERE project = 'pandaria' AND token_hash = '<old_hash>';
```

> ⚠️ Pandaria 项目目前使用单 token；如果你的项目要支持轮转，需要先在消费者侧实现双 token 支持。

---

## 3. 依赖与 Cargo 集成

`aspectus-client` 是同步发布的 Rust crate（path 依赖即可，未来会发到 crates.io）。

```toml
# crates/your-gateway/Cargo.toml
[dependencies]
aspectus-client = { path = "../../Aspectus/crates/aspectus-client" }
aspectus-core   = { path = "../../Aspectus/crates/aspectus-core" }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

[features]
# 用 feature flag 隔离 Aspectus 集成，便于灰度
default = ["aspectus-auth"]
aspectus-auth = []
```

Pandaria 的真实写法（参考）：

```toml
# crates/api-gateway/Cargo.toml:31-32
aspectus-client = { path = "../../../Aspectus/crates/aspectus-client", optional = true }
aspectus-core  = { path = "../../../Aspectus/crates/aspectus-core",  optional = true }

# crates/api-gateway/Cargo.toml:40-41
default = ["sqlite", "aspectus-auth"]
aspectus-auth = ["aspectus-client", "aspectus-core", "tenant/aspectus-auth"]
```

**为什么用 feature flag**：
- 灰度期可关闭 feature，回到旧 HMAC 路径
- 单测可禁用 feature，避免 Aspectus 调用
- e2e 测试可通过 `wiremock` 模拟 Aspectus

---

## 4. 环境变量

消费者进程必须设置以下环境变量（**强制项**：`ASPECTUS_SERVICE_TOKEN`）：

| 变量 | 必填 | 默认 | 说明 |
|------|:--:|------|------|
| `ASPECTUS_BASE_URL` | 否 | `http://localhost:3100` | Aspectus 服务根 URL（无尾斜杠） |
| `ASPECTUS_SERVICE_TOKEN` | **是** | — | 来自 Step 2 的 Service Token 原文 |
| `ASPECTUS_TIMEOUT_MS` | 否 | `2000` | 单次 `/introspect` 请求超时 |

最小配置（开发）：

```bash
export ASPECTUS_BASE_URL=http://localhost:3100
export ASPECTUS_SERVICE_TOKEN=aspectus-dev-pandaria-service-token
```

最小配置（生产）：

```bash
export ASPECTUS_BASE_URL=https://aspectus.pandaria.io
export ASPECTUS_SERVICE_TOKEN="$(vault read -field=value secret/aspectus/pandaria/service-token)"
```

Aspectus 侧的 `.env` 用 `SERVICE_TOKENS=pandaria=...` 提供（见 README Step 4）。

---

## 5. 接入核心：Bearer → /introspect → TenantContext

下面给出一个**完整可工作的 Rust/axum 中间件**，直接对齐 Pandaria 的 `auth_middleware` 实现。

### 5.1 类型与客户端初始化

```rust
use std::sync::Arc;
use std::time::Duration;

use aspectus_client::AspectusClient;

#[derive(Debug, Clone)]
pub struct AspectusConfig {
    pub base_url: String,
    pub service_token: String,
    pub timeout_ms: u64,
}

impl AspectusConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            base_url: std::env::var("ASPECTUS_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3100".into()),
            service_token: std::env::var("ASPECTUS_SERVICE_TOKEN")
                .map_err(|_| "ASPECTUS_SERVICE_TOKEN not set")?,
            timeout_ms: std::env::var("ASPECTUS_TIMEOUT_MS")
                .unwrap_or_else(|_| "2000".into())
                .parse()
                .unwrap_or(2000),
        })
    }
}

pub struct AppState {
    pub aspectus: AspectusClient,
    pub tenant_cache: TenantCache,
    // ... 其他状态
}

impl AppState {
    pub fn new(aspectus_config: &AspectusConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(aspectus_config.timeout_ms))
            .build()?;
        let aspectus = AspectusClient::with_reqwest(
            &aspectus_config.base_url,
            &aspectus_config.service_token,
            reqwest_client,
        );
        Ok(Self {
            aspectus,
            tenant_cache: TenantCache::new(),
        })
    }
}
```

### 5.2 TenantContext（消费者侧定义）

```rust
use serde::{Deserialize, Serialize};

/// 注入到 axum Request extensions 的租户上下文。
/// 来自 IntrospectResponse，由消费者业务层读取。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub scopes: Vec<String>,
    pub quotas: serde_json::Value,
}

impl TenantContext {
    /// 从 Aspectus IntrospectResponse 构造。
    /// `quotas` 字段按 project 名字子树读取（如 `quotas["pandaria"]`）。
    pub fn from_introspect(
        tenant_id: String,
        user_id: Option<String>,
        scope: Option<String>,
        project_quotas: Option<&serde_json::Value>,
    ) -> Self {
        Self {
            tenant_id,
            user_id,
            scopes: scope
                .unwrap_or_default()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
            quotas: project_quotas.cloned().unwrap_or(serde_json::json!({})),
        }
    }
}

/// Axum Request extension 包装，避免与 String extension 冲突。
#[derive(Clone, Debug)]
pub struct TenantId(pub String);
```

### 5.3 本地缓存（减少 99% 的 `/introspect` 调用）

```rust
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct CacheEntry {
    ctx: TenantContext,
    inserted_at: Instant,
}

/// token → TenantContext 缓存。
///
/// - TTL = 60s（短于 Aspectus 侧 Redis 缓存，确保配额变更及时生效）
/// - 每 1024 次 lookup 触发一次清理（移除 > 300s 的 entry）
pub struct TenantCache {
    entries: DashMap<String, CacheEntry>,
    counter: AtomicU64,
}

impl TenantCache {
    pub fn new() -> Self {
        Self { entries: DashMap::new(), counter: AtomicU64::new(0) }
    }

    pub fn get(&self, token: &str) -> Option<TenantContext> {
        if self.counter.fetch_add(1, Ordering::Relaxed) % 1024 == 0 {
            let now = Instant::now();
            self.entries.retain(|_, v| now.duration_since(v.inserted_at) < Duration::from_secs(300));
        }
        self.entries.get(token)
            .filter(|e| e.inserted_at.elapsed() < Duration::from_secs(60))
            .map(|e| e.ctx.clone())
    }

    pub fn insert(&self, token: String, ctx: TenantContext) {
        self.entries.insert(token, CacheEntry { ctx, inserted_at: Instant::now() });
    }
}
```

### 5.4 认证 middleware（完整实现）

```rust
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;

use crate::{AppState, TenantContext, TenantId};

fn extract_bearer(req: &Request) -> Result<&str, Response> {
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(json!({"error": "missing_bearer"}))).into_response()
        })
}

/// Aspectus Token Introspection 认证中间件（RFC 7662）。
///
/// 行为：
/// - `/healthz` 跳过认证
/// - 命中本地缓存（TTL 60s）→ 直接放行
/// - 未命中 → POST /introspect（带 retry）→ 注入 TenantContext
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, Response> {
    // 1. 健康检查放行
    if req.uri().path() == "/healthz" {
        return Ok(next.run(req).await);
    }

    // 2. 提取 Bearer token
    let token = extract_bearer(&req)?;

    // 3. 查本地缓存
    if let Some(ctx) = state.tenant_cache.get(token) {
        inject_context(&mut req, &ctx);
        return Ok(instrument(req, &ctx.tenant_id, next).await);
    }

    // 4. 调 Aspectus /introspect（带指数退避重试）
    let introspect = introspect_with_retry(&state.aspectus, token)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "aspectus introspection failed");
            (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "aspectus_unavailable"})))
                .into_response()
        })?;

    // 5. 校验响应
    if !introspect.active {
        return Err(unauthorized("token_inactive"));
    }
    let tenant_id = introspect.tenant_id.clone()
        .ok_or_else(|| unauthorized("missing_tenant_id"))?;

    // 6. 构造 TenantContext（消费者按 project 名字读取 quota 子树）
    let ctx = TenantContext::from_introspect(
        tenant_id.clone(),
        introspect.user_id.clone(),
        introspect.scope.clone(),
        introspect.quotas.as_ref().and_then(|q| q.get("pandaria")),
    );

    // 7. 写缓存 + 注入
    state.tenant_cache.insert(token.to_string(), ctx.clone());
    inject_context(&mut req, &ctx);

    Ok(instrument(req, &tenant_id, next).await)
}

fn inject_context(req: &mut Request, ctx: &TenantContext) {
    req.extensions_mut().insert(TenantId(ctx.tenant_id.clone()));
    req.extensions_mut().insert(ctx.clone());
}

async fn instrument(req: Request, tenant_id: &str, next: Next) -> Response {
    let span = tracing::info_span!(
        "http_request",
        http.method = %req.method(),
        http.uri = %req.uri(),
        tenant_id = %tenant_id,
    );
    next.run(req).instrument(span).await
}

fn unauthorized(reason: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({"error": reason}))).into_response()
}

/// 指数退避重试：最多 2 次，100ms / 200ms。
async fn introspect_with_retry(
    client: &aspectus_client::AspectusClient,
    token: &str,
) -> Result<aspectus_core::introspect::IntrospectResponse, aspectus_client::ClientError> {
    let mut attempts: u32 = 0;
    loop {
        match client.introspect(token).await {
            Ok(resp) => return Ok(resp),
            Err(_) if attempts < 2 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempts - 1))).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### 5.5 挂载到 axum Router

```rust
use axum::{Router, middleware, routing::post};

pub fn build_router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/api/v1/sessions", post(create_session))
        // ... 其他受保护路由
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/healthz", get(health))
        .nest("/api/v1", protected)
        .with_state(state)
}
```

---

## 6. 错误处理矩阵

| Aspectus 端行为 | 消费者侧应返回 | Pandaria 当前实现 |
|------------------|----------------|-------------------|
| `active: false`（token 过期/吊销/格式错） | **401** | 401 `Unauthorized` |
| `200 { active: true }` 但 `tenant_id` 缺失 | **401** | 401 `Unauthorized` |
| `200 { active: true }` 但无当前 project 配额子树 | **403** | 403 `Forbidden("tenant not configured for pandaria")` |
| `/introspect` 网络不可达 / 5xx | **503**（fail-closed） | 503 `ServiceUnavailable` |
| `401` from Aspectus（Service Token 失效） | **503** | 503 `ServiceUnavailable` |
| 客户端没有 Bearer header | **401** | 401 `Unauthorized` |
| `Authorization: Bearer <空>` | **401** | 401 `Unauthorized` |

**fail-closed vs fail-open**：

默认建议 **fail-closed**（Aspectus 不可达 → 503 拒绝请求）。理由：
- 失败期间如果放行，攻击者可以通过 DDoS Aspectus 来绕过认证
- 503 让上游 LB / API gateway 自然重试到健康实例
- Pandaria 当前就是 fail-closed

如果你评估后决定 fail-open（Aspectus 挂了放行），**必须**：
1. 在 `auth_middleware` 顶部加 `MAX_DURATION_UNAVAILABLE` 配置（如 30s）
2. 在 `tracing::warn!` 中记录降级状态
3. 通过 feature flag `fail-open-on-aspectus-down` 显式开启
4. 在 metric 中区分正常 401 vs 降级 200

---

## 7. 性能优化：本地 JWT 验签

`aspectus-client` v0.9.0+ 提供 `verify_jwt()`，**本地 RS256 验签**，完全不走网络。

### 7.1 构造客户端

```rust
use aspectus_client::AspectusClient;

// 方式 1：从环境变量读取（推荐用于生产部署）
//   读取 ASPECTUS_URL 与 ASPECTUS_SERVICE_TOKEN；任何一项缺失返回 ClientError::Parse
let client = AspectusClient::from_env()?;

// 方式 2：显式传入（推荐用于测试或需要动态配置的场合）
let client = AspectusClient::new("http://localhost:3100", service_token);
```

### 7.2 选择验证路径

```rust
// JWT：本地验签（首次调用 fetch JWKS，后续 1h 缓存）
let resp = client.verify_jwt("eyJhbGciOi...").await?;

// API Key / Opaque Token：必须走 /introspect
let resp = client.introspect("pk_live_abc...").await?;

// 智能路由（推荐）：根据 token 前缀自动选择
let resp = client.verify("eyJ..." /* 或 pk_live_* / ot_* */).await?;
```

**何时切换到 `verify_jwt`**：

| 流量特征 | 建议 |
|----------|------|
| JWT 占比 > 90% | **强烈建议切换**——p95 延迟从 ~5ms 降到 < 1ms，Aspectus QPS 降到 10% 以下 |
| JWT + API Key 混合 | 用 `client.verify()` 自动路由 |
| API Key 占比 > 50% | 维持 `/introspect` 即可 |

**已知未优化点**：Pandaria 当前实现**总是**走 `/introspect`，没有利用 `verify_jwt` 优化路径。这是一项后续工作（issue 待开）。

**JWKS 缓存失效**：key rotation 时调 `client.refresh_jwks().await` 强制刷新。

---

## 8. 可观测性

### 8.1 Tracing Span

参考 §5.4 的 `instrument()` 函数：每个受保护请求都会带 `tenant_id` 字段进 trace span。

### 8.2 关键指标（建议消费者侧暴露）

```rust
// 在 auth_middleware 中累加
metrics::counter!("aspectus.introspect.cache.hit").increment(1);
// 或
metrics::counter!("aspectus.introspect.cache.miss").increment(1);
metrics::counter!("aspectus.introspect.active.false").increment(1);
metrics::counter!("aspectus.introspect.unavailable").increment(1);
metrics::histogram!("aspectus.introspect.duration_ms").record(elapsed_ms);
```

**告警阈值建议**：
- `aspectus.introspect.cache.hit_ratio < 0.8` → 缓存配置问题
- `aspectus.introspect.unavailable` 5min 突增 → Aspectus 故障
- `aspectus.introspect.duration_ms.p99 > 50ms` → 网络/Redis 问题

### 8.3 错误日志禁忌

- ❌ **永远不要**打印 `Authorization` 整个 header
- ❌ **永远不要**打印 token 字符串（Bearer 值）
- ✅ 只记录 `tenant_id`、`token_prefix`（前 8 字符）、`scope` 摘要

---

## 9. 灰度与回滚

### 9.1 灰度三阶段

```
阶段 1（双跑）
  - 旧 HMAC/NextAuth 路径默认开启
  - Aspectus 路径放在 `aspectus-auth` feature 下，仅内部测试
  - 验证：双跑期间两种 token 都能通过请求

阶段 2（部分切量）
  - 通过 LB header `X-Auth-Backend: aspectus` 路由指定租户到 Aspectus 路径
  - 其他租户仍在旧路径
  - 观察 1-2 周

阶段 3（完全切换）
  - 移除 HMAC/NextAuth 代码
  - 删除 feature flag
  - 更新监控告警
```

### 9.2 回滚开关

Pandaria 的做法：

```rust
// Cargo.toml
default = ["sqlite", "aspectus-auth"]

// 紧急回滚：
cargo run -p pandaria-server --no-default-features --features sqlite
```

### 9.3 上线检查清单

- [ ] Service Token 已写入 Aspectus DB（`SELECT * FROM service_tokens`）
- [ ] 消费者环境变量 `ASPECTUS_SERVICE_TOKEN` 已配置
- [ ] Aspectus `/health` 返回 200 且 `db.status=ok`
- [ ] 消费者能调通 `curl $ASPECTUS_BASE_URL/introspect -H "Authorization: Bearer $ASPECTUS_SERVICE_TOKEN" -d "token=pk_live_test"`
- [ ] 缓存命中率 > 90%
- [ ] p95 延迟 < 50ms
- [ ] 旧的 HMAC/NextAuth 路径**未删除**（feature flag 仍在）

---

## 10. 验证清单

### 10.1 必跑测试

| 测试 | 期望结果 | 文件 |
|------|----------|------|
| 有效 token + 有效配额 | 200，业务正常 | `e2e_aspectus_auth::test_create_session_with_aspectus_auth` |
| 过期/吊销 token (`active: false`) | 401 | `e2e_aspectus_auth::test_inactive_token_rejected` |
| 有 tenant 但无 `quotas.pandaria` 子树 | 403 | `e2e_aspectus_auth::test_no_pandaria_quota_rejected` |
| Aspectus 网络不可达 | 503 | `e2e_aspectus_unavailable::test_aspectus_unavailable_returns_503` |
| Aspectus 返回 500 | 503 | `e2e_aspectus_unavailable::test_aspectus_server_error_returns_503` |
| 配额耗尽（如 max_concurrent_sessions=1） | 业务 4xx（资源创建被拒） | `e2e_aspectus_quotas::test_session_limit_enforced_from_aspectus_quota` |

### 10.2 手动 curl 验证

```bash
# 1. 假设你已有 Service Token 和一个 API Key
SVC=aspectus-dev-pandaria-service-token
SUBJ=pk_live_your_api_key

# 2. 自检
curl -X POST http://localhost:3100/introspect \
  -H "Authorization: Bearer $SVC" \
  -d "token=$SUBJ"
# 期望：200 + {"active": true, "tenant_id": "...", ...}

# 3. 用 invalid token 验证 401（消费者侧）
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer invalid" \
  -d '{"title":"test"}'
# 期望：401

# 4. 验证健康检查免认证
curl http://localhost:8080/healthz
# 期望：200
```

### 10.3 契约测试（与 Aspectus 自动对齐）

消费者侧的 contract test 应在 CI 中跑：

1. 起 `wiremock` 模拟 `/introspect` 各种响应
2. 验证消费者 middleware 对每种响应都返回正确 HTTP code
3. 验证注入的 `TenantContext` 字段映射正确

Pandaria 已有 3 个此类 e2e 测试作为参考实现。

---

## 附录 A：与 Pandaria 实现的差异

如果你正在做第二个消费者项目（如 Constell），以下差异值得注意：

| 项 | Pandaria 当前实现 | 建议新项目做法 |
|----|------------------|---------------|
| Cache TTL | 60s | 60s（一致） |
| Cache 清理频率 | 每 1024 次 lookup | 每 1024 次 lookup（一致） |
| 重试 | 2 次指数退避 | 2 次指数退避（一致） |
| JWT 本地验签 | **未启用**——总是走 `/introspect` | 启用 `client.verify()` |
| Service Token 轮转 | 单 token | 考虑双 token 滚动支持 |
| 配额执行 | `tenant` crate 内部执行 | 消费者根据 quota 自行实现（如令牌桶） |
| Scope 匹配 | 未做通配符展开 | 建议实现 `pandaria:session:*` 通配符匹配（ADR-006） |

## 附录 B：常见问题

**Q: 消费者进程可以缓存 `IntrospectResponse` 多长时间？**
A: Pandaria 选 60s。Aspectus 侧的 Redis 缓存 TTL = `min(token剩余有效期/10, 300s)`。如果你的 token TTL 较短（如 5min JWT），可以缩短消费者侧缓存到 30s。

**Q: 配额在 `quotas` 的哪个子树？**
A: 你的 project 名字（如 `pandaria`、`constell`）。读 `quotas[<your_project>]`，missing 则按业务策略决定 fail-open/fail-closed。

**Q: Service Token 泄露怎么办？**
A: 立即在 Aspectus DB `DELETE FROM service_tokens WHERE project='<your_project>'`，然后签发新 token + 滚动更新。所有用旧 token 的请求会立即返回 401。

**Q: Aspectus 不可达时如何降级？**
A: 见 §6「fail-closed vs fail-open」。默认建议 fail-closed。

**Q: 一个 tenant 是否可以在多个 consumer 项目（如 Pandaria + Constell）有 API Key？**
A: 可以。`/api-keys` 创建时指定 `project` 字段，每个 key 绑定到单一 (tenant, project, scopes)。

---

## 相关文档

- [README.md](../README.md) — Aspectus 运维快速开始
- [AGENTS.md](../AGENTS.md) — 架构原则
- [ADR-001: Token 自省采用 RFC 7662](adr/001-token-introspection-rfc7662.md)
- [ADR-002: API Key — per-tenant、per-project scoped](adr/002-api-key-per-tenant-per-project.md)
- [ADR-003: 配额配置与执行分离](adr/003-quota-config-vs-enforcement.md)
- [ADR-011: Service Token — 独立的内部认证层](adr/011-service-token-separate-auth.md)
- [docs/openapi.yaml](openapi.yaml) — API 完整 schema
- [Pandaria 真实实现](https://github.com/pandaria/crates/api-gateway/src/middleware/auth.rs) — `auth.rs:25-114`

---

**文档维护者**：Aspectus Team · 反馈：本文档随 Aspectus 版本演进，发现与代码不符请提 PR。
