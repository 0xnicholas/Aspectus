import { useState } from "react";
import { api } from "../api/client";

export function ApiKeys() {
  const [saId, setSaId] = useState("");
  const [project, setProject] = useState("pandaria");
  const [scopes, setScopes] = useState("");
  const [keys, setKeys] = useState<any[]>([]);
  const [newKey, setNewKey] = useState<string | null>(null);

  const list = async () => {
    if (!saId) return;
    setKeys(await api.listApiKeys(saId));
  };

  const create = async () => {
    if (!saId) return;
    const res = await api.createApiKey({
      service_account_id: saId,
      project,
      scopes: scopes ? scopes.split(",").map(s => s.trim()) : [],
    });
    setNewKey(res.key);
    list();
  };

  const revoke = async (id: string) => {
    await api.revokeApiKey(id);
    list();
  };

  return (
    <div>
      <h1>API Keys</h1>
      {newKey && (
        <div style={{ padding: 16, background: "#fff3cd", borderRadius: 8, marginTop: 12 }}>
          <strong>⚠️ Copy this key now — it won't be shown again:</strong>
          <pre style={{ background: "#eee", padding: 8, borderRadius: 4, marginTop: 8 }}>{newKey}</pre>
        </div>
      )}
      <div style={{ display: "flex", gap: 8, marginTop: 16, flexWrap: "wrap" }}>
        <input placeholder="Service Account ID" value={saId} onChange={e => setSaId(e.target.value)} style={inputStyle} />
        <select value={project} onChange={e => setProject(e.target.value)} style={inputStyle}>
          {["pandaria","tavern","emerald","constell","tokencamp","heirloom"].map(p => <option key={p}>{p}</option>)}
        </select>
        <input placeholder="scopes (comma separated)" value={scopes} onChange={e => setScopes(e.target.value)} style={inputStyle} />
        <button onClick={list} style={btnStyle}>List</button>
        <button onClick={create} style={btnStyle}>Create</button>
      </div>
      <table style={{ width: "100%", marginTop: 24, borderCollapse: "collapse" }}>
        <thead><tr style={{ textAlign: "left", borderBottom: "1px solid #ddd" }}><th>Prefix</th><th>Project</th><th>Scopes</th><th>Status</th><th></th></tr></thead>
        <tbody>
          {keys.map(k => (
            <tr key={k.id} style={{ borderBottom: "1px solid #eee" }}>
              <td style={{ padding: 8, fontFamily: "monospace", fontSize: 12 }}>{k.key_prefix}</td>
              <td>{k.project}</td>
              <td style={{ fontSize: 12 }}>{(k.scopes || []).join(", ")}</td>
              <td>{k.revoked_at ? "🚫 Revoked" : "✅ Active"}</td>
              <td>{!k.revoked_at && <button onClick={() => revoke(k.id)} style={{ ...btnStyle, fontSize: 12 }}>Revoke</button>}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const inputStyle: React.CSSProperties = { padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc" };
const btnStyle: React.CSSProperties = { padding: "8px 16px", borderRadius: 6, background: "#1a1a2e", color: "#fff", border: "none", cursor: "pointer" };
