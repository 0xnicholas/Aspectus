# Contract Testing for `POST /introspect`

> 最后更新：2026-06-21 · 对应测试：`crates/aspectus-server/tests/http_tests/contract_test.rs`

---

## 为什么把契约测试放在 Aspectus？

AGENTS.md 明确要求：

> 契约测试：确保自省响应格式与 Pandaria/Constell 的期望一致

契约测试是**服务端**的责任，不是消费者的责任。原因：

1. **消费者多，服务端一** — Pandaria / Constell / Tokencamp / Heirloom / Emerald 各自有 1 套集成测试。契约写在 Aspectus 一处即可，5 个项目共用。
2. **服务端先变** — 如果 Aspectus 改了 schema，Pandaria 的测试要在 Aspectus 升级后才能跑通。Aspectus 的契约测试**先**失败 → 服务端发版前就知道破坏了契约。
3. **维护更便宜** — 5 份散落的契约测试 → 1 份集中的 schema 验证 + snapshot。

如果契约测试在 Pandaria 仓库里：

- Aspectus 改了 schema，Pandaria CI 才会挂（滞后反馈）
- Constell 等其他消费者要复制 1 套同样的测试（重复造轮）
- Aspectus 团队没有「我的契约现在长什么样」的权威视图

**契约的所有者是 Aspectus。测试也住在 Aspectus。**

---

## 测试套件结构

文件：`crates/aspectus-server/tests/http_tests/contract_test.rs`

| Test | 验证什么 | Snapshot |
|------|---------|:--------:|
| `introspect_active_api_key_contract` | 有效 API Key 的完整响应形状 | `introspect_active_api_key.snap` |
| `introspect_revoked_api_key_contract` | 吊销后必须返回**仅** `{active:false}` | `introspect_revoked_api_key.snap` |
| `introspect_unknown_token_contract` | 未知 token → `{active:false}` | `introspect_unknown_token.snap` |
| `introspect_malformed_token_contract` | 格式错乱 token → `{active:false}` | `introspect_malformed_token.snap` |
| `introspect_active_with_quotas_contract` | `quotas` 子树结构 | `introspect_quotas_subtree.snap` |
| `introspect_missing_service_token_returns_problem_details` | 401 + `application/problem+json` | — |
| `introspect_wrong_service_token_returns_problem_details` | 401 + `application/problem+json` | — |
| `introspect_always_includes_active_field` | `active` 字段始终存在 | — |
| `introspect_inactive_omits_identity_fields` | inactive 时**绝不**泄漏 identity | — |

每个 snapshot 都是**契约本身**。Snapshot 改了 = 契约改了。

---

## 实际契约（来自测试）

### Active 响应（API Key）

```json
{
  "active": true,
  "tenant_id": "<KSUID>",
  "user_id": "<KSUID>",
  "identity_type": "service_account",
  "client_id": "pandaria",
  "scope": "pandaria:session:create pandaria:session:read",
  "token_type": "Bearer",
  "token_format": "api_key"
}
```

**字段保证**（来自 IntrospectResponse struct 的 `skip_serializing_if = "Option::is_none"`）：

| 字段 | 出现条件 | 缺失含义 |
|------|---------|----------|
| `active` | **总是** | 不存在 = 协议错误 |
| `tenant_id` | token 关联 tenant | 不存在 = token 是孤儿（错误） |
| `user_id` | token 关联 identity | service_account 时 = SA id；user 时 = user id |
| `identity_type` | token 关联 identity | `"user"` \| `"service_account"` |
| `client_id` | token 关联 project | = 创建时的 `project` 字段 |
| `scope` | token 有 scope | 空格分隔（OAuth2 / RFC 7662） |
| `token_type` | token 关联类型 | 总是 `"Bearer"` |
| `token_format` | token 签发时记录 | `"api_key"` \| `"jwt"` \| `"opaque"` |
| `exp` | token 有过期时间 | API Key / Opaque 没有 exp；JWT 有 |
| `quotas` | tenant 配置了配额 | 没配 = 省略（不是 `null`） |

### Inactive 响应（吊销 / 过期 / 未知 / 格式错）

```json
{ "active": false }
```

**严格只包含 `active: false`**。任何额外字段（`tenant_id`, `user_id`, `scope`, ...）都是隐私 bug。

### HTTP 状态码矩阵

| 输入 | HTTP Status | Content-Type | Body |
|------|:--:|------|------|
| 有效 Service Token + 任意 subject token | **200** | `application/json` | `IntrospectResponse` |
| 缺失 Service Token | **401** | `application/problem+json` | RFC 7807 |
| 错误 Service Token | **401** | `application/problem+json` | RFC 7807 |

> ⚠️ 注意 RFC 7662：即使 token 无效也返回 **200**（不是 4xx）。只有**调用方**认证失败才是 401。

---

## Volatile 字段处理

`tenant_id` 和 `user_id` 是 KSUID（21 字符随机），每次跑测试都不同。如果直接 snapshot，每次 CI 都会失败。

解决方案：`stable_active_view()` 辅助函数把这两个字段替换成 `<KSUID>` 占位符：

```rust
fn stable_active_view(body: &Value) -> Value {
    json!({
        "active": body["active"],
        "tenant_id": "<KSUID>",
        "user_id": "<KSUID>",
        // ... 其他字段原样保留
    })
}
```

Snapshot 因此保持稳定。但**这两个字段的存在性和类型**仍由 `assert!` 单独验证。

**局限**：snapshot 不能捕捉「`tenant_id` 被改名」vs「`tenant_id` 被删除」的差异。如果想严格捕捉 schema 演变，需要用 `jsonschema` crate + openapi.yaml 派生 schema — 这是后续工作（见 §Future）。

