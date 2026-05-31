# ADR-013: Emerald entity_id 使用 `tenant_id:user_id` 复合映射

> 状态：Accepted
> 日期：2026-05-31
> 来源：[AGENTS.md](../../AGENTS.md#adr-004emerald-的-entity_id-使用-tenant_iduser_id-复合映射)

---

## Context

Emerald 是 Pandaria 生态的记忆系统，通过 `entity_id` 隔离不同实体的记忆。在 Aspectus 引入 per-user 身份之前，Emerald 的 `entity_id` 直接使用 `tenant_id`——即同一租户下所有用户共享记忆。

当 Aspectus 上线后，同一 tenant 下存在多个独立 User。如果继续用 `tenant_id` 作为 `entity_id`，Alice 和 Bob 会共享记忆画像——这违反了隐私隔离原则。

## Decision

**`entity_id` 从 `tenant_id` 升级为 `tenant_id:user_id` 复合格式。**

```
迁移前（当前）：entity_id = "org-acme"
迁移后（Phase 2）：entity_id = "org-acme:user-123"
```

**迁移路径（分阶段）**：

```
Phase 1（当前）：entity_id = tenant_id
  → Emerald 记忆按 tenant 隔离，所有用户共享

Phase 2（Aspectus 上线后）：entity_id = tenant_id:user_id
  → 新 session 使用新格式
  → 旧 Emerald 数据保留原 entity_id，不迁移
```

**不迁移旧数据的原因**：
- Emerald 记忆有 natural decay（不活跃的记忆自然衰减消失）
- 强制迁移旧数据风险高（数据量大、语义变化不可逆）
- 新 entity 从 Phase 2 开始累积自己的记忆，旧共享记忆随时间自然消失

## Alternatives Considered

### Alternative A：全量迁移旧 Emerald 数据

**拒绝理由**：
- Emerald 数据量可能很大，迁移耗时长且有数据损坏风险
- 旧共享记忆迁移后归属不明确（「这段记忆是 Alice 产生的还是 Bob 产生的？」）
- 记忆衰减机制让迁移收益有限——旧数据不迁移也不会造成严重的 UX 问题

### Alternative B：在 Aspectus 中增加 tenant-level identity（不区分用户）

即：Aspectus 只管理 tenant，不管理 user。Emerald 保持 `entity_id = tenant_id`。

**拒绝理由**：违背 Aspectus Phase 3 目标（per-user 身份）。如果 Emerald 不能区分用户，则同一租户下 Agent 的记忆会互相污染——Alice 的 Agent 会「记住」Bob 的对话。

### Alternative C：Emerald 自己管理 entity 隔离，不用 Aspectus 的身份

**拒绝理由**：回到身份孤岛问题。如果每个项目各自管理 entity 映射（Emerald 用 user、Pandaria 用 account、Tavern 用 executor），则同一用户在不同系统间的行为无法关联——这正是 Aspectus 要解决的问题。

## Consequences

**正面**：
- 用户级记忆隔离：Alice 和 Bob 的 Agent 记忆完全独立
- 渐进迁移：零风险（旧数据不动，新数据自然过渡）
- 实现简单：Pandaria 的 `EmeraldMemoryStore` adapter 在构造 `entity_id` 时从 `tenant_id` 改为 `tenant_id:user_id`

**负面**：
- 过渡期存在两种 entity_id 格式：系统需兼容（但 Emerald 本身按 entity_id 隔离，无跨 entity 查询——所以影响可控）
- 旧共享记忆不会自动分配给具体用户——在过渡期内用户可能「丢失」历史记忆

**缓解措施**：
- 过渡期建议设短（如 1-2 个月），让旧记忆自然衰减
- 若业务需要，可提供可选的数据迁移脚本（按 `last_accessed_at` 筛选活跃记忆，由管理员确认后手动迁移）
- Pandaria 在 Phase 2 部署时同步切换 `entity_id` 格式
