import { useState } from "react";
import { Button, Input, Table, Badge, toast } from "../components/ui";
import { api } from "../api/client";
import { Modal } from "../components/ui";

export function ApiKeys() {
  const [saId, setSaId] = useState("");
  const [project, setProject] = useState("pandaria");
  const [scopes, setScopes] = useState("");
  const [keys, setKeys] = useState<any[]>([]);
  const [newKey, setNewKey] = useState<string | null>(null);
  const [revokeTarget, setRevokeTarget] = useState<string | null>(null);

  const list = async () => {
    if (!saId) return toast("Enter a Service Account ID", "error");
    try { setKeys(await api.listApiKeys(saId)); } catch { toast("Failed to load keys", "error"); }
  };

  const create = async () => {
    if (!saId) return toast("Service Account ID required", "error");
    try {
      const res = await api.createApiKey({ service_account_id: saId, project, scopes: scopes ? scopes.split(",").map(s => s.trim()) : [] });
      setNewKey(res.key); list(); toast("Key created!");
    } catch (e: any) { toast(e.message, "error"); }
  };

  const revoke = async () => {
    if (!revokeTarget) return;
    try { await api.revokeApiKey(revokeTarget); toast("Key revoked"); setRevokeTarget(null); list(); }
    catch (e: any) { toast(e.message, "error"); }
  };

  const columns = [
    { key: "key_prefix", header: "Prefix", width: 160, render: (k: any) => <code style={{ fontSize: 12 }}>{k.key_prefix}</code> },
    { key: "project", header: "Project", width: 100 },
    { key: "scopes", header: "Scopes", render: (k: any) => <span style={{ fontSize: 12 }}>{(k.scopes || []).join(", ") || "—"}</span> },
    { key: "status", header: "Status", width: 100, render: (k: any) => k.revoked_at ? <Badge variant="destructive">Revoked</Badge> : <Badge variant="success">Active</Badge> },
    { key: "actions", header: "", width: 80, render: (k: any) => !k.revoked_at && <Button size="sm" variant="destructive" onClick={() => setRevokeTarget(k.id)}>Revoke</Button> },
  ];

  return (
    <div>
      <h1>API Keys</h1>
      {newKey && (
        <div style={{ padding: 16, background: "#fff3cd", borderRadius: 8, marginTop: 12, border: "1px solid #ffc107" }}>
          <strong>⚠️ Copy this key now — it won't be shown again:</strong>
          <pre style={{ background: "#eee", padding: 8, borderRadius: 4, marginTop: 8, fontSize: 12, wordBreak: "break-all" }}>{newKey}</pre>
          <Button size="sm" variant="ghost" onClick={() => setNewKey(null)}>Dismiss</Button>
        </div>
      )}
      <div style={{ display: "flex", gap: 12, marginTop: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Service Account ID" value={saId} onChange={e => setSaId(e.target.value)} />
        <div>
          <label style={{ fontSize: 13, fontWeight: 500, color: "#555", display: "block", marginBottom: 4 }}>Project</label>
          <select value={project} onChange={e => setProject(e.target.value)} style={{ padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc", fontSize: 14 }}>
            {["pandaria","tavern","emerald","constell","tokencamp","heirloom"].map(p => <option key={p}>{p}</option>)}
          </select>
        </div>
        <Input label="Scopes (comma)" value={scopes} onChange={e => setScopes(e.target.value)} />
        <Button onClick={list}>List</Button>
        <Button onClick={create}>Create</Button>
      </div>
      <Table columns={columns} data={keys} rowKey={k => k.id} />
      <Modal open={!!revokeTarget} title="Revoke API Key" message="This action cannot be undone. The key will stop working immediately."
        confirmLabel="Revoke" variant="destructive" onConfirm={revoke} onCancel={() => setRevokeTarget(null)} />
    </div>
  );
}
