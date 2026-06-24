# Aspectus Operations Runbook

> 版本：v1.1.0 | 最后更新：2026-06-23

本文档描述 Aspectus 生产运维的标准操作流程、本地开发速查和常见故障处理。

---

## 服务概览

| 属性 | 值 |
|------|-----|
| 服务名 | aspectus |
| 端口 | 3100 |
| 副本数 | 3 (生产) |
| 依赖 | PostgreSQL 17, Redis 7 |
| 关键端点 | `POST /introspect` (每请求必调), `POST /authorize`, `POST /oauth/token` |

---

## 本地开发速查

### 启动依赖

```bash
docker compose up -d
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus sqlx migrate run
```

### 运行全部测试

```bash
DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus \
  REDIS_URL=redis://localhost:6380 \
  cargo test --workspace
```

### 启动后端

```bash
cargo run -p aspectus-server
```

### 启动管理控制台

```bash
cd console
npm install
npm run dev
# http://localhost:5180/
```

---

## 日常运维

### 启动

```bash
# Kubernetes
kubectl scale deployment aspectus --replicas=3

# docker-compose
docker compose up -d
```

### 停止

```bash
# Kubernetes (graceful, waits for in-flight requests)
kubectl scale deployment aspectus --replicas=0

# docker-compose
docker compose stop aspectus
```

### 重启（零停机）

Aspectus 支持优雅关闭。滚动重启不会丢失 in-flight 请求：

```bash
kubectl rollout restart deployment aspectus
```

### 扩缩容

```bash
# 手动扩容
kubectl scale deployment aspectus --replicas=5

# 或调整 HPA
kubectl patch hpa aspectus -p '{"spec":{"maxReplicas":20}}'
```

### 查看日志

```bash
# 最近 100 行
kubectl logs deployment/aspectus --tail=100

# 实时跟踪
kubectl logs deployment/aspectus -f

# 按 trace_id 过滤（所有请求都有 trace_id）
kubectl logs deployment/aspectus | grep "trace_id=<id>"
```

### 健康检查

```bash
# 基本检查
curl http://aspectus:3100/health
# → {"status":"ok"}

# 完整检查（含 PG + Redis）
curl "http://aspectus:3100/health?full=true"
# → {"status":"ok","postgres":"ok","redis":"ok"}
```

### Service Token 轮换

```bash
# 1. 轮换 pandaria 的 service token（返回一次性新 token）
curl -X POST -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  http://aspectus:3100/service-tokens/pandaria/rotate

# 2. 更新对应项目的 secret 配置

# 3. 如 token 已泄露，先吊销再创建新的
curl -X DELETE -H "Authorization: Bearer $ADMIN_SERVICE_TOKEN" \
  http://aspectus:3100/service-tokens/pandaria
```

---

## 故障处理

### 1. Redis 不可用

**症状**：
- `/health?full=true` 返回 `"redis":"error"`
- `/introspect` p95 延迟从 2ms 升至 10ms+
- Prometheus 告警 `AspectusRedisUnavailable`

**影响**：Aspectus 自动降级——自省缓存失效，所有请求回退到 PostgreSQL 查询。服务仍可用但延迟升高。

**处理步骤**：

1. **确认 Redis 状态**：
   ```bash
   kubectl get pods -l app=redis
   kubectl logs deployment/redis --tail=20
   ```

2. **检查 Redis 内存**：
   ```bash
   kubectl exec deployment/redis -- redis-cli INFO memory | grep used_memory_human
   ```

3. **如果 Redis OOM**：
   - 增加 Redis 内存限制
   - 设置 `maxmemory-policy allkeys-lru`（Aspectus 缓存可丢失）

4. **如果 Redis Pod 挂掉**：
   - Redis 重启后 Aspectus 自动恢复，无需额外操作
   - 首次请求会 cache miss，后续恢复正常

5. **如果 Redis 长时间不可用（> 30 分钟）**：
   - 考虑临时降低 `DB_MIN_CONNECTIONS` 以释放 PG 连接
   - Aspectus 在无 Redis 下可无限期运行（只是慢一些）

### 2. PostgreSQL 故障

**症状**：
- `/health?full=true` 返回 `"postgres":"error"`
- 所有请求返回 500
- Prometheus 告警 `AspectusDatabaseConnectionPoolSaturated`

**影响**：服务完全不可用。所有端点依赖 PostgreSQL。

**处理步骤**：

1. **检查 PG 连接**：
   ```bash
   kubectl exec deployment/aspectus -- psql "$DATABASE_URL" -c "SELECT 1"
   ```

2. **检查连接池**：
   ```bash
   # 查看活跃连接数
   kubectl exec deployment/aspectus -- psql "$DATABASE_URL" -c \
     "SELECT count(*) FROM pg_stat_activity WHERE datname='aspectus'"
   ```

3. **如果连接池饱和**：
   - 临时增加 `DB_MAX_CONNECTIONS`（更新 ConfigMap + 重启）
   - 检查是否有长时间运行的查询：
     ```sql
     SELECT pid, now() - pg_stat_activity.query_start AS duration, query
     FROM pg_stat_activity
     WHERE state != 'idle' ORDER BY duration DESC LIMIT 10;
     ```

4. **如果 PG 主库故障**：
   - 切换到副本：更新 `DATABASE_URL` Secret 指向副本
   - 重启 Aspectus Pods
   - 注意：副本可能有几秒的复制延迟

