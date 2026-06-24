import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Button, Badge, Card, CopyButton, DateInput, Input, Modal, PageHeader, Select, Table, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

const CONSUMER_PROJECTS = ["pandaria", "emerald", "constell", "tokencamp", "heirloom"];

export function ServiceAccountDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [account, setAccount] = useState<any>(null);
  const [keys, setKeys] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [project, setProject] = useState("pandaria");
  const [scopes, setScopes] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [newKey, setNewKey] = useState<string | null>(null);
  const [revokeTarget, setRevokeTarget] = useState<string | null>(null);

  const load = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const [a, k] = await Promise.all([api.getServiceAccount(id), api.listApiKeys(id)]);
      setAccount(a);
      setKeys(k);
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load(); }, [id]);

  const createKey = async () => {
    if (!id) return;
    try {
      const res = await api.createApiKey({
        owner_type: "service_account",
        owner_id: id,
        project,
        scopes: scopes ? scopes.split(",").map((s) => s.trim()).filter(Boolean) : [],
        expires_at: expiresAt ? new Date(expiresAt).toISOString() : undefined,
      });
      setNewKey(res.key);
      toast("Key created!");
      setScopes("");
      setExpiresAt("");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const revoke = async () => {
    if (!revokeTarget) return;
    try {
      await api.revokeApiKey(revokeTarget);
      toast("Key revoked");
      setRevokeTarget(null);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  if (loading) return <EmptyState message="Loading..." />;
  if (!account) return <EmptyState message="Service account not found" />;

  const columns = [
    { key: "key_prefix", header: "Prefix", width: 160, render: (k: any) => <code className="text-xs text-gray-500">{k.key_prefix}</code> },
    { key: "project", header: "Project", width: 100 },
    { key: "scopes", header: "Scopes", render: (k: any) => <span className="text-xs">{(k.scopes || []).join(", ") || "—"}</span> },
    { key: "expires_at", header: "Expires", width: 160, render: (k: any) => k.expires_at ? new Date(k.expires_at).toLocaleString() : "—" },
    { key: "status", header: "Status", width: 100, render: (k: any) => k.revoked_at ? <Badge variant="destructive">Revoked</Badge> : <Badge variant="success">Active</Badge> },
    { key: "actions", header: "", width: 100, render: (k: any) => !k.revoked_at && <Button size="sm" variant="destructive" onClick={() => setRevokeTarget(k.id)}>Revoke</Button> },
  ];

  return (
    <div>
      <PageHeader title={account.label} subtitle={`Service Account ID: ${account.id}`} />

      {newKey && (
        <div className="mb-6 rounded-xl border border-yellow-400 bg-yellow-50 p-4">
          <div className="flex items-center justify-between">
            <strong className="text-sm text-yellow-900">⚠️ Copy this key now — it won't be shown again</strong>
            <Button size="sm" variant="ghost" onClick={() => setNewKey(null)}>Dismiss</Button>
          </div>
          <div className="mt-3 flex items-center gap-2 rounded-lg bg-white p-3">
            <code className="flex-1 break-all text-xs">{newKey}</code>
            <CopyButton text={newKey} />
          </div>
        </div>
      )}

      <Card className="mb-6">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div>
            <div className="text-xs text-gray-500">ID</div>
            <code className="text-sm text-gray-900">{account.id}</code>
          </div>
          <div>
            <div className="text-xs text-gray-500">Label</div>
            <div className="text-sm text-gray-900">{account.label}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Tenant</div>
            <code className="text-sm text-gray-900">{account.tenant_id}</code>
          </div>
          <div>
            <div className="text-xs text-gray-500">Description</div>
            <div className="text-sm text-gray-900">{account.description || "—"}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Created</div>
            <div className="text-sm text-gray-900">{new Date(account.created_at).toLocaleString()}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Expires</div>
            <div className="text-sm text-gray-900">{account.expires_at ? new Date(account.expires_at).toLocaleString() : "—"}</div>
          </div>
        </div>
      </Card>

      <Card title="Create API Key" className="mb-6">
        <div className="flex flex-wrap items-end gap-3">
          <Select
            label="Project"
            value={project}
            onChange={setProject}
            options={CONSUMER_PROJECTS.map((p) => ({ value: p, label: p }))}
          />
          <Input label="Scopes (comma)" value={scopes} onChange={(e) => setScopes(e.target.value)} placeholder="pandaria:session:read" />
          <DateInput label="Expires At (optional)" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} />
          <Button onClick={createKey}>Create</Button>
        </div>
      </Card>

      <Card title="API Keys">
        {keys.length === 0 ? (
          <EmptyState message="No API keys for this service account." />
        ) : (
          <Table columns={columns} data={keys} rowKey={(k) => k.id} />
        )}
      </Card>

      <div className="mt-6">
        <Button variant="ghost" onClick={() => navigate("/service-accounts")}>← Back to Service Accounts</Button>
      </div>

      <Modal
        open={!!revokeTarget}
        title="Revoke API Key"
        message="This action cannot be undone. The key will stop working immediately."
        confirmLabel="Revoke"
        variant="destructive"
        onConfirm={revoke}
        onCancel={() => setRevokeTarget(null)}
      />
    </div>
  );
}
