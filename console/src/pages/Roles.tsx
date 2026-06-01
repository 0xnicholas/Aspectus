import { useState, useEffect } from "react";
import { Button, Input, Table, Badge, toast } from "../components/ui";
import { api } from "../api/client";

export function Roles() {
  const [roles, setRoles] = useState<any[]>([]);
  const [userId, setUserId] = useState("");
  const [selectedRole, setSelectedRole] = useState("");

  useEffect(() => { api.listRoles().then(setRoles).catch(() => toast("Failed to load roles", "error")); }, []);

  const assign = async () => {
    if (!userId || !selectedRole) return toast("Select user and role", "error");
    try { await api.assignRole(userId, selectedRole); toast("Role assigned!"); }
    catch (e: any) { toast(e.message, "error"); }
  };

  const typeVariant = (t: string) => t === "user" ? "info" : t === "service_account" ? "warning" : "success";

  const columns = [
    { key: "name", header: "Name", render: (r: any) => <strong>{r.name}</strong> },
    { key: "type", header: "Type", width: 140, render: (r: any) => <Badge variant={typeVariant(r.type) as any}>{r.type}</Badge> },
    { key: "is_default", header: "Default", width: 80, render: (r: any) => r.is_default ? "✅" : "" },
    { key: "description", header: "Description", render: (r: any) => <span style={{ color: "#666", fontSize: 13 }}>{r.description}</span> },
  ];

  return (
    <div>
      <h1>Roles</h1>
      <div style={{ display: "flex", gap: 12, marginTop: 16, alignItems: "flex-end" }}>
        <Input label="User ID" value={userId} onChange={e => setUserId(e.target.value)} />
        <div>
          <label style={{ fontSize: 13, fontWeight: 500, color: "#555", display: "block", marginBottom: 4 }}>Role</label>
          <select value={selectedRole} onChange={e => setSelectedRole(e.target.value)} style={{ padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc", fontSize: 14 }}>
            <option value="">Select role...</option>
            {roles.map(r => <option key={r.id} value={r.id}>{r.name}</option>)}
          </select>
        </div>
        <Button onClick={assign}>Assign</Button>
      </div>
      <Table columns={columns} data={roles} rowKey={r => r.id} />
    </div>
  );
}
