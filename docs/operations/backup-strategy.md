# Aspectus — 备份与恢复策略

> 版本：v1.0.0 | 最后更新：2026-06-15

---

## 备份对象

| 数据 | 类型 | 关键性 | 备份策略 |
|------|------|:-----:|---------|
| PostgreSQL | 持久数据 | 关键 | 每日全量 + 持续 WAL 归档 |
| Redis | 缓存数据 | 低 | 不备份（可重建） |
| JWT 密钥 | 机密 | 关键 | 离线安全存储（1Password / Vault） |
| Service Token | 机密 | 关键 | 离线安全存储 |

---

## PostgreSQL 备份

### 频率与保留

| 备份类型 | 频率 | 保留 |
|---------|------|------|
| 全量 pg_dump | 每日 02:00 UTC | 30 天 |
| WAL 归档 | 持续 | 与日备对齐 |
| 月归档 | 每月 1 日 | 12 个月 |
| 迁移前快照 | 每次 migration 前 | 7 天 |

### 备份命令

```bash
# 每日全量备份
pg_dump -Fc -f "aspectus_$(date +%Y%m%d).dump" "$DATABASE_URL"

# WAL 归档（需配置 archive_command）
# postgresql.conf:
# archive_mode = on
# archive_command = 'aws s3 cp %p s3://aspectus-backups/wal/%f'

# 验证备份完整性
pg_restore --list aspectus_20260615.dump | head -20
```

### 恢复流程

**RTO 目标：< 1 小时**

1. **从全量备份恢复**：
   ```bash
   # 创建空数据库
   createdb aspectus_restore
   
   # 恢复
   pg_restore -d "$DATABASE_URL" --clean --if-exists aspectus_20260615.dump
   ```

2. **应用 WAL（时间点恢复）**：
   ```bash
   # 恢复到特定时间点
   pg_restore -d "$DATABASE_URL" \
     --recovery-target-time="2026-06-15 14:30:00 UTC" \
     aspectus_20260615.dump
   ```

3. **验证恢复**：
   ```bash
   psql "$DATABASE_URL" -c "
     SELECT
       (SELECT count(*) FROM tenants) AS tenants,
       (SELECT count(*) FROM users) AS users,
       (SELECT count(*) FROM api_keys WHERE revoked_at IS NULL) AS active_keys,
       (SELECT max(created_at) FROM audit_logs) AS last_audit;
   "
   ```

4. **更新应用配置**：如果数据库地址改变，更新 `DATABASE_URL` Secret。

### 恢复演练

**频率**：每月一次

**演练步骤**：
1. 从月归档备份恢复到临时数据库
2. 验证表数量和行数在预期范围内
3. 运行 `sqlx migrate info` 确认 migration 状态一致
4. 启动 Aspectus 并执行烟雾测试
5. 记录 RTO（实际恢复时间）

---

## Redis 持久化策略

Aspectus 使用 Redis 作为 **缓存层**（cache aside pattern），不存储持久数据。

| Redis 数据 | 用途 | 丢失影响 |
|-----------|------|---------|
| `introspect:{hash}` | API Key / JWT 自省缓存 | 回退到 PG 查询，延迟增加 5ms |
| `svc_token:{hash}` | Service Token 缓存 | 回退到 PG 查询 |
| `scope_expand:{user_id}` | 用户 Scope 展开缓存 | 回退到 PG JOIN 查询 |
| `jwt_revoked:{jti}` | JWT 吊销列表 | ⚠️ 吊销失效——已吊销 JWT 重新有效 |
| `rate_limit:{key}` | 内存限流计数器 | 限流窗口重置 |

**RDB 配置**：
```conf
# redis.conf
save 900 1     # 15 分钟内至少 1 次写操作
save 300 10    # 5 分钟内至少 10 次写操作
save 60 10000  # 1 分钟内至少 10000 次写操作
```

**注意**：`jwt_revoked` 的丢失意味着在 Redis 重启后，之前吊销的 JWT 会短暂可用（直到它们自然过期）。如果这是不可接受的，考虑将 JWT 吊销列表也写入 PostgreSQL。

---

## 机密管理

| 机密 | 存储位置 | 轮换频率 |
|------|---------|:-------:|
| JWT 私钥 | Kubernetes Secret / Vault | 每季度 |
| JWT 公钥 | Kubernetes Secret（不敏感）| 随私钥 |
| Service Token | Kubernetes Secret / Vault | 每季度 |
| 数据库密码 | Kubernetes Secret / Vault | 每半年 |
| Redis 密码 | Kubernetes Secret / Vault | 每半年 |

**轮换流程**见 [运维手册](runbook.md) — 第三节和第六节。

---

## 审计日志归档

`audit_logs` 表是 append-only，随时间增长。建议实施归档策略：

1. **90 天热数据**：保留在 PostgreSQL 主表中
2. **90 天 - 1 年**：归档到对象存储（S3/MinIO）的 Parquet 文件
3. **> 1 年**：转移到冷存储（Glacier/Archive tier）

**归档脚本示例**：
```sql
-- 导出 90 天前的审计日志到 CSV
\copy (SELECT * FROM audit_logs WHERE created_at < now() - INTERVAL '90 days')
TO '/tmp/audit_archive.csv' CSV HEADER;

-- 然后删除（仅在验证归档成功后执行）
-- DELETE FROM audit_logs WHERE created_at < now() - INTERVAL '90 days';
```

**注意**：实际删除前务必确认归档文件已成功上传并验证可读。
