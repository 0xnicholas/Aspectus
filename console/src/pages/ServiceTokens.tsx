import { useEffect, useState } from "react";
import { Button, Modal, Select, PageHeader, Table, Badge, CopyButton, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

const CONSUMER_PROJECTS = ["pandaria", "emerald", "constell", "tokencamp", "heirloom"];

interface NewToken {
  project: string;
  token: string;
  token_prefix: string;
}

export function ServiceTokens() {
  const [tokens, setTokens] = useState<any[]>([]);
  const [project, setProject] = useState("pandaria");
  const [newToken, setNewToken] = useState<NewToken | null>(null);
  const [rotateTarget, setRotateTarget] = useState<string | null>(null);
  const [revokeTarget, setRevokeTarget] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      setTokens(await api.listServiceTokens());
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const create = async () => {
    try {
      const res = await api.createServiceToken(project);
      setNewToken({ project: res.project, token: res.token, token_prefix: res.token_prefix });
      toast("Service token created — copy it now");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const rotate = async () => {
    if (!rotateTarget) return;
    try {
      const res = await api.rotateServiceToken(rotateTarget);
      setNewToken({ project: res.project, token: res.token, token_prefix: res.token_prefix });
      toast("Service token rotated — copy the new token now");
      setRotateTarget(null);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const revoke = async () => {
    if (!revokeTarget) return;
    try {
      await api.revokeServiceToken(revokeTarget);
      toast("Service token revoked");
      setRevokeTarget(null);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const columns = [
    { key: "project", header: "Project", render: (t: any) => <span className="font-medium">{t.project}</span> },
    { key: "token_prefix", header: "Prefix", render: (t: any) => <code className="text-xs text-gray-500">{t.token_prefix || "—"}</code> },
    { key: "status", header: "Status", render: (t: any) => t.revoked_at
      ? <Badge variant="destructive">Revoked</Badge>
      : <Badge variant="success">Active</Badge> },
    { key: "created_at", header: "Created", render: (t: any) => new Date(t.created_at).toLocaleString() },
    { key: "updated_at", header: "Updated", render: (t: any) => new Date(t.updated_at).toLocaleString() },
    { key: "actions", header: "", render: (t: any) => (
      <div className="flex gap-2">
        {!t.revoked_at && (
          <>
            <Button size="sm" variant="outline" onClick={() => setRotateTarget(t.project)}>Rotate</Button>
            <Button size="sm" variant="destructive" onClick={() => setRevokeTarget(t.project)}>Revoke</Button>
          </>
        )}
      </div>
    )},
  ];

  return (
    <div>
      <PageHeader title="Service Tokens" subtitle="Manage ecosystem project tokens used to call /introspect." />

      {newToken && (
        <div className="mb-6 rounded-xl border border-yellow-400 bg-yellow-50 p-4">
          <div className="flex items-center justify-between">
            <strong className="text-sm text-yellow-900">⚠️ Copy this token now — it will not be shown again</strong>
            <Button size="sm" variant="ghost" onClick={() => setNewToken(null)}>Dismiss</Button>
          </div>
          <div className="mt-2 text-sm text-yellow-800">Project: <span className="font-medium">{newToken.project}</span> · Prefix: <code>{newToken.token_prefix}</code></div>
          <div className="mt-3 flex items-center gap-2 rounded-lg bg-white p-3">
            <code className="flex-1 break-all text-xs">{newToken.token}</code>
            <CopyButton text={newToken.token} />
          </div>
        </div>
      )}

      <div className="flex flex-wrap items-end gap-3">
        <Select
          label="Project"
          value={project}
          onChange={setProject}
          options={CONSUMER_PROJECTS.map((p) => ({ value: p, label: p }))}
        />
        <Button onClick={create}>Create Token</Button>
        <Button variant="outline" onClick={load} loading={loading}>Refresh</Button>
      </div>

      {tokens.length === 0 && !loading ? (
        <EmptyState message="No service tokens found. Create one to get started." />
      ) : (
        <Table columns={columns} data={tokens} rowKey={(t) => t.project} />
      )}

      <Modal
        open={!!rotateTarget}
        title="Rotate Service Token"
        message={`A new token will be generated for ${rotateTarget}. The old token will stop working immediately.`}
        confirmLabel="Rotate"
        variant="primary"
        onConfirm={rotate}
        onCancel={() => setRotateTarget(null)}
      />

      <Modal
        open={!!revokeTarget}
        title="Revoke Service Token"
        message={`The ${revokeTarget} token will be revoked immediately. This action cannot be undone.`}
        confirmLabel="Revoke"
        variant="destructive"
        onConfirm={revoke}
        onCancel={() => setRevokeTarget(null)}
      />
    </div>
  );
}
