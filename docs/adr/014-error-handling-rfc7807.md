# ADR-014: API 错误响应采用 RFC 7807 Problem Details

> 状态：Accepted
> 日期：2026-05-31
> 来源：[AGENTS.md](../../AGENTS.md#错误处理)

---

## Context

Aspectus 的所有 API（管理 API + `/introspect`）需要统一的错误响应格式。不同 API 的错误场景多样：

- **管理 API**：认证失败（401）、授权失败（403）、资源不存在（404）、参数校验失败（422）
- **自省端点**：遵循 RFC 7662，无效 token 返回 `{ active: false }`（200），但自省端点本身的认证失败需要错误响应

各项目（Pandaria、Tavern 等）需要能可靠地解析和区分错误类型，而不是依赖 HTTP status code 加非结构化 body。

## Decision

**所有 API 错误（`/introspect` 的自省结果除外）返回 RFC 7807 Problem Details 格式。**

```json
// 认证失败
HTTP 401
Content-Type: application/problem+json

{
  "type": "https://aspectus.dev/errors/unauthorized",
  "title": "Unauthorized",
  "status": 401,
  "detail": "Invalid or missing Service Token",
  "instance": "/introspect"
}
```

```json
// 参数校验失败
HTTP 422
Content-Type: application/problem+json

{
  "type": "https://aspectus.dev/errors/validation-failed",
  "title": "Validation Failed",
  "status": 422,
  "detail": "Field 'project' must be one of: pandaria, tavern, emerald, constell, tokencamp, heirloom",
  "instance": "/api-keys",
  "errors": [
    { "field": "project", "message": "invalid value 'unknown-project'" }
  ]
}
```

**设计要点**：

| 场景 | HTTP Status | type URI | 说明 |
|------|------------|----------|------|
| 认证失败 | 401 | `.../errors/unauthorized` | 缺失或无效的 Service Token、API Key |
| 授权失败 | 403 | `.../errors/forbidden` | 已认证但无权限执行操作 |
| 资源不存在 | 404 | `.../errors/not-found` | tenant/user/api-key 不存在 |
| 参数校验失败 | 422 | `.../errors/validation-failed` | 请求参数不合法 |
| 冲突 | 409 | `.../errors/conflict` | 如重复创建同名 tenant |
| 内部错误 | 500 | `.../errors/internal-error` | 不暴露内部细节，`detail` 为通用消息 |

**自省端点的特殊处理**：
- `/introspect` 对无效 subject token 返回 `HTTP 200 { active: false }`（遵循 RFC 7662，非 RFC 7807）
- `/introspect` 本身的认证失败（无效 Service Token）返回 RFC 7807 401 错误
- `/introspect` 本身的参数缺失（无 `token` 参数）返回 RFC 7807 422 错误

## Alternatives Considered

### Alternative A：纯 HTTP Status Code + 空 body

**拒绝理由**：无法传递足够的诊断信息。调用方只知道「出错了」但不知道具体原因，排查困难。对于管理 API（人工操作），清晰的错误消息至关重要。

### Alternative B：自定义错误格式

```json
{ "error": "unauthorized", "message": "Invalid token" }
```

**拒绝理由**：每个项目需要写自定义解析逻辑。RFC 7807 是标准（`application/problem+json`），有成熟的客户端库支持。遵循标准降低生态集成成本。

### Alternative C：Logto 的错误格式

Logto 使用 OAuth2 标准错误响应（`application/json`）：
```json
{ "error": "invalid_client", "error_description": "Client authentication failed" }
```

**部分借鉴**：Logto 的格式也是标准化的，但 RFC 7807 更通用（不仅限于 OAuth2 场景），且提供了 `type` URI 和 `instance` 字段，更适合管理 API 的多样错误场景。我们采纳 RFC 7807 作为统一格式，对 OAuth2 特定的错误（Phase 3 引入）可复用 `error` + `error_description` 字段作为 RFC 7807 的扩展属性。

## Consequences

**正面**：
- 标准格式：各项目可用现成的 RFC 7807 解析库
- 自描述：`type` URI 指向错误文档，`instance` 指出具体请求路径
- 可扩展：`errors` 数组（校验失败）、自定义扩展属性灵活附加

**负面**：
- 对于简单错误场景，RFC 7807 显得略重（4 个必填字段）
- `type` URI 需要可访问的错误文档页面（可先指向项目 README 或 wiki）

**缓解措施**：
- Aspectus client library 封装错误解析，各项目无需手动处理 JSON
- `type` URI 短期内使用 `https://aspectus.dev/errors/...`（占位），后续替换为实际文档 URL
- 内部错误（500）的 `detail` 统一为通用消息，`trace_id` 通过响应 header 传递（关联服务端日志）
