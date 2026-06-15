# Aspectus — 生产就绪路线图

> 日期：2026-06-15
> 当前版本：**v0.8.0**（功能完备，未经验证）
> 目标版本：**v1.0.0**（生产级可用）

---

## 当前状态与差距总结

v0.8.0 实现了生态所需全部核心特性（自省、API Key、JWT、OAuth2、用户、角色、配额、Metrics），但在以下维度存在生产化差距：

| 维度 | 当前 | 目标 | 差距 |
|------|:----:|:----:|------|
| 测试覆盖 | 23 tests | ≥150 tests | 无 HTTP 层测试、无 OAuth 集成测试、无并发测试 |
| 安全性 | 基础 | 生产级 | 缺 rate limiting、PKCE、body size limit、input sanitization |
| 性能验证 | 1 个 micro-bench | 负载测试 + profiling | 无并发压测、无内存分析、无 p95/p99 真实数据 |
| 运维就绪 | Dockerfile | K8s + runbook | 缺 Helm chart、备份策略、告警规则、故障手册 |
| 集成验证 | 零 | Pandaria 端到端 | Pandaria api-gateway 尚未接入 `/introspect` |
| 代码质量 | 可编译 | 一致、可维护 | OAuth 路由内联 SQL、部分 unwrap()、ID 格式不一致 |

---

## 路线图总览

```
v0.8.0 ──→ v0.9.0 ──→ v0.10.0 ──→ v0.11.0 ──→ v1.0.0-rc ──→ v1.0.0
   ✅       测试+安全     性能+韧性    运维+部署     集成+稳定      发布
           (3-4 周)     (2-3 周)    (2-3 周)    (3-4 周)      (1 周)
```