---

## 如何运行

```bash
# 1. 启动依赖
docker compose up -d

# 2. 运行契约测试（仅）
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus \
REDIS_URL=redis://localhost:6380 \
cargo test -p aspectus-server --test http_tests contract_test

# 3. 运行所有 http_tests
DATABASE_URL=... REDIS_URL=... \
cargo test -p aspectus-server --test http_tests

# 4. 运行所有集成测试
DATABASE_URL=... REDIS_URL=... cargo test --workspace
```

---

## 如何更新 Snapshot

如果**故意**改了契约（新增字段、修改类型），需要更新 snapshot：

```bash
# 自动接受所有 pending snapshot 变更（适合 PR review 后批量更新）
cargo insta accept

# 交互式 review（推荐 — 每个变更都看 diff）
cargo insta review
```

更新后提交 `.snap` 文件。**Snapshot 文件必须随代码一起 review**，否则契约变更无迹可循。

### CI 失败时的排查

```
thread 'introspect_active_api_key_contract' panicked at 'snapshot assertion failed'
```

1. `cat crates/aspectus-server/tests/snapshots/contract_test__introspect_active_api_key.snap.new` 查看新输出
2. 如果是有意变更 → `cargo insta accept` 后提交
3. 如果是回归 → 修代码，让 snapshot 重新匹配

---

## 添加新契约测试

以「新增 `aud`（audience）字段」为例：

1. **修改 `IntrospectResponse` struct** — 加 `pub aud: Option<String>` 字段
2. **修改 server 代码** — 让 introspect 路由填充 `aud`
3. **snapshot 自动失败**（这是好事！）— `introspect_active_api_key_contract` 会输出新字段
4. **更新 snapshot** — `cargo insta review`，确认新字段符合预期
5. **更新本文件** — 在「Active 响应」表格加一行
6. **更新 consumer-integration.md** — 通知所有生态项目
7. **跨仓库通知** — 在 Pandaria / Constell 等仓库发 issue，提醒他们升级

---

## 与其他文档的关系

| 文档 | 关系 |
|------|------|
| `docs/openapi.yaml` | IntrospectResponse schema 的**规范定义** |
| `docs/adr/001-token-introspection-rfc7662.md` | 为什么用 RFC 7662，response 字段含义 |
| `docs/consumer-integration.md` §6 | 消费者侧的契约视图（错误处理矩阵） |
| `crates/aspectus-server/tests/http_tests/introspect_test.rs` | HTTP 层 sanity tests（无 snapshot） |
| `crates/aspectus-server/tests/integration_test.rs` | SDK 层 store-level tests |

---

## Future 改进

- [x] **JSON Schema 验证** — 已完成（v0.9.1+）。详见下文 §Schema 验证（`jsonschema`）。
- [x] **JWT 完整覆盖** — 已完成（v0.9.1+）：`introspect_active_jwt_contract` + `introspect_expired_jwt_contract`
- [x] **Opaque token 覆盖** — 已完成（v0.9.1+）：`introspect_active_opaque_contract`
- [ ] **Performance contract** — 加一个 benchmark 验证 p95 < 5ms（ADR-001 承诺）
- [ ] **JWKS schema 验证** — `/.well-known/jwks.json` 也应该有契约测试
- [ ] **Multi-tenant 隔离验证** — 验证 token A 不能看到 tenant B 的任何信息（即使 active=true）

## Schema 验证（`jsonschema`）

除了 snapshot 之外，每次 /introspect 响应还会经过严格 JSON Schema 验证。

Schema 定义（`contract_test::introspect_schema`）：

```json
{
  "type": "object",
  "required": ["active"],
  "properties": {
    "active": { "type": "boolean" },
    "tenant_id": { "type": "string" },
    "user_id": { "type": "string" },
    "identity_type": { "enum": ["user", "service_account"] },
    "client_id": { "type": "string" },
    "scope": { "type": "string" },
    "token_type": { "type": "string" },
    "token_format": { "enum": ["api_key", "jwt", "opaque"] },
    "exp": { "type": "integer", "minimum": 0 },
    "quotas": { "type": "object" }
  },
  "additionalProperties": false,
  "if": { "properties": { "active": { "const": true } } },
  "then": {
    "required": ["tenant_id", "user_id", "identity_type",
                  "client_id", "scope", "token_type", "token_format"]
  }
}
```

**3 个保证**：

| 保障 | 机制 | 捕捉的回归 |
|------|------|-----------|
| 未知字段 | `additionalProperties: false` | 静默新增字段（如不小心返回了内部状态） |
| active=true 必填字段 | `if/then` + `required` | 签名后忘记填充 `client_id` 等 |
| 类型正确 | `type` / `enum` | `active` 变字符串、`token_format` 加新值 |

### Schema 自己的测试（5 个）

为了保证 schema 不是「摆设」，加了：

```rust
schema_accepts_valid_active_response     // valid active response passes
schema_accepts_valid_inactive_response   // {active:false} alone passes
schema_rejects_unknown_field             // catches leaked secret fields
schema_rejects_missing_required_fields_when_active
schema_rejects_wrong_active_type         // active must be boolean
```

这意味着如果 schema 本身写错（比如漏掉 required），这些测试会失败。

### Snapshot vs Schema：双层保护

| 变更类型 | 哪个 catch |
|----------|-----------|
| 字段被**删除** | snapshot（旧 snapshot 找不到匹配） |
| 字段被**重命名** | snapshot |
| 字段被**新增** | schema（`additionalProperties: false`） |
| active=true 缺必填 | schema（`if/then`） |
| 字段**类型变了** | 两者都 catch |
| 字段**值变了**（如 scope 从字符串变数组） | snapshot |
