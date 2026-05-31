# ADR-015: 实体 ID 格式 — 短 ID (varchar(21))

> 状态：Accepted
> 日期：2026-05-31
> 来源：AGENTS.md 数据模型中的 ID 字段设计，参考 Logto 的 ID 格式

---

## Context

Aspectus 中所有核心实体（Tenant、User、Service Account、API Key、Role、AuditLog）都需要唯一标识符。ID 格式影响：

1. **API 可读性**：URL 中的 ID 出现在日志、调试信息、审计记录中
2. **索引性能**：PostgreSQL B-tree 索引对 ID 类型的查询效率
3. **排序性**：是否需要按创建时间排序
4. **全局唯一性**：是否需要跨系统唯一（分布式场景）

Logto 在所有表中使用 `varchar(21)` 作为主键——这是一种短 ID 格式（类似 Stripe 的 ID）。

## Decision

**所有实体 ID 使用 `varchar` 格式的短 ID，长度 ≤ 25 字符。不使用 UUID。**

具体格式取决于实体类型：

| 实体 | ID 格式 | 示例 |
|------|---------|------|
| Tenant | `org-` + 短 ID | `org-acme` |
| User | `user-` + 短 ID | `user-abc123def` |
| Service Account | `sa-` + 短 ID | `sa-xyz789ghi` |
| API Key | `pk_live_` + 短 ID | `pk_live_abc123def456` |
| Role | `role-` + 短 ID | `role-agent-dev` |
| AuditLog | 短 ID（无前缀） | `log-abc123def456` |

**短 ID 生成方案**：使用 KSUID 或类似的 time-sortable 随机 ID。

```
KSUID: 21 字符，Base62 编码
  - 4 bytes: Unix timestamp (second precision)
  - 16 bytes: random payload
  → 按时间大致有序（适合 B-tree 索引）
  → 全局唯一（无需协调）
  → 人类可读（Base62，无特殊字符）
```

**为什么不使用 UUID v4**：
- UUID v4 为 36 字符（`550e8400-e29b-41d4-a716-446655440000`），在 URL、日志、调试信息中冗长
- UUID 完全随机打散 B-tree 索引，写入性能在大量插入时劣化
- UUID 不带时间信息，无法通过 ID 大致推断创建顺序

**为什么不用自增整数**：
- 自增 ID 暴露实体数量（如 `GET /users/1042` 泄漏用户总数）
- 多实例部署时需要序列协调（或改用 UUID 回退），增加复杂度
- 不适合分布式生态（如果未来 Aspectus 需要多实例写入）

## Alternatives Considered

### Alternative A：UUID v4

**拒绝理由**：URL/日志可读性差，B-tree 索引写入性能劣化。但保留了作为后备方案的可能性——如果 KSUID 库不成熟，UUID v7（time-ordered UUID）可以作为折中。

### Alternative B：自增 BIGSERIAL

**拒绝理由**：暴露实体数量，多实例部署需要全局序列。适合内部工具，不适合对外 API。

### Alternative C：Ulid

**拒绝理由**：Ulid 是 26 字符（比 KSUID 多 5 字符），且使用 Crockford Base32（含 `I`/`L`/`O`/`U` 等易混淆字符）。KSUID 的 Base62（`0-9a-zA-Z`）更直观。

### 与 Logto 的对比

Logto 在所有表中使用 `varchar(21)` 作为 ID 类型。Logto 的 ID 生成使用 `nanoid`（自定义字母表），长度 12-21 字符。我们采纳了 Logto 的「短 ID + varchar」模式，但选择 KSUID（time-sortable）而非 Nano ID（纯随机），因为 time-sortable 对 B-tree 索引更友好。

## Consequences

**正面**：
- URL 友好：ID 短且无特殊字符，适合 RESTful API
- 索引性能：time-sortable 特性让 B-tree 插入集中在新页，减少页分裂
- 可调试：带前缀的 ID 一眼可辨实体类型
- 全局唯一：无需中心化 ID 生成服务

**负面**：
- KSUID 库在 Rust 生态中不如 UUID 成熟（但 `ksuid` crate 已有稳定版本）
- 前缀与 ID 之间用 `-` 分隔——需要注意在某些场景下 `-` 是否被视为分隔符（如 URL path）

**缓解措施**：
- Rust `ksuid` crate 已足够稳定（1.0+），或使用 Rust 原生实现（简单算法，100 行以内）
- 前缀使用 `_` 而非 `-`（如 `pk_live_abc123`）避免与 URL path 冲突
- 预留切换能力：如果 KSUID 不合适，迁移到 UUID v7 的成本可控（varchar 列兼容两种格式）
