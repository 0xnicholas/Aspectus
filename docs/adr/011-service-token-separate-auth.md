# ADR-011: Service Token — 独立的内部认证层

> 状态：Accepted
> 日期：2026-05-29
> 来源：[概念与架构设计](../superpowers/specs/2026-05-29-concepts-and-architecture-design.md#service-token内部)

---

## Context

`/introspect` 端点需要认证——不能让任何人随便调用来探测 token 有效性。但调用方是生态项目（Pandaria api-gateway、Tavern server 等），不是人类用户。如何认证这些服务？

关键区分：调用 `/introspect` 的认证（「谁在问」）与被自省的 subject token（「要验证的那个 token」）是两个独立的认证层。

## Decision

**引入 Service Token 概念——每个 Project 持有一个独立的 Service Token，仅用于调用 `/introspect` 端点。**

```
请求：
Authorization: Bearer {service_token}  ← 认证调用方身份（Pandaria api-gateway）
Body: token={subject_token}            ← 待验证的用户/服务账号 token
```

**Service Token 的特点**：
- 每个 Project 恰好一个
- 静态配置（环境变量/配置文件），不通过 API 动态创建
- 短有效期 + 自动轮转（类似 session token 而非 API Key）
- 不被 `/introspect` 自省（Service Token 本身不是 subject token）
- 验证路径：本地缓存的 Service Token 哈希 + 定期从 DB 刷新

**验证流程**：
```
1. 从 Authorization header 提取 service_token
2. sha256(service_token) → Redis 缓存查询
   → 命中：放行，继续处理 subject token
   → 未命中：DB 查询 project 的 service_token_hash → 命中则写缓存
3. 验证 subject token
```

## Alternatives Considered

### Alternative A：复用 subject token 认证调用方

（即：调用 `/introspect` 的 Authorization header 就用待验证的 token 本身）

**拒绝理由**：如果 token 已经泄露，攻击者可以通过 `/introspect` 查询这个 token 的信息（包括 scopes、tenant_id 等），进一步扩大攻击面。两层 token 提供了纵深防御。

### Alternative B：mTLS 认证

**拒绝理由**：mTLS 在基础设施层解决认证问题，但运维复杂度高（证书管理、轮转）。对于纯应用层的 Aspectus，Bearer token 更轻量。且各项目的部署环境差异大（有些在 K8s、有些在 VPS），统一要求 mTLS 不现实。

### Alternative C：每个项目一组 IP whitelist

**拒绝理由**：IP 白名单在云原生环境中极其脆弱（Pods IP 动态变化、NAT 网关）。适合边界防护，不适合服务间认证。

### Alternative D：OAuth2 Client Credentials (类似 Logto 的 M2M)

Logto 中，MachineToMachine 应用通过 Client Credentials flow 获取 access token，然后用这个 access token 调 API。

**保留为未来选项**：Phase 3 引入 OAuth2 后，可以考虑用 Client Credentials 替代静态 Service Token。但 Phase 1 MVP 不需要完整的 OAuth2 token endpoint 和 client 管理。Service Token 是更简单的起点。

## Consequences

**正面**：
- 纵深防御：即使 subject token 泄露，攻击者无法通过 `/introspect` 获取更多信息
- 实现简单：静态配置 + Redis 缓存，无 OAuth2 token endpoint 依赖
- 明确的语义分离：认证调用方 ≠ 验证 subject token

**负面**：
- 每个 Project 需要额外维护一个 Service Token
- Service Token 轮转需要协调（Project 侧更新配置 → Aspectus 侧更新 DB）
- 不是 OAuth2 标准方式，未来迁移到 Client Credentials 需要成本

**缓解措施**：
- Service Token 支持双 key 滚动更新：Aspectus 同时接受 old_key 和 new_key，滚动期内 Project 轮转
- Aspectus client library 封装 Service Token 管理
- 在 ADR 中记录「未来可能迁移到 OAuth2 Client Credentials」，保留扩展路径
