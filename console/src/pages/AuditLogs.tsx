export function AuditLogs() {
  return (
    <div>
      <h1>Audit Logs</h1>
      <p style={{ color: "#666" }}>Audit logs are stored in PostgreSQL. Query directly:</p>
      <pre style={{ background: "#1a1a2e", color: "#eee", padding: 16, borderRadius: 8, marginTop: 12 }}>
{`SELECT action, target_type, actor_id, created_at
FROM audit_logs
WHERE tenant_id = '...'
ORDER BY created_at DESC
LIMIT 50;`}
      </pre>
    </div>
  );
}
