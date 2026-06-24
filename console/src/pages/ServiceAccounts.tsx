import { useState } from "react";
import { Button, Input, LinkButton, Table, toast } from "../components/ui";
import { api } from "../api/client";

export function ServiceAccounts() {
  const [tenantId, setTenantId] = useState("");
  const [label, setLabel] = useState("");
  const [accounts, setAccounts] = useState<any[]>([]);

  const list = async () => {
    if (!tenantId) return toast("Enter a tenant ID", "error");
    try { setAccounts(await api.listServiceAccounts(tenantId)); } catch { toast("Failed", "error"); }
  };

  const create = async () => {
    if (!tenantId || !label) return toast("Tenant ID and label required", "error");
    try { await api.createServiceAccount({ tenant_id: tenantId, label }); toast("SA created!"); setLabel(""); list(); }
    catch (e: any) { toast(e.message, "error"); }
  };

  const columns = [
    { key: "id", header: "ID", render: (a: any) => <code className="text-xs text-gray-500">{a.id}</code> },
    { key: "label", header: "Label", render: (a: any) => <span className="font-medium">{a.label}</span> },
    { key: "created_at", header: "Created", render: (a: any) => new Date(a.created_at).toLocaleDateString() },
    { key: "actions", header: "", render: (a: any) => <LinkButton size="sm" variant="outline" to={`/service-accounts/${a.id}`}>View</LinkButton> },
  ];

  return (
    <div>
      <h1>Service Accounts</h1>
      <div style={{ display: "flex", gap: 12, marginTop: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Tenant ID" value={tenantId} onChange={e => setTenantId(e.target.value)} />
        <Button onClick={list}>List</Button>
      </div>
      <div style={{ display: "flex", gap: 12, marginTop: 12, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Label" value={label} onChange={e => setLabel(e.target.value)} />
        <Button onClick={create}>Create</Button>
      </div>
      <Table columns={columns} data={accounts} rowKey={a => a.id} />
    </div>
  );
}
