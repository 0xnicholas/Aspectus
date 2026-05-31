# ADR-007: Hybrid Token 模型 — JWT + Opaque + API Key

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#2-token-模型)

---

## Context

Aspectus 需要支持三种 token 使用场景：
1. **高频服务间调用**：每个请求都调用 `/introspect`，需要极低延迟（p95 < 5ms）
2. **需要吊销能力**：管理员需要能即时吊销 token（如发现泄露）
3. **长期 Agent SDK 凭证**：类似 API Key，一次创建长期有效，无需 refresh

这三种场景对 token 格式和验证路径有不同需求。单一 token 格式无法同时满足。

## Decision

**采用 Hybrid 方案，三种 token 类型共存：**

| Token 类型 | 格式 | 验证路径 | 吊销能力 | 适用场景 | Phase |
|-----------|------|---------|---------|---------|-------|
| **API Key** | 随机字符串 (带前缀) | sha256 → Redis 缓存 → PostgreSQL fallback | ✅ (DB 设 revoked_at + 删缓存) | 长期 Agent SDK 凭证 | Phase 1 |
| **Opaque Token** | 随机字符串 (256-bit random) | sha256 → Redis 缓存 → PostgreSQL fallback | ✅ (DB 设 revoked_at + 删缓存) | 需吊销能力的短期 token（如 refresh token） | Phase 2+ |
| **JWT** | Self-contained, RS256 签名 | 本地验签 + exp 检查 + Redis 吊销列表 | ✅ (jti 加入 Redis Set) | 高频服务间调用（网关层每个请求鉴权） | Phase 2+ |

**JWT 验证流程**：
```
1. 验证 JWT 签名 (RS256 public key，微秒级)
2. 检查 exp（过期时间）
3. 检查 jti 是否在 Redis 吊销集合中
   → 不在集合中 → active: true（纯 CPU 验签，< 1ms）
   → 在集合中 → active: false
```

**Opaque / API Key 验证流程**：
```
1. sha256(token) 生成 lookup key
2. Redis 查缓存
   → 命中 → 直接返回（~1ms）
   → 未命中 → PostgreSQL 查询（~5-10ms）
      → 找到 → 写入 Redis 缓存 → 返回
      → 未找到 → active: false
```

**缓存 TTL**：`min(token 剩余有效期的 1/10, 300s)`

## Alternatives Considered

### Alternative A：全 JWT

**拒绝理由**：JWT 是 self-contained 的，签发后无法吊销（除非等过期）。虽然可以通过 Redis 吊销集合解决，但每个 JWT 验证都需要额外一次 Redis 查询（查吊销集合），削弱了 JWT 的「零网络调用」优势。

### Alternative B：全 Opaque

**拒绝理由**：每次都查 Redis/DB，p95 延迟达不到 < 5ms。在高 QPS 场景下（如 Pandaria api-gateway 每秒数千请求），Redis 和 DB 压力大。

### Alternative C：全 API Key（随机字符串，无 JWT）

**拒绝理由**：API Key 是长期凭证，适合 Agent SDK。但对于服务间调用——每次鉴权都是同一个 Key，一旦泄露需要轮转——JWT 的短有效期 + 自动 refresh 模式更安全。且 JWT 的零网络验证（验签）在性能上更优。

### 与 Logto 的对比

Logto 的 token 模型是基于 OIDC 标准：
- Access Token：Opaque 或 JWT（可选配置），通过 `/introspect` 或本地验签
- Refresh Token：长期 Opaque，存储在 DB
- Personal Access Token（PAT）：类似我们的 API Key，存储在 `personal_access_tokens` 表

Logto 不区分「高频场景用 JWT、低频场景用 Opaque」——它统一使用 OAuth2 token endpoint 签发，格式是配置选项。

Aspectus 的 Hybrid 方案更细粒度：由签发方（管理 API）根据使用场景决定签发哪种 token，而不是全局配置。这更贴合 Aspectus「自省优先于签发」的设计原则。

## Consequences

**正面**：
- 高性能路径（JWT）：微秒级验证，适合网关层每个请求都鉴权的场景
- 灵活性：不同场景选择最适合的 token 类型
- 吊销能力覆盖：JWT 通过 Redis Set 吊销，Opaque/API Key 通过 DB + 缓存失效

**负面**：
- 实现复杂度高：需要维护三套验证逻辑
- JWT key rotation 需要额外机制（定期轮转签名密钥，更新 public key）
- `/introspect` 端点内部需要根据 token 格式分支处理

**缓解措施**：
- 通过 `token_type_hint`（RFC 7662 的可选参数）或 token 前缀区分类型
- JWT 使用短有效期（5-15 分钟），配合 refresh token 自动续期，减少吊销列表大小
- Phase 1 MVP 仅实现 API Key 验证。Opaque Token 和 JWT 在 Phase 2+ 补充——届时自省端点根据 token 格式走不同验证分支
