import { useState } from "react";
import { api } from "../api/client";

export function Users() {
  const [tenantId, setTenantId] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [users, setUsers] = useState<any[]>([]);

  const listUsers = async () => {
    if (!tenantId) return;
    const data = await api.listUsers(tenantId);
    setUsers(data);
  };

  const createUser = async () => {
    if (!tenantId || !email || !password) return;
    await api.createUser({ tenant_id: tenantId, email, password, display_name: email.split("@")[0] });
    setEmail(""); setPassword("");
    listUsers();
  };

  const toggleSuspend = async (id: string, current: boolean) => {
    await api.suspendUser(id, !current);
    listUsers();
  };

  return (
    <div>
      <h1>Users</h1>
      <div style={{ display: "flex", gap: 8, marginTop: 16, flexWrap: "wrap" }}>
        <input placeholder="Tenant ID" value={tenantId} onChange={e => setTenantId(e.target.value)} style={inputStyle} />
        <button onClick={listUsers} style={btnStyle}>List</button>
      </div>
      <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
        <input placeholder="Email" value={email} onChange={e => setEmail(e.target.value)} style={inputStyle} />
        <input placeholder="Password (≥8)" value={password} type="password" onChange={e => setPassword(e.target.value)} style={inputStyle} />
        <button onClick={createUser} style={btnStyle}>Create User</button>
      </div>
      <table style={{ width: "100%", marginTop: 24, borderCollapse: "collapse" }}>
        <thead><tr style={{ textAlign: "left", borderBottom: "1px solid #ddd" }}><th>ID</th><th>Email</th><th>Suspended</th><th></th></tr></thead>
        <tbody>
          {users.map(u => (
            <tr key={u.id} style={{ borderBottom: "1px solid #eee" }}>
              <td style={{ padding: 8, fontSize: 12, fontFamily: "monospace" }}>{u.id}</td>
              <td>{u.email}</td>
              <td>{u.is_suspended ? "🚫" : "✅"}</td>
              <td><button onClick={() => toggleSuspend(u.id, u.is_suspended)} style={{ ...btnStyle, fontSize: 12 }}>{u.is_suspended ? "Unsuspend" : "Suspend"}</button></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const inputStyle: React.CSSProperties = { padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc" };
const btnStyle: React.CSSProperties = { padding: "8px 16px", borderRadius: 6, background: "#1a1a2e", color: "#fff", border: "none", cursor: "pointer" };
