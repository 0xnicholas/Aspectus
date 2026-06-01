import { useState, useEffect } from "react";
import { api } from "../api/client";

export function Roles() {
  const [roles, setRoles] = useState<any[]>([]);
  const [userId, setUserId] = useState("");
  const [selectedRole, setSelectedRole] = useState("");

  useEffect(() => { api.listRoles().then(setRoles); }, []);

  const assign = async () => {
    if (!userId || !selectedRole) return;
    await api.assignRole(userId, selectedRole);
    alert("Role assigned!");
  };


  return (
    <div>
      <h1>Roles</h1>
      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <input placeholder="User ID" value={userId} onChange={e => setUserId(e.target.value)} style={inputStyle} />
        <select value={selectedRole} onChange={e => setSelectedRole(e.target.value)} style={inputStyle}>
          <option value="">Select role...</option>
          {roles.map(r => <option key={r.id} value={r.id}>{r.name} ({r.type})</option>)}
        </select>
        <button onClick={assign} style={btnStyle}>Assign</button>
      </div>
      <table style={{ width: "100%", marginTop: 24, borderCollapse: "collapse" }}>
        <thead><tr style={{ textAlign: "left", borderBottom: "1px solid #ddd" }}><th>Name</th><th>Type</th><th>Default</th><th>Description</th></tr></thead>
        <tbody>
          {roles.map(r => (
            <tr key={r.id} style={{ borderBottom: "1px solid #eee" }}>
              <td style={{ padding: 8, fontWeight: 600 }}>{r.name}</td>
              <td><span style={{ padding: "2px 8px", borderRadius: 4, fontSize: 12, background: r.type === "user" ? "#e3f2fd" : r.type === "service_account" ? "#fce4ec" : "#e8f5e9" }}>{r.type}</span></td>
              <td>{r.is_default ? "✅" : ""}</td>
              <td style={{ fontSize: 13, color: "#666" }}>{r.description}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const inputStyle: React.CSSProperties = { padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc" };
const btnStyle: React.CSSProperties = { padding: "8px 16px", borderRadius: 6, background: "#1a1a2e", color: "#fff", border: "none", cursor: "pointer" };