| 版本 | 主题 | 周期 | 优先级 |
|------|------|:--:|:------:|
| [v0.9.0](#v090--测试加固--安全强化) | 测试加固 + 安全强化 | 3-4 周 | 🔴 阻塞 |
| [v0.10.0](#v0100--性能验证--韧性提升) | 性能验证 + 韧性提升 | 2-3 周 | 🟡 高 |
| [v0.11.0](#v0110--运维部署--生产环境) | 运维部署 + 生产环境 | 2-3 周 | 🟡 高 |
| [v1.0.0-rc](#v100-rc--集成验证--稳定化) | 集成验证 + 稳定化 | 3-4 周 | 🔴 阻塞 |
| [v1.0.0](#v100--生产发布) | 生产发布 | 1 周 | 🔴 阻塞 |

**总周期：约 11-15 周**（全职投入）

---

## v0.9.0 — 测试加固 + 安全强化

| 属性 | 值 |
|------|-----|
| **目标** | 测试覆盖率达 80%+，消除已知安全漏洞，代码一致性修复 |
| **周期** | 3-4 周 |
| **阻塞 v1.0.0?** | 是 |

### 0.9.1 测试体系建设（第 1-2 周）

#### HTTP 层集成测试

- [ ] 创建 `tests/http/` 目录，使用 `axum_test` 或 `reqwest` + `testcontainers`
- [ ] **自省端点测试**（至少 12 个用例）：
  - API Key 有效 → `{active: true, ...}`
  - API Key 吊销 → `{active: false}`
  - API Key 过期 → `{active: false}`
  - JWT 有效 → `{active: true, identity_type: "user"}`
  - JWT 吊销 → `{active: false}`
  - Opaque Token 有效 / 过期
  - 格式错误 → `{active: false}`
  - Service Token 缺失 → 401
  - Service Token 错误 → 401
  - 缺失 token 参数 → 422
  - 配额字段存在于响应中（配置后）
  - 缓存命中（两次连续调用返回一致结果）
- [ ] **OAuth2 流程测试**（至少 8 个用例）：
  - 有效凭证 → 授权码 → 交换 access_token + refresh_token
  - 错误密码 → 401
  - 无效 client_id / redirect_uri → 422
  - 过期授权码 → 401（验证 60s TTL）
  - 授权码被重复使用 → 第二次失败
  - Refresh token 轮转 → 旧 token 失效
  - 过期 refresh token → 401
  - Refresh token 被盗用检测（双次使用 → 吊销所有关联 token）
- [ ] **管理 API 测试**（至少 15 个用例）：
  - Tenant CRUD 完整流程
  - User 创建、列表、挂起、解除挂起
  - API Key 创建 → 返回原文一次 → 不再可见
  - API Key 吊销 → 缓存失效
  - Scope 校验：合法 scope 通过，非法 scope 422
  - Role 分配 + scope 展开验证
  - role_type 约束：user-only role 赋给 SA → 422
  - OAuth2 Client 创建 + 列表
  - 审计日志记录（验证关键事件产生日志）
  - 跨租户隔离：租户 A 的 SA 无法创建租户 B 的 API Key（422）

#### 单元测试补强

- [ ] `aspectus-core`：所有 struct 的 JSON 序列化往返测试（`IntrospectResponse` 对 `active: false` 时字段为 null）
- [ ] `aspectus-core`：`Project::FromStr` 对 6 个项目均正确，非法值 → Err
- [ ] `aspectus-core`：`RoleType` 枚举序列化
- [ ] `aspectus-auth`：`ApiKeyCreator` 不暴露原始 key 到 store（store 只接收 hash）
- [ ] `aspectus-auth`：`RedisCache` JSON 序列化往返 + TTL 过期
- [ ] `aspectus-auth`：`build_response` 对 API Key + Opaque + JWT 三种 token 格式输出正确的 `token_format` 字段
- [ ] `aspectus-server`：`ScopeExpander` 对无角色用户返回空字符串
- [ ] `aspectus-server`：`ProblemDetails` 所有变体的 JSON 格式符合 RFC 7807

#### 并发与边界测试

- [ ] **并发吊销**：5 个并发请求同时吊销同一个 API Key → 仅一个成功，其余幂等
- [ ] **并发自省**：100 个并发自省同一有效 API Key → 全部返回 `active: true`，无 5xx
- [ ] **Redis 不可用降级**：Redis 关闭时自省仍可工作（回退到 PostgreSQL）
- [ ] **数据库连接池耗尽**：超过 max_connections 的并发 → 排队或快速失败，不 crash
- [ ] **恶意输入**：超长 token（>64KB）、null bytes、Unicode 绕过、SQL 注入尝试
- [ ] **Scope 展开性能**：100 个 scope 的角色展开 < 5ms

### 0.9.2 安全强化（第 3 周）

#### Rate Limiting

- [ ] 引入 `tower::ServiceBuilder` + 基于 Redis 的滑动窗口 rate limiter
- [ ] `/authorize`：5 次/分钟/IP（防暴力破解）
- [ ] `/oauth/token`：30 次/分钟/IP
- [ ] `/introspect`：10000 次/分钟/service_token（正常流量远低于此，防滥用）
- [ ] 管理 API：100 次/分钟/service_token
- [ ] Rate limit 超限 → `429 Too Many Requests` + `Retry-After` header
- [ ] Rate limit 指标暴露到 `/metrics`

#### OAuth2 安全加固

- [ ] **PKCE (RFC 7636)**：`/authorize` 接收 `code_challenge` + `code_challenge_method=S256`，`/token` 校验 `code_verifier`
- [ ] `redirect_uri` 精确匹配（拒绝部分匹配 / 子路径）
- [ ] 授权码 TTL 从 60s 延长至 300s（PKCE 保护下安全）
- [ ] Refresh token 重用检测：一旦检测到已吊销的 refresh token 被使用 → 吊销该用户所有的 refresh token（token replay 防护）
- [ ] `state` 参数验证（防止 CSRF）

#### 输入校验与防护

- [ ] 请求体大小限制：`/authorize` 和 `/oauth/token` 应用 `DefaultBodyLimit`（当前遗漏）
- [ ] Email 格式校验（`/users` 创建时）
- [ ] Tenant name：非空、≤128 字符、仅允许 `[a-zA-Z0-9_-]`
- [ ] Display name：≤128 字符、无控制字符
- [ ] Scope 列表长度上限：64 个 scope/API Key
- [ ] SQL 注入防护审计：验证所有 `sqlx::query!()` 使用参数绑定（非字符串拼接）
- [ ] Response header 安全：添加 `X-Content-Type-Options: nosniff`、`X-Frame-Options: DENY`

### 0.9.3 代码一致性修复（第 4 周）

- [ ] OAuth 路由（`routes/oauth.rs`）重构：内联 SQL 迁移到 store trait + PgStore 实现
  - `PgAuthorizationCodeStore`：`create_code`、`exchange_code`、`cleanup_expired`
  - `PgRefreshTokenStore`：`create`、`rotate`、`revoke_all_for_user`
  - `PgOAuth2ClientStore`：`create`、`list`、`validate_redirect_uri`
- [ ] 消除所有 `unwrap()` 和 `expect()`：转换为 `anyhow::Result` + `ProblemDetails` 响应（特别是 `main.rs` 第 46-48 行的 JWT panic）
- [ ] ID 生成统一：将 `hex::encode(random)[..21]` 替换为真正的 KSUID（`ksuid` crate 已引入 `Cargo.toml` 但未使用）
- [ ] Redis 客户端复用：`main.rs` 中只创建一个 `redis::Client`，通过 `Arc` 共享

### 验收（v0.9.0）

```
✅ cargo test --workspace 运行 ≥150 个测试，全部通过
✅ cargo tarpaulin（或 cargo-llvm-cov）显示行覆盖率 ≥80%
✅ HTTP 层测试包含 testcontainers（PG + Redis）自动拉起
✅ Rate limit 超限返回 429 + Retry-After
✅ OAuth2 流程强制 PKCE（无 code_challenge → 422）
✅ Refresh token 重用 → 全部用户 token 吊销
✅ cargo clippy --all-targets 零警告
✅ cargo audit 零已知漏洞
✅ 所有 unwrap() / expect() 已消除（测试除外）
```

---

## v0.10.0 — 性能验证 + 韧性提升

| 属性 | 值 |
|------|-----|
| **目标** | 验证性能 SLA，消除单点故障，提升韧性 |
| **周期** | 2-3 周 |
| **阻塞 v1.0.0?** | 否（v1.0.0 可在无此阶段的情况下达到可接受水平，但生产部署前强烈建议） |

### 0.10.1 负载测试（第 1 周）

- [ ] 使用 `k6` 或 `wrk2` 建立负载测试套件
- [ ] **自省负载测试**：
  - 目标：1000 RPS（模拟 6 个项目 × 各自的服务间调用）
  - 指标：p50 < 2ms, p95 < 5ms, p99 < 10ms
  - 场景：100% 缓存命中、50% 缓存命中、0% 缓存命中
  - 验证：零 5xx 错误
- [ ] **OAuth2 负载测试**：
  - 目标：50 RPS（授权码交换 + refresh token 轮转）
  - 指标：p95 < 200ms
- [ ] **管理 API 负载测试**：
  - 目标：20 RPS
  - 指标：p95 < 100ms

### 0.10.2 性能剖析与优化（第 1-2 周）

- [ ] 使用 `tokio-console` + `cargo-flamegraph` 定位热点
- [ ] Scope 展开缓存：为 `ScopeExpander::expand` 添加 Redis 缓存（TTL = 60s），当前每次请求都查询 DB
- [ ] 数据库连接池调优：根据负载测试结果调整 `max_connections` / `min_connections`
- [ ] JSON 序列化优化：评估 `serde_json::to_vec` vs 直接写入 response body
- [ ] 自省响应预计算：将 `IntrospectResponse` 的序列化形式存入 Redis（避免每次反序列化再序列化）

### 0.10.3 韧性（第 2-3 周）

- [ ] **Redis 降级**：Redis 不可用时 `RedisCache` 不 panic，回退到直接查 PostgreSQL
  - 当前行为：`ConnectionManager::new().expect()` → panic
  - 目标：惰性连接 + 重试（指数退避）+ 优雅降级
  - 添加 `circuit_breaker` pattern：Redis 连续失败 N 次 → 跳过缓存 N 秒
- [ ] **数据库重连**：PG 连接断开后自动重连（`sqlx` 默认支持，但需验证）
- [ ] **健康检查增强**：`/health` 端点报告 PG + Redis 连接状态
  - 响应格式：`{ "status": "ok"|"degraded"|"down", "postgres": "ok", "redis": "ok" }`
- [ ] **优雅关闭验证**：确保 shutdown 信号期间完成 in-flight 请求（最多等待 30s）

### 验收（v0.10.0）

```
✅ k6 负载测试：1000 RPS 自省 p95 < 5ms
✅ Redis 关闭后自省仍可工作（降级到 PG）
✅ /health?full=true 返回 PG + Redis 连通性
✅ Scope 展开有缓存层
✅ flamegraph 无热点（无可优化瓶颈）
```

---

## v0.11.0 — 运维部署 + 生产环境

| 属性 | 值 |
|------|-----|
| **目标** | Kubernetes 部署就绪，具备备份、告警、操作手册 |
| **周期** | 2-3 周 |
| **阻塞 v1.0.0?** | 否（v1.0.0 可用 docker-compose 部署） |

### 0.11.1 Kubernetes 部署（第 1 周）

- [ ] Helm chart：`charts/aspectus/`
  - Deployment（3 replicas + anti-affinity）
  - Service（ClusterIP）
  - Ingress（可选）
  - ConfigMap（环境变量）
  - Secret（DB 密码、JWT 密钥、Service Token）
  - HPA（基于 CPU/内存的自动扩缩容）
- [ ] PostgreSQL：外部 PostgreSQL 或 CloudNative PG operator 集成
  - 连接字符串的 Secret 管理
  - TLS 连接支持
- [ ] Redis：外部 Redis 或 Redis Cluster
  - 哨兵模式支持
  - TLS 连接支持
- [ ] `/health` 就绪探针 + 存活探针配置
- [ ] PodDisruptionBudget

### 0.11.2 数据管理（第 2 周）

- [ ] **数据库备份策略文档**：
  - 备份频率：每日全量 + 持续 WAL 归档
  - 保留策略：30 天日备 + 12 个月月备
  - 恢复流程 + 恢复时间目标（RTO < 1h）
- [ ] **Redis 持久化**：
  - RDB 快照频率配置
  - 重启后数据恢复验证
  - 或明确文档说明 Redis 仅作为缓存层，数据丢失可接受（cache aside pattern）
- [ ] **Migration 执行流程**：
  - CI/CD 集成：部署前自动 `sqlx migrate run --dry-run` 验证
  - 回滚策略：每个 migration 对应一个 down migration（或接受不可回滚的设计决策）
  - 大表 migration 的锁策略（`CONCURRENTLY`、分批处理）
- [ ] **审计日志数据治理**：
  - 审计日志保留策略（如 90 天后归档到 S3）
  - 审计日志表分区（按 `created_at` 按月分区）

### 0.11.3 可观测性（第 2-3 周）

- [ ] **Prometheus 告警规则**：
  ```yaml
  # 示例告警
  - alert: IntrospectHighLatency
    expr: histogram_quantile(0.95, introspect_duration_seconds) > 0.01
    for: 5m
  - alert: IntrospectErrorRate
    expr: rate(introspect_errors_total[5m]) > 0.01
  - alert: DatabaseConnectionPoolExhausted
    expr: db_connections_active / db_connections_max > 0.9
  - alert: RedisUnavailable
    expr: redis_up == 0
  - alert: HighRateLimitHits
    expr: rate(rate_limit_exceeded_total[5m]) > 10
  ```
- [ ] **Grafana Dashboard**：
  - 总览：RPS、p50/p95/p99 延迟、错误率
  - 自省详情：缓存命中率、按 token 类型分布
  - Auth 详情：登录成功率、OAuth2 错误分布
  - 管理操作：API Key 创建/吊销趋势
  - 基础设施：PG 连接数、Redis 内存
- [ ] **结构化日志规范**：
  - 统一 trace_id 注入（已有 `tracing`）
  - 日志级别：`error`（需人工介入）、`warn`（降级/边界）、`info`（关键事件）、`debug`（详细）
  - 敏感字段脱敏验证

### 0.11.4 操作文档（第 3 周）

- [ ] **运维手册（Runbook）**：
  - 服务启动/停止/重启流程
  - 常见故障处理：
    - Redis 不可用 → 降级运行
    - PG 主库故障 → 切换到副本
    - JWT 密钥泄露 → 紧急轮换
    - API Key 批量泄露 → 批量吊销
  - 扩容/缩容操作
  - 日志查询方法
- [ ] **安全检查清单**：
  - 定期密钥轮换（JWT signing key、Service Token 每季度）
  - 依赖审计（`cargo audit` 每月 + 关键 CVE 即时响应）
  - 数据库备份验证（每月恢复演练）

### 验收（v0.11.0）

```
✅ Helm chart 可部署到 K8s 集群
✅ HPA 基于负载自动扩缩（验证：负载增加 → pod 数量增加）
✅ Prometheus 告警规则覆盖全部关键指标
✅ Grafana dashboard 导入即用
✅ 运维手册覆盖 ≥5 个故障场景
✅ 备份恢复演练通过（RTO < 1h）
```

---

## v1.0.0-rc — 集成验证 + 稳定化

| 属性 | 值 |
|------|-----|
| **目标** | Pandaria 完全集成 Aspectus，端到端验证，API 冻结 |
| **周期** | 3-4 周 |
| **阻塞 v1.0.0?** | 是——v1.0.0 必须有一个真实消费者验证通过 |

### 1.0-rc.1 Pandaria api-gateway 集成（第 1-2 周）

- [ ] Pandaria api-gateway 修改：
  - 移除 HMAC token 验证逻辑
  - 每次请求调用 `POST /introspect`（携带 Service Token + 用户 API Key/JWT）
  - 解析 IntrospectResponse → 注入 `X-Aspectus-Tenant-Id`、`X-Aspectus-User-Id`、`X-Aspectus-Scopes` headers 到上游
  - 对 `active: false` → 返回 401
  - 对 Aspectus 不可达 → circuit breaker 模式（`active: true` 缓存 + 短暂允许）vs fail-close
- [ ] 端到端测试（Pandaria 侧）：
  - 用有效 API Key 创建 Session → 成功
  - 吊销 API Key → 下一个请求被拒绝
  - 用有效 JWT 创建 Session → 成功
  - 吊销 JWT → 下一个请求被拒绝
  - Aspectus 不可达 → Pandaria circuit breaker 生效
- [ ] 性能测量：
  - 添加 `/introspect` 调用后的 Pandaria 请求增加延迟
  - 目标：p95 增加 < 10ms

### 1.0-rc.2 契约测试（第 2 周）

- [ ] `/introspect` 响应 JSON Schema 写入文件（`schemas/introspect-response-v1.json`）
- [ ] Pandaria api-gateway 侧验证：反序列化 `/introspect` 响应与 Schema 一致
- [ ] `aspectus-client` crate 版本锁定，触发消费者侧 CI 验证
- [ ] 编写消费者集成指南（`docs/guides/integrating-with-aspectus.md`）：
  - Step 1：获取 Service Token
  - Step 2：选择 token 类型（API Key vs JWT vs Opaque）
  - Step 3：实现 `/introspect` 调用（含 Rust / TypeScript / Go 示例）
  - Step 4：处理自省响应（active: false vs active: true）
  - Step 5：错误处理（Aspectus 不可达、超时）

### 1.0-rc.3 API 冻结（第 3 周）

- [ ] 发布 `aspectus-core` v1.0.0-rc crate（API 冻结先行版）
- [ ] 所有 `/introspect` 响应字段标识为 stable：
  - `active`, `tenant_id`, `user_id`, `identity_type`, `client_id`, `scope`, `token_type`, `exp`, `quotas`, `token_format`
- [ ] 管理 API 端点签名稳定化：
  - 每个请求/响应 struct 出具文档
  - 弃用字段标记 `#[deprecated]`（而非直接删除）
- [ ] 版本化策略文档：`docs/VERSIONING.md`
  - SemVer 严格管理
  - `/introspect` 响应格式向后兼容承诺
  - 管理 API 变更流程（deprecation → 2 minor versions → removal）

### 1.0-rc.4 Bug Bash + 文档完善（第 3-4 周）

- [ ] 组织 1 周的 Bug Bash（内部团队）：
  - 探索性测试：随机 API 操作序列、异常输入、错误恢复
  - 所有发现 bug → GitHub issues → 按严重性分类
  - P0（数据丢失/安全）→ 必须修复
  - P1（功能不可用）→ 必须修复
  - P2（体验问题）→ 可推迟到 v1.0.x
- [ ] 文档最终审核：
  - README quick start 可在全新机器上执行通过
  - OpenAPI spec 与实际行为一致（自动校验）
  - ADR 更新到最新状态（特别是与原始计划有差异的部分）
- [ ] CHANGELOG.md 编写

### 验收（v1.0.0-rc）

```
✅ Pandaria api-gateway 完全切换到 Aspectus，HMAC 逻辑已移除
✅ Pandaria 端到端测试通过（创建 Session → 正常，吊销 → 拒绝）
✅ /introspect 契约测试通过（JSON Schema 校验）
✅ 集成指南完成并可由外部开发者按照操作
✅ Bug Bash 发现的所有 P0/P1 bug 已修复
✅ API 冻结声明发布
```

---

## v1.0.0 — 生产发布

| 属性 | 值 |
|------|-----|
| **目标** | 生产部署 + 正式发布 v1.0.0 |
| **周期** | 1 周 |
| **阻塞 v1.0.0?** | 是（最后一个版本） |

### 发布清单

#### 部署

- [ ] 生产环境部署（K8s 集群或 docker-compose，视运维成熟度）
- [ ] 生产数据库 migration 执行（`sqlx migrate run`）
- [ ] Service Token 配置（所有 6 个 Project 的生产 token）
- [ ] JWT 密钥对生成 + 安全存储（不要用 dev test keys）
- [ ] SSL/TLS 证书配置
- [ ] DNS 配置（如 `identity.pandaria.dev`）
- [ ] 监控告警确认（Prometheus + Grafana 正常工作）

#### 验证

- [ ] **烟雾测试**（生产环境）：
  - `/health` → 200
  - Service Token 认证 → 通过
  - API Key 创建 → 返回原文 → 自省 active: true
  - OAuth2 登录 → 获取 JWT → 自省 active: true
  - 管理员操作（创建 Tenant、User）→ 正常
- [ ] **性能基线**（生产环境）：
  - 自省 p95 < 5ms
  - 与 staging 环境性能对比无显著差异
- [ ] **Pandaria 确认**：生产流量通过 Aspectus 验证

#### 发布

- [ ] Git tag `v1.0.0`
- [ ] Crates.io 发布：`aspectus-core` v1.0.0
- [ ] GitHub Release 撰写（release notes + 升级指南）
- [ ] `ROADMAP.md` 更新（v1.0.0 标记为 ✅）
- [ ] 内部通知：所有生态项目团队

#### 回滚预案

- [ ] Pandaria 保留 2 小时的 Aspectus 不可达 → 回退到 HMAC 验证的能力
- [ ] 数据库 migration 已备份（pg_dump）
- [ ] 回滚决策树文档化

### 验收（v1.0.0）

```
✅ 生产环境运行中，无 P0/P1 告警
✅ Pandaria 生产流量正常通过 Aspectus 验证
✅ 24 小时运行无 5xx、无 crash、无内存泄漏
✅ Git tag v1.0.0 已推送
✅ 所有生态项目团队已收到通知
```

---

## v1.0.x — 发布后（展望）

| 版本 | 内容 |
|------|------|
| v1.0.1 | 生产反馈驱动的 bug fix |
| v1.0.2 | Tavern 集成 |
| v1.0.3 | Constell + Tokencamp 集成 |
| v1.1.0 | Heirloom 数据级授权接入 |
| v1.2.0 | Emerald entity_id 迁移（`tenant_id:user_id`） |
| v1.3.0 | Service Account Role（SA 可通过 Role 获得 scope） |

---

## 风险与依赖

| 风险 | 影响 | 缓解 |
|------|------|------|
| Pandaria 集成复杂度过高 | 延迟 v1.0.0 发布 | v1.0.0-rc 阶段优先完成集成 |
| 生产环境 Redis 不可用 | `/introspect` 延迟增加 | v0.10.0 确保降级路径 |
| JWT 密钥管理不当 | 安全事件 | v0.11.0 编写密钥轮换 SOP |
| 测试覆盖未达 80% | 线上缺陷风险 | v0.9.0 阻塞项，不通过不前进 |
| 生态项目团队未准备好接入 | v1.0.0 发布无消费者 | v1.0.0-rc 阶段与 Pandaria 团队同步 |

---

## 附录：版本对照

| 原计划（ROADMAP.md）| 实际交付（当前状态）| 新计划 |
|---|---|---|
| v0.4.0 JWT + Opaque | v0.4.0 ✅ | — |
| v0.5.0 User + Role | v0.5.0 ✅ | — |
| v0.6.0 OAuth2 | v0.6.0 ✅ | — |
| v0.7.0 Refresh Token | v0.7.0 ✅ | — |
| v0.8.0 Metrics | v0.8.0 ✅ | — |
| v1.0.0（原计划：User + OAuth2 + API 冻结）| 功能已吸收至 v0.5-v0.7 | v1.0.0 重新定义为生产发布 |

---

*本文档在 v0.9.0 启动时更新为执行计划。各阶段在开始前可进行细节调整。*
