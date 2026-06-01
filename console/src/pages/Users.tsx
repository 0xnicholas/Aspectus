import { useState } from "react";
import { Button, Input, Table, Modal, toast } from "../components/ui";
import { api } from "../api/client";

export function Users() {
  const [tenantId, setTenantId] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [users, setUsers] = useState<any[]>([]);
  const [suspendTarget, setSuspendTarget] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  const listUsers = async () => {
    if (!tenantId) return toast("Enter a tenant ID", "error");
    setLoading(true);
    try {
      setUsers(await api.listUsers(tenantId));
    } catch { toast("Failed to load users", "error"); }
    setLoading(false);
  };

  const createUser = async () => {
    if (!tenantId || !email || !password) return toast("All fields required", "error");
    if (password.length < 8) return toast("Password must be ≥8 chars", "error");
    try {
      await api.createUser({ tenant_id: tenantId, email, password, display_name: email.split("@")[0] });
      toast("User created!");
      setEmail(""); setPassword("");
      listUsers();
    } catch (e: any) { toast(e.message, "error"); }
  };

  const toggleSuspend = async () => {
    if (!suspendTarget) return;
    try {
      await api.suspendUser(suspendTarget.id, !suspendTarget.is_suspended);
      toast(suspendTarget.is_suspended ? "User unsuspended" : "User suspended");
      setSuspendTarget(null);
      listUsers();
    } catch (e: any) { toast(e.message, "error"); }
  };

  const columns = [
    { key: "id", header: "ID", width: 180, render: (u: any) => <code style={{ fontSize: 11 }}>{u.id}</code> },
    { key: "email", header: "Email" },
    { key: "is_suspended", header: "Status", width: 100, render: (u: any) => u.is_suspended ? <Badge variant="destructive">Suspended</Badge> : <Badge variant="success">Active</Badge> },
    { key: "actions", header: "", width: 100, render: (u: any) => <Button size="sm" variant={u.is_suspended ? "primary" : "destructive"} onClick={() => setSuspendTarget(u)}>{u.is_suspended ? "Unsuspend" : "Suspend"}</Button> },
  ];

  return (
    <div>
      <h1>Users</h1>
      <div style={{ display: "flex", gap: 12, marginTop: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Tenant ID" value={tenantId} onChange={e => setTenantId(e.target.value)} />
        <Button onClick={listUsers} loading={loading}>List</Button>
      </div>
      <div style={{ display: "flex", gap: 12, marginTop: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Email" value={email} onChange={e => setEmail(e.target.value)} />
        <Input label="Password (≥8)" type="password" value={password} onChange={e => setPassword(e.target.value)} />
        <Button onClick={createUser}>Create User</Button>
      </div>
      <Table columns={columns} data={users} rowKey={u => u.id} emptyText={loading ? "Loading..." : "No users yet"} />
      <Modal open={!!suspendTarget} title={suspendTarget?.is_suspended ? "Unsuspend User" : "Suspend User"}
        message={`${suspendTarget?.is_suspended ? "Unsuspend" : "Suspend"} ${suspendTarget?.email}?`}
        confirmLabel={suspendTarget?.is_suspended ? "Unsuspend" : "Suspend"}
        variant={suspendTarget?.is_suspended ? "primary" : "destructive"}
        onConfirm={toggleSuspend} onCancel={() => setSuspendTarget(null)} />
    </div>
  );
}

function Badge({ variant, children }: { variant: string; children: React.ReactNode }) {
  const colors: Record<string, React.CSSProperties> = {
    success: { background: "#e8f5e9", color: "#2e7d32" },
    danger: { background: "#fce4ec", color: "#c62828" },
  };
  return <span style={{ padding: "2px 8px", borderRadius: 4, fontSize: 12, fontWeight: 500, ...colors[variant] }}>{children}</span>;
}
