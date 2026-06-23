import { useState } from "react";
import { Button, Input, Select, DateInput, PageHeader, Table, Badge, Modal, CopyButton, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

const CONSUMER_PROJECTS = ["pandaria", "emerald", "constell", "tokencamp", "heirloom"];
const OWNER_TYPES = [
  { value: "service_account", label: "Service Account" },
  { value: "user", label: "User" },
];

export function ApiKeys() {
  const [ownerType, setOwnerType] = useState("service_account");
  const [ownerId, setOwnerId] = useState("");
  const [project, setProject] = useState("pandaria");
  const [scopes, setScopes] = useState("");
  const [expiresAt, setExpiresAt] = useState("");

  const [listSaId, setListSaId] = useState("");
  const [keys, setKeys] = useState<any[]>([]);
  const [newKey, setNewKey] = useState<string | null>(null);
  const [revokeTarget, setRevokeTarget] = useState<string | null>(null);

  const list = async () => {
    if (!listSaId) return toast("Enter a Service Account ID", "error");
    try {
      setKeys(await api.listApiKeys(listSaId));
    } catch {
      toast("Failed to load keys", "error");
    }
  };

  const create = async () => {
    if (!ownerId) return toast("Owner ID required", "error");
    try {
      const res = await api.createApiKey({
        owner_type: ownerType,
        owner_id: ownerId,
        project,
        scopes: scopes ? scopes.split(",").map((s) => s.trim()).filter(Boolean) : [],
        expires_at: expiresAt ? new Date(expiresAt).toISOString() : undefined,
      });
      setNewKey(res.key);
      toast("Key created!");
      if (ownerType === "service_account") {
        setListSaId(ownerId);
        setKeys(await api.listApiKeys(ownerId));
      }
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
      list();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

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
      <PageHeader title="API Keys" subtitle="Create and revoke per-tenant, per-project API keys." />

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

      <div className="rounded-xl border border-border bg-white p-5">
        <h3 className="mb-3 text-sm font-semibold text-gray-900">Create API Key</h3>
        <div className="flex flex-wrap items-end gap-3">
          <Select
            label="Owner Type"
            value={ownerType}
            onChange={setOwnerType}
            options={OWNER_TYPES}
          />
          <Input label="Owner ID" value={ownerId} onChange={(e) => setOwnerId(e.target.value)} placeholder={ownerType === "service_account" ? "sa_..." : "user_..."} />
          <Select
            label="Project"
            value={project}
            onChange={setProject}
            options={CONSUMER_PROJECTS.map((p) => ({ value: p, label: p }))}
          />
          <Input label="Scopes (comma)" value={scopes} onChange={(e) => setScopes(e.target.value)} placeholder="pandaria:session:read" />
          <DateInput label="Expires At (optional)" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} />
          <Button onClick={create}>Create</Button>
        </div>
      </div>

      <div className="mt-6 rounded-xl border border-border bg-white p-5">
        <h3 className="mb-3 text-sm font-semibold text-gray-900">List Keys by Service Account</h3>
        <div className="flex flex-wrap items-end gap-3">
          <Input label="Service Account ID" value={listSaId} onChange={(e) => setListSaId(e.target.value)} />
          <Button onClick={list}>List</Button>
        </div>
      </div>

      {keys.length === 0 ? (
        <EmptyState message="No keys loaded. Enter a Service Account ID and click List." />
      ) : (
        <Table columns={columns} data={keys} rowKey={(k) => k.id} />
      )}

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
