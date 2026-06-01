import { useState, useEffect } from "react";
import { Button, Input, Table, toast } from "../components/ui";
import { api } from "../api/client";

export function Roles() {
  const [roles, setRoles] = useState<any[]>([]);
  const [userId, setUserId] = useState("");
  const [selectedRole, setSelectedRole] = useState("");

  useEffect(() => { api.listRoles().then(setRoles).catch(() => toast("Failed", "error")); }, []);

  const assign = async () => {
    if (!userId || !selectedRole) return toast("Select user and role", "error");
    try { await api.assignRole(userId, selectedRole); toast("Role assigned!"); }
    catch (e: any) { toast(e.message, "error"); }
  };

  const typeBadge = (t: string) => {
    const colors: Record<string, string> = {
      user: "bg-blue-100 text-blue-800", service_account: "bg-yellow-100 text-yellow-800", both: "bg-green-100 text-green-800",
    };
    return `inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${colors[t] || ""}`;
  };

  const columns = [
    { key: "name", header: "Name", render: (r: any) => <span className="font-medium">{r.name}</span> },
    { key: "type", header: "Type", render: (r: any) => <span className={typeBadge(r.type)}>{r.type}</span> },
    { key: "default", header: "Default", render: (r: any) => r.is_default ? "✅" : "" },
    { key: "desc", header: "Description", render: (r: any) => <span className="text-sm text-gray-500">{r.description}</span> },
  ];

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900">Roles</h1>
      <div className="mt-4 flex flex-wrap items-end gap-3">
        <Input label="User ID" value={userId} onChange={e => setUserId(e.target.value)} />
        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium text-gray-600">Role</label>
          <select value={selectedRole} onChange={e => setSelectedRole(e.target.value)}
            className="h-10 rounded-md border border-border bg-white px-3 text-sm">
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
