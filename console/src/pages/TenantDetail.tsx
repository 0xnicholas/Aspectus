import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Button, Card, LinkButton, Modal, PageHeader, Table, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

export function TenantDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [tenant, setTenant] = useState<any>(null);
  const [users, setUsers] = useState<any[]>([]);
  const [accounts, setAccounts] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [quotasText, setQuotasText] = useState("");
  const [showEdit, setShowEdit] = useState(false);

  const load = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const [t, u, a] = await Promise.all([
        api.getTenant(id),
        api.listUsers(id),
        api.listServiceAccounts(id),
      ]);
      setTenant(t);
      setUsers(u);
      setAccounts(a);
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load(); }, [id]);

  const openEdit = () => {
    setQuotasText(JSON.stringify(tenant?.quotas ?? {}, null, 2));
    setShowEdit(true);
  };

  const saveQuotas = async () => {
    if (!id || !tenant) return;
    let quotas: Record<string, any>;
    try {
      quotas = JSON.parse(quotasText);
    } catch {
      return toast("Invalid JSON", "error");
    }
    try {
      await api.updateTenantQuotas(id, quotas);
      toast("Quotas updated");
      setShowEdit(false);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  if (loading) return <EmptyState message="Loading..." />;
  if (!tenant) return <EmptyState message="Tenant not found" />;

  const userColumns = [
    { key: "email", header: "Email", render: (u: any) => <span className="font-medium">{u.email}</span> },
    { key: "status", header: "Status", render: (u: any) => u.is_suspended
      ? <span className="inline-flex items-center rounded bg-red-100 px-2 py-0.5 text-xs font-medium text-red-800">Suspended</span>
      : <span className="inline-flex items-center rounded bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800">Active</span> },
    { key: "created_at", header: "Created", render: (u: any) => new Date(u.created_at).toLocaleString() },
  ];

  const accountColumns = [
    { key: "label", header: "Label", render: (a: any) => <span className="font-medium">{a.label}</span> },
    { key: "description", header: "Description", render: (a: any) => a.description || "—" },
    { key: "created_at", header: "Created", render: (a: any) => new Date(a.created_at).toLocaleString() },
    { key: "actions", header: "", render: (a: any) => <LinkButton size="sm" variant="outline" to={`/service-accounts/${a.id}`}>View</LinkButton> },
  ];

  return (
    <div>
      <PageHeader title={tenant.name} subtitle={`Tenant ID: ${tenant.id}`} />

      <Card className="mb-6">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div>
            <div className="text-xs text-gray-500">ID</div>
            <code className="text-sm text-gray-900">{tenant.id}</code>
          </div>
          <div>
            <div className="text-xs text-gray-500">Name</div>
            <div className="text-sm text-gray-900">{tenant.name}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Created</div>
            <div className="text-sm text-gray-900">{new Date(tenant.created_at).toLocaleString()}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Quotas</div>
            <code className="text-sm text-gray-900">{JSON.stringify(tenant.quotas ?? {})}</code>
          </div>
        </div>
        <div className="mt-4">
          <Button size="sm" variant="outline" onClick={openEdit}>Edit Quotas</Button>
        </div>
      </Card>

      <Card title="Users" className="mb-6">
        {users.length === 0 ? (
          <EmptyState message="No users in this tenant." />
        ) : (
          <Table columns={userColumns} data={users} rowKey={(u) => u.id} />
        )}
      </Card>

      <Card title="Service Accounts">
        {accounts.length === 0 ? (
          <EmptyState message="No service accounts in this tenant." />
        ) : (
          <Table columns={accountColumns} data={accounts} rowKey={(a) => a.id} />
        )}
      </Card>

      <div className="mt-6">
        <Button variant="ghost" onClick={() => navigate("/tenants")}>← Back to Tenants</Button>
      </div>

      <Modal
        open={showEdit}
        title={`Edit Quotas — ${tenant.name}`}
        message="Update the per-project quota JSON for this tenant."
        confirmLabel="Save"
        variant="primary"
        onConfirm={saveQuotas}
        onCancel={() => setShowEdit(false)}
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
