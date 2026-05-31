# ADR-001: Token 自省采用 RFC 7662 语义

> 状态：Accepted
> 日期：2026-05-29
> 来源：[AGENTS.md](../../AGENTS.md#adr-001token-自省用-oauth2-token-introspection-rfc-7662-语义)

---

## Context

Aspectus 的主要消费者不是人类用户，而是 Pandaria 生态中的其他服务（Pandaria api-gateway、Tavern server、Constell、Tokencamp 等）。每个服务在接收外部请求时都需要验证该请求携带的 token 是否有效，并获取关联的 `tenant_id`、`user_id`、`scopes`。

这需要一个标准化的、所有服务都能轻松集成的 token 验证接口。

## Decision

自省端点遵循 **OAuth 2.0 Token Introspection (RFC 7662)** 语义，但不实现完整的 OAuth2 Authorization Server——仅实现 token introspection 所需的子集。

**请求**：
```
POST /introspect
Authorization: Bearer {service_token}
Content-Type: application/x-www-form-urlencoded

token={subject_token}
```

**响应（有效 token）**：
```json
{
  "active": true,
  "tenant_id": "org-acme",
  "user_id": "user-123",
  "identity_type": "user",
  "client_id": "pandaria",
  "scope": "pandaria:session:create pandaria:session:read",
  "token_type": "Bearer",
  "exp": 1717000000,
  "quotas": {
    "pandaria": { "max_concurrent_sessions": 50 }
  }
}
```

**响应（无效 token）**：
```json
{ "active": false }
```

**关键设计点**：
- 无效/过期 token 返回 `{ active: false }` 而非 HTTP 4xx，遵循 RFC 7662 的信息隐藏原则
- 扩展字段（`tenant_id`、`identity_type`、`quotas`）作为 top-level 字段而非嵌套在某个扩展对象中，减少各项目解析复杂度
- `scope` 字段为空格分隔字符串，与 RFC 7662 和 OAuth2 标准一致

## Alternatives Considered

### Alternative A：自定义 RPC 端点

```
GET /verify?token=xxx
→ { tenant_id, user_id, scopes }
```

**拒绝理由**：自定义协议要求每个项目编写专用 HTTP client 代码。RFC 7662 有现成的 OAuth2 client 库支持，接入成本更低。且标准端点有助于未来生态外项目接入。

### Alternative B：gRPC 端点

**拒绝理由**：Pandaria 生态中并非所有项目使用 gRPC（Constell 是 web 应用）。HTTP/JSON 是最通用、最低门槛的集成方式。p95 < 5ms 的性能目标在 HTTP+Redis 缓存下完全可达。

### Alternative C：完整 OIDC UserInfo 端点

**拒绝理由**：UserInfo 端点面向人类用户场景（返回用户 profile），而 Aspectus 的核心场景是服务间 token 验证（返回 tenant/scopes/quotas）。语义不匹配。

### 与 Logto 的对比

Logto 实现了完整的 OIDC Provider，包含 token introspection 端点作为 OAuth2 标准的一部分。Logto 的 introspect 响应也更符合标准 OAuth2 格式（含 `sub`、`client_id` 等）。Aspectus 的区别在于：
- 不实现完整的 Authorization Server，introspect 是独立端点
- 自省端点本身通过 Service Token 认证（Logto 用 client credentials）
- 响应中携带生态特有的 `quotas`、`identity_type` 字段

## Consequences

**正面**：
- 各项目可用标准 OAuth2 client 库调用，无需自定义 HTTP client
- 无效 token 不泄漏信息（统一返回 `active: false`）
- 工业标准降低认知负担

**负面**：
- 不符合 RFC 7662 的扩展字段（`quotas`、`identity_type`）可能让使用严格 RFC 解析器的项目需要额外处理
- 需要明确文档说明哪些 RFC 7662 字段被支持、哪些被省略

**缓解措施**：
- 在 Aspectus client library 中封装解析逻辑，其他项目直接用 client 而无需关心 RFC 细节
- 在 API 文档中明确标注「RFC 7662 subset with extensions」
