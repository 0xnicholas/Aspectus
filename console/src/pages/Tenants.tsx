import { useEffect, useState } from "react";
import { Button, Input, Modal, PageHeader, Table, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

export function Tenants() {
  const [tenants, setTenants] = useState<any[]>([]);
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [editTenant, setEditTenant] = useState<any>(null);
  const [quotasText, setQuotasText] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      setTenants(await api.listTenants());
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const create = async () => {
    if (!name) return toast("Name required", "error");
    try {
      const t = await api.createTenant(name);
      toast(`Created: ${t.id}`);
      setName("");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const openEdit = (tenant: any) => {
    setEditTenant(tenant);
    setQuotasText(JSON.stringify(tenant.quotas ?? {}, null, 2));
  };

  const saveQuotas = async () => {
    if (!editTenant) return;
    let quotas: Record<string, any>;
    try {
      quotas = JSON.parse(quotasText);
    } catch {
      return toast("Invalid JSON", "error");
    }
    try {
      await api.updateTenantQuotas(editTenant.id, quotas);
      toast("Quotas updated");
      setEditTenant(null);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const columns = [
    { key: "id", header: "ID", render: (t: any) => <code className="text-xs text-gray-500">{t.id}</code> },
    { key: "name", header: "Name", render: (t: any) => <span className="font-medium">{t.name}</span> },
    { key: "created_at", header: "Created", render: (t: any) => new Date(t.created_at).toLocaleString() },
    { key: "quotas", header: "Quotas", render: (t: any) => <code className="text-xs text-gray-500">{JSON.stringify(t.quotas ?? {})}</code> },
    { key: "actions", header: "", render: (t: any) => <Button size="sm" variant="outline" onClick={() => openEdit(t)}>Edit Quotas</Button> },
  ];

  return (
    <div>
      <PageHeader title="Tenants" subtitle="Create and manage tenants in the Pandaria ecosystem." />

      <div className="flex flex-wrap items-end gap-3">
        <Input label="Tenant Name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Acme Corp" />
        <Button onClick={create}>Create</Button>
        <Button variant="outline" onClick={load} loading={loading}>Refresh</Button>
      </div>

      {tenants.length === 0 && !loading ? (
        <EmptyState message="No tenants found." />
      ) : (
        <Table columns={columns} data={tenants} rowKey={(t) => t.id} />
      )}

      <Modal
        open={!!editTenant}
        title={`Edit Quotas — ${editTenant?.name}`}
        message="Update the per-project quota JSON for this tenant."
        confirmLabel="Save"
        variant="primary"
        onConfirm={saveQuotas}
        onCancel={() => setEditTenant(null)}
      >
        <textarea
          value={quotasText}
          onChange={(e) => setQuotasText(e.target.value)}
          rows={10}
          className="mt-4 w-full rounded-md border border-border bg-gray-50 p-3 font-mono text-sm outline-none focus:border-primary focus:ring-1 focus:ring-primary"
        />
      </Modal>
    </div>
  );
}
