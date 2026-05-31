# ADR-009: 审计日志 — 结构化列 vs JSONB

> 状态：Accepted
> 日期：2026-05-29
> 来源：[AGENTS.md](../../AGENTS.md#安全约束)

---

## Context

Aspectus 需要不可变的审计日志来记录所有敏感操作（API Key 创建/吊销、Token 签发、配额变更）。审计日志的核心约束：
1. **不可变**：append-only，无 UPDATE/DELETE
2. **敏感信息隔离**：密钥原文、JWT signature、用户密码绝不能出现在日志中
3. **可查询**：支持按 tenant、actor、action、时间范围查询

关键设计决策：使用结构化列（固定 schema）还是 JSONB（灵活 schema）？

## Decision

**采用结构化列（固定 schema），不使用 JSONB 作为主载荷。**

```sql
create table audit_logs (
  id          varchar(21) primary key,
  tenant_id   varchar(21) not null,
  actor_id    varchar(21) not null,       -- User 或 Service Account ID
  actor_type  identity_type not null,     -- 'user' | 'service_account'
  action      varchar(64) not null,       -- 'api_key.created' | 'token.introspected' | ...
  target_type varchar(32) not null,       -- 'api_key' | 'tenant' | 'service_account'
  target_id   varchar(21) not null,       -- affected resource ID
  metadata    jsonb not null default '{}', -- 额外上下文（不含密钥）
  created_at  timestamptz not null default (now())
);

-- 索引
create index audit_logs__tenant on audit_logs (tenant_id, created_at desc);
create index audit_logs__actor on audit_logs (tenant_id, actor_id, created_at desc);
create index audit_logs__action on audit_logs (tenant_id, action, created_at desc);
```

**设计要点**：
- `action`：固定格式 `{resource}.{verb}`，便于精确查询
- `metadata`（JSONB）：仅用于不影响查询模式的额外上下文（如 `{"scope": "pandaria:session:*", "reason": "manual creation"}`）
- 密钥绝不进入任何字段（`metadata` 中不记录 `key_hash` 原文、JWT claims、密码值）
- 无 UPDATE/DELETE 权限：应用层 + DB 权限层双重保障

## Alternatives Considered

### Alternative A：全结构化列，无 JSONB

所有字段均为固定列（包括额外上下文也拆成固定列）。

**拒绝理由**：审计事件的额外上下文差异大——如 `api_key.created` 需要记录 `project` 和 `scopes[]`，而 `tenant.quota.updated` 需要记录 `before` 和 `after` 值。全部用固定列会导致列数膨胀（20+ 列大多是 NULL），且新增审计事件类型需要添加新列（DDL 变更）。结构化核心列 + JSONB metadata 的组合是最佳平衡点。

### Alternative B：全 JSONB（类似 Logto）

Logto 的审计日志设计：

```sql
create table logs (
  tenant_id  varchar(21),
  id         varchar(21),
  key        varchar(128),       -- 事件类型 key
  payload    jsonb,              -- 所有上下文
  created_at timestamptz
);
```

**优点**：极致灵活，新增审计事件类型无需 schema migration。

**拒绝理由**：
- 无法在 DB 层添加列级约束（如 `actor_id` NOT NULL）
- 查询需要 GIN 索引或 JSONB path 查询，不如 B-tree 索引高效
- 对审计日志来说，灵活性不是优势——审计事件类型应该被严格控制（因为每个事件类型都有合规意义），不应允许随意添加新 shape
- 密钥泄露风险更高：JSONB 中可能意外写入敏感字段，结构化列可以通过 Rust 类型系统在编译期防止

### Alternative C：双表设计（事件表 + 属性表）

```
audit_events: id, tenant_id, action, created_at
audit_attributes: event_id, key, value
```

**拒绝理由**：EAV（Entity-Attribute-Value）反模式。查询需要多次 JOIN 或 pivot，性能和可维护性都很差。

## Consequences

**正面**：
- 类型安全：Rust 编译期保证所有字段正确填充
- 查询性能：B-tree 索引直接覆盖常见查询模式
- 密钥安全：编译期保证敏感字段不会出现在 struct 中（`#[derive(Serialize)]` 不包含 key_hash 等敏感字段）
- Schema migration 是可控的——审计日志是低频写入路径，migration 锁表影响小

**负面**：
- 新增审计事件类型可能需要添加新列（适应新 shape），需要 migration
- 不如 JSONB 灵活——如果一个审计事件有特殊字段（如配额变更的 before/after 值），只能放在 metadata JSONB 中

**缓解措施**：
- `metadata` JSONB 字段提供必要的灵活性（与 Logto 的 JSONB payload 不同——这里是辅助字段，不是主载荷）
- 新增列是低频操作（审计事件类型有限且稳定），通过 migration 正常管理
- 在 Rust 中为每种 action 类型定义专门的 metadata struct，既有 JSONB 的灵活性又有类型安全
