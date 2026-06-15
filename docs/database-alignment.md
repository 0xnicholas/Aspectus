# Aspectus — 数据库对齐方案

> 参见主设计文档：[pandaria/docs/database-design.md](../../pandaria/docs/database-design.md)

## 定位

Aspectus 是 Pandaria 生态的**单一身份源**。在统一数据库方案中，Aspectus 的数据库 (`aspectus`) 是 identity schema 的权威存储，管理所有 Tenant、User、API Key、Role、Scope 和审计日志。

## 当前状态：已对齐

Aspectus 的数据库设计已是生态中最完整的。**当前所有表无需任何变更**。

| 表 | 状态 | 说明 |
|----|:--:|------|
| `tenants` | ✅ | 生态所有 tenant 的权威源 |
| `users` | ✅ | 含 argon2id 密码哈希、suspension |
| `service_accounts` | ✅ | 机器账号 |
| `api_keys` | ✅ | per-tenant, per-project scoped |
| `scopes` | ✅ | 格式 `{project}:{resource}:{action}` |
| `roles` | ✅ | 支持 user/service_account/both 三种类型 |
| `roles_scopes` | ✅ | 角色-权限关联 |
| `users_roles` | ✅ | 用户-角色关联，带 role_type 约束检查 |
| `service_tokens` | ✅ | 项目间内部认证 |
| `oauth2_clients` | ✅ | OAuth2 客户端 |
| `authorization_codes` | ✅ | 授权码 |
| `refresh_tokens` | ✅ | 刷新令牌 |
| `audit_logs` | ✅ | append-only 审计日志 |

## 与其他项目的边界

```
aspectus.tenants.id ──(逻辑外键，无 DB 约束)──→ pandaria.sessions.tenant_id
                     ──(逻辑外键，无 DB 约束)──→ tavern.workflow_instances.tenant_id
```

**`tenant_id` 是应用层逻辑纽带，不是数据库级外键。** Pandaria 和 Tavern 各自在每个请求中通过 `POST /introspect` 验证 token 并获取 `tenant_id`，然后以此为 key 写入各自的数据库。Aspectus 不需要知道其他项目的表结构。

## 集成要求

当 Pandaria 和 Tavern 接入 Aspectus `/introspect` 时：

1. **Service Token**：为每个生态项目在 `service_tokens` 中预置一条记录
2. **Scope 种子**：确保每个项目的 scope 已写入 `scopes` 表（Pandaria 6 个、Tavern 4 个）
3. **配额**：在各项目的 `tenants.quotas` JSONB 中配置初始配额

```sql
-- 示例：为 Pandaria 和 Tavern 创建 Service Token
INSERT INTO service_tokens (project, token_hash) VALUES
('pandaria', 'sha256_of_pandaria_service_token'),
('tavern',   'sha256_of_tavern_service_token');
```

## 数据库连接

```bash
# 生产环境
DATABASE_URL=postgres://aspectus_app:xxx@postgres:5432/aspectus

# 开发环境（与 pandaria、tavern 同 PG 实例、不同 database）
DATABASE_URL=postgres://postgres:postgres@localhost:5432/aspectus
```

## 下一步

- [x] 完整 schema 就绪（10 个 migration）
- [ ] 待其他项目接入 `/introspect` 时，确认 scope 种子数据与各项目实际需求一致
- [ ] 配额 JSONB schema 标准化（与其他项目协商 `quotas` 字段格式）
- [ ] Performance: `/introspect` 热路径 P95 < 5ms 持续监控
