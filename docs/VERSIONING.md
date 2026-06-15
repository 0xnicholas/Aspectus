# Aspectus — API 版本化策略

> 版本：v1.0.0-rc | 最后更新：2026-06-15

本文档定义 Aspectus API 的版本化承诺和变更流程。自 v1.0.0 起生效。

---

## SemVer 约定

Aspectus 遵循 [Semantic Versioning 2.0.0](https://semver.org)。

| 版本号 | 含义 | 消费者影响 |
|--------|------|-----------|
| **Major** (X.0.0) | Breaking change | 需要修改消费者代码 |
| **Minor** (0.X.0) | 新增功能，向后兼容 | 无需修改代码 |
| **Patch** (0.0.X) | Bug fix，向后兼容 | 无需修改代码 |

---

## `/introspect` 响应格式 —— 稳定性承诺

`POST /introspect` 是 Aspectus 的**最高优先级 API**——每个生态项目在每个请求中调用它。其响应格式遵循严格的向后兼容保证。

### Stable 字段（v1.0.0+）

以下字段在 v1.x 系列中保证存在且类型不变：

| 字段 | 类型 | 说明 |
|------|------|------|
| `active` | `boolean` | token 是否有效 |
| `tenant_id` | `string \| null` | 租户 ID |
| `user_id` | `string \| null` | 用户/服务账号 ID |
| `identity_type` | `string \| null` | `"user"` 或 `"service_account"` |
| `client_id` | `string \| null` | 项目标识（如 `"pandaria"`） |
| `scope` | `string \| null` | 空格分隔的 scope 列表 |
| `token_type` | `string \| null` | `"Bearer"` |
| `exp` | `integer \| null` | Unix 过期时间戳 |
| `quotas` | `object \| null` | per-project 配额限制 |
| `token_format` | `string \| null` | `"api_key"` / `"jwt"` / `"opaque"` |

### 新增字段

Minor 版本可以新增字段。消费者应该：
- 使用 `serde(default)` 或等价的 JSON 反序列化默认值
- **不依赖**尚未在本文档中声明为 stable 的字段

### 废弃字段

Major 版本（v2.0.0+）**可能**删除废弃字段。废弃字段会在 Minor 版本中先标记 `#[deprecated]`，并保留至少 2 个 Minor 版本。

### 示例：向后兼容的消费者代码（Rust）

```rust
#[derive(Deserialize)]
struct IntrospectResponse {
    active: bool,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    // 所有字段使用 serde(default) 以处理新增字段
    #[serde(default)]
    identity_type: Option<String>,
    // ... 其他 stable 字段 ...
    
    // 对未知字段不报错
    #[serde(flatten)]
    _extra: serde_json::Value,
}
```

---

## 管理 API 变更流程

管理 API（Tenant、User、API Key、Role、OAuth2 Client CRUD）遵循以下流程：

1. **新增端点**：Minor 版本新增，不影响已有端点
2. **新增请求/响应字段**：Minor 版本新增，使用 `Option` 或 `#[serde(default)]`
3. **废弃端点**：
   - Minor 版本 N：标记为 deprecated（响应头添加 `Deprecation: true`）
   - Minor 版本 N+2：移除端点
4. **移除字段**：
   - 仅在 Major 版本进行
   - 提前 2 个 Minor 版本发出 deprecation 警告

---

## 数据库 Migration 策略

| 操作 | 版本影响 | 注意事项 |
|------|---------|---------|
| 新增表 | Minor | 不影响已有功能 |
| 新增列 | Minor | 使用 `ADD COLUMN IF NOT EXISTS` |
| 新增索引 | Patch | 使用 `CREATE INDEX CONCURRENTLY` |
| 修改列类型 | Major | 需要数据迁移 |
| 删除列 | Major | 提前 2 个 Minor 版本废弃 |
| 删除表 | Major | 需要确认无消费者使用 |

---

## 客户端兼容性矩阵

| 客户端版本 | v1.0.x | v1.1.x | v2.0.0 |
|-----------|:------:|:------:|:------:|
| aspectus-client v1.0 | ✅ | ✅ | ❌ |
| aspectus-client v1.1 | ✅ | ✅ | ❌ |
| aspectus-client v2.0 | ✅ | ✅ | ✅ |

**原则**：客户端 MAJOR 版本与 Aspectus MAJOR 版本对齐。客户端可以兼容同 MAJOR 或更高 PATCH 的服务端。

---

## v1.0.0 API 冻结声明

自 v1.0.0 起，以下 API 行为进入**长期稳定**：

| 端点 | 稳定性 |
|------|:------:|
| `POST /introspect` | ✅ **Stable** |
| `GET /health` | ✅ **Stable** |
| `GET /metrics` | ✅ **Stable** |
| `GET /.well-known/jwks.json` | ✅ **Stable** |
| `POST /authorize` | ✅ **Stable** |
| `POST /oauth/token` | ✅ **Stable** |
| 管理 API (全部) | ✅ **Stable** |

任何对这些端点的 breaking change 需要 MAJOR 版本升级（v2.0.0）。

---

## 版本发布检查清单

每个版本发布前：

- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --all-targets` 零警告
- [ ] OpenAPI spec 与实际行为一致（手动审查）
- [ ] CHANGELOG.md 更新
- [ ] 如果涉及 schema 变更：migration 已测试且可重复执行
- [ ] 如果涉及 `/introspect` 格式变更：所有消费者已通知
- [ ] Git tag 格式：`v{major}.{minor}.{patch}`
