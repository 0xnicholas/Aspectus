import { useState } from "react";
import { Button, Input, LinkButton, Table, Modal, toast } from "../components/ui";
import { api } from "../api/client";

export function Users() {
  const [tenantId, setTenantId] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [users, setUsers] = useState<any[]>([]);
  const [suspendTarget, setSuspendTarget] = useState<any>(null);
  const [unlockTarget, setUnlockTarget] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  const listUsers = async () => {
    if (!tenantId) return toast("Enter a tenant ID", "error");
    setLoading(true);
    try { setUsers(await api.listUsers(tenantId)); } catch { toast("Failed", "error"); }
    setLoading(false);
  };

  const createUser = async () => {
    if (!tenantId || !email || !password) return toast("All fields required", "error");
    if (password.length < 8) return toast("Password ≥8 chars", "error");
    try {
      await api.createUser({ tenant_id: tenantId, email, password, display_name: email.split("@")[0] });
      toast("User created!"); setEmail(""); setPassword(""); listUsers();
    } catch (e: any) { toast(e.message, "error"); }
  };

  const toggleSuspend = async () => {
    if (!suspendTarget) return;
    await api.suspendUser(suspendTarget.id, !suspendTarget.is_suspended);
    toast(suspendTarget.is_suspended ? "Unsuspended" : "Suspended");
    setSuspendTarget(null); listUsers();
  };

  const unlock = async () => {
    if (!unlockTarget) return;
    await api.unlockUser(unlockTarget.id);
    toast("User unlocked");
    setUnlockTarget(null); listUsers();
  };

  const isLocked = (u: any) => {
    if (!u.locked_until) return false;
    return new Date(u.locked_until) > new Date();
  };

  const statusBadge = (u: any) => {
    if (u.is_suspended) {
      return <span className="inline-flex items-center rounded bg-red-100 px-2 py-0.5 text-xs font-medium text-red-800">Suspended</span>;
    }
    if (isLocked(u)) {
      return <span className="inline-flex items-center rounded bg-yellow-100 px-2 py-0.5 text-xs font-medium text-yellow-800">Locked</span>;
    }
    return <span className="inline-flex items-center rounded bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800">Active</span>;
  };

  const columns = [
    { key: "id", header: "ID", render: (u: any) => <code className="text-xs text-gray-500">{u.id}</code> },
    { key: "email", header: "Email" },
    { key: "status", header: "Status", render: statusBadge },
    { key: "actions", header: "", render: (u: any) => (
      <div className="flex gap-2">
        <LinkButton size="sm" variant="outline" to={`/users/${u.id}`}>View</LinkButton>
        {isLocked(u) && (
          <Button size="sm" variant="outline" onClick={() => setUnlockTarget(u)}>Unlock</Button>
        )}
        <Button size="sm" variant={u.is_suspended ? "primary" : "destructive"} onClick={() => setSuspendTarget(u)}>
          {u.is_suspended ? "Unsuspend" : "Suspend"}
        </Button>
      </div>
    )},
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900">Users</h1>
      <div className="mt-4 flex flex-wrap items-end gap-3">
        <Input label="Tenant ID" value={tenantId} onChange={e => setTenantId(e.target.value)} />
        <Button onClick={listUsers} loading={loading}>List</Button>
      </div>
      <div className="mt-3 flex flex-wrap items-end gap-3">
        <Input label="Email" value={email} onChange={e => setEmail(e.target.value)} />
        <Input label="Password (≥8)" type="password" value={password} onChange={e => setPassword(e.target.value)} />
        <Button onClick={createUser}>Create</Button>
      </div>
      <Table columns={columns} data={users} rowKey={u => u.id} />
      <Modal open={!!suspendTarget} title={suspendTarget?.is_suspended ? "Unsuspend" : "Suspend"}
        message={`${suspendTarget?.email} will be ${suspendTarget?.is_suspended ? "unsuspended" : "suspended"}.`}
        confirmLabel={suspendTarget?.is_suspended ? "Unsuspend" : "Suspend"}
        variant={suspendTarget?.is_suspended ? "primary" : "destructive"}
        onConfirm={toggleSuspend} onCancel={() => setSuspendTarget(null)} />
      <Modal open={!!unlockTarget} title="Unlock User"
        message={`Unlock ${unlockTarget?.email}? This clears failed login attempts and any active lockout.`}
        confirmLabel="Unlock" variant="primary"
        onConfirm={unlock} onCancel={() => setUnlockTarget(null)} />
    </div>
  );
}