5. **恢复后验证**：
   ```bash
   curl "http://aspectus:3100/health?full=true"
   ```

### 3. JWT 密钥泄露 / 轮换

**症状**：安全事件——JWT 签名私钥可能已泄露。

**影响**：攻击者可以伪造有效的 JWT token。

**紧急处理步骤**：

1. **立即生成新密钥对**：
   ```bash
   openssl genrsa -out new_private.pem 2048
   openssl rsa -in new_private.pem -pubout -out new_public.pem
   ```

2. **更新 Secret**：
   ```bash
   kubectl create secret generic aspectus-secret \
     --from-literal=jwt-private-key-pem="$(cat new_private.pem)" \
     --from-literal=jwt-public-key-pem="$(cat new_public.pem)" \
     --dry-run=client -o yaml | kubectl apply -f -
   ```

3. **重启 Aspectus**：
   ```bash
   kubectl rollout restart deployment aspectus
   ```

4. **吊销所有现存 token**：
   ```bash
   # 清除 JWT 吊销列表缓存（Redis）
   kubectl exec deployment/redis -- redis-cli KEYS "jwt_revoked:*" | \
     xargs kubectl exec deployment/redis -- redis-cli DEL
   ```
   注意：密钥轮换后所有现存 JWT 自动失效（签名不匹配）。用户需要重新登录。

5. **通知消费者**：所有生态项目需要更新 JWKS 公钥缓存。

### 4. API Key 批量泄露

**症状**：多个 API Key 泄露，需要批量吊销。

**处理步骤**：

1. **列出受影响 Service Account 的所有 Key**：
   ```bash
   curl -H "Authorization: Bearer $SERVICE_TOKEN" \
     "http://aspectus:3100/api-keys?service_account_id=<sa_id>" | jq '.[].id'
   ```

2. **批量吊销**：
   ```bash
   for key_id in $KEY_IDS; do
     curl -X DELETE -H "Authorization: Bearer $SERVICE_TOKEN" \
       "http://aspectus:3100/api-keys/$key_id"
   done
   ```

3. **创建新 Key**：参考 [API Key 管理文档](../README.md)

4. **审计**：查询审计日志确认吊销操作已记录。

### 5. OAuth2 暴力破解检测

**症状**：
- Prometheus 告警 `AspectusOAuth2HighFailureRate`
- `/authorize` 端点大量 401 响应

**处理步骤**：

1. **查看请求来源 IP**：
   ```bash
   kubectl logs deployment/aspectus | grep "/authorize" | grep "401" | \
     awk '{print $NF}' | sort | uniq -c | sort -rn | head -20
   ```

2. **临时封禁 IP**（如果有 WAF/nginx）：
   ```bash
   # 在 Ingress/Nginx 层面封禁
   kubectl annotate ingress aspectus \
     nginx.ingress.kubernetes.io/whitelist-source-range="<allowed_ips>"
   ```

3. **确认 Rate Limiting 生效**：`/authorize` 默认 5 次/分钟/IP。确认限流中间件在运行。

---

## 数据管理

### 数据库备份

- **频率**：每日全量 pg_dump + 持续 WAL 归档
- **保留**：30 天日备，12 个月月备
- **恢复流程**：见 [备份策略文档](backup-strategy.md)

### 审计日志归档

- 审计日志表 `audit_logs` 从不清除（append-only）
- 建议：90 天后归档到对象存储（S3/MinIO）
- 归档脚本：
  ```sql
  -- 导出 90 天前的审计日志
  COPY (SELECT * FROM audit_logs WHERE created_at < now() - INTERVAL '90 days')
  TO '/tmp/audit_archive.csv' CSV HEADER;
  ```

### Migration 执行

```bash
# 在部署前运行（CI/CD 中自动化）
sqlx migrate run --database-url "$DATABASE_URL"

# 验证 migration 状态
sqlx migrate info --database-url "$DATABASE_URL"
```

**回滚**：Aspectus migration 不可回滚（没有 down migration）。如果 migration 失败：
1. 从备份恢复数据库
2. 修复 migration
3. 重新执行

---

## 监控指标

### 关键指标（Grafana）

| 指标 | 说明 | 正常范围 |
|------|------|---------|
| `introspect_total` | 自省请求总数 | — |
| `introspect_duration_seconds` | 自省延迟直方图 | p95 < 5ms |
| `db_connections_active` | 活跃 DB 连接数 | < 40 (max=50) |
| `redis_up` | Redis 可达性 | 1 |
| `rate_limit_exceeded_total` | 被限流请求数 | < 1/s |
| `oauth_authorize_total` | OAuth2 授权尝试 | — |

### 告警响应时间

| 严重级别 | 响应时间 | 通知方式 |
|---------|:------:|---------|
| critical | 5 分钟 | PagerDuty / 电话 |
| warning | 30 分钟 | Slack / 邮件 |

---

## 安全检查清单（每季度）

- [ ] JWT 签名密钥轮换
- [ ] Service Token 轮换（每个 Project）
- [ ] `cargo audit` 依赖漏洞扫描
- [ ] 数据库备份恢复演练
- [ ] 访问审计日志审查（可疑操作）
- [ ] Rate limit 阈值复核（是否有合法流量被限）
