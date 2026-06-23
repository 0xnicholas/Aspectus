import { useEffect, useState } from "react";
import { Button, Input, DateInput, Select, PageHeader, Table, Pagination, Modal, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

const LIMIT_OPTIONS = [50, 100, 250, 500, 1000].map((n) => ({ value: String(n), label: String(n) }));

export function AuditLogs() {
  const [logs, setLogs] = useState<any[]>([]);
  const [filters, setFilters] = useState({
    tenant_id: "",
    action: "",
    target_type: "",
    target_id: "",
    actor_id: "",
    from: "",
    to: "",
  });
  const [limit, setLimit] = useState(100);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<any>(null);

  const search = async (nextOffset = offset) => {
    setLoading(true);
    try {
      const res = await api.listAuditLogs({
        ...filters,
        from: filters.from ? new Date(filters.from).toISOString() : undefined,
        to: filters.to ? new Date(filters.to).toISOString() : undefined,
        limit,
        offset: nextOffset,
      });
      setLogs(res);
      setOffset(nextOffset);
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  useEffect(() => {
    search(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [limit]);

  const updateFilter = (key: keyof typeof filters, value: string) => {
    setFilters((f) => ({ ...f, [key]: value }));
  };

  const columns = [
    { key: "created_at", header: "Timestamp", render: (l: any) => new Date(l.created_at).toLocaleString() },
    { key: "action", header: "Action", render: (l: any) => <code className="text-xs">{l.action}</code> },
    { key: "actor", header: "Actor", render: (l: any) => <span className="text-sm text-gray-600">{l.actor_type}:{l.actor_id}</span> },
    { key: "target", header: "Target", render: (l: any) => <span className="text-sm text-gray-600">{l.target_type}:{l.target_id}</span> },
    { key: "tenant_id", header: "Tenant", render: (l: any) => <code className="text-xs text-gray-500">{l.tenant_id || "—"}</code> },
    { key: "details", header: "", render: (l: any) => <Button size="sm" variant="ghost" onClick={() => setSelected(l)}>Details</Button> },
  ];

  return (
    <div>
      <PageHeader title="Audit Logs" subtitle="Query the append-only audit log of management actions." />

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-4">
        <Input label="Tenant ID" value={filters.tenant_id} onChange={(e) => updateFilter("tenant_id", e.target.value)} />
        <Input label="Action" value={filters.action} onChange={(e) => updateFilter("action", e.target.value)} placeholder="e.g. api_key.created" />
        <Input label="Target Type" value={filters.target_type} onChange={(e) => updateFilter("target_type", e.target.value)} placeholder="e.g. api_key" />
        <Input label="Target ID" value={filters.target_id} onChange={(e) => updateFilter("target_id", e.target.value)} />
        <Input label="Actor ID" value={filters.actor_id} onChange={(e) => updateFilter("actor_id", e.target.value)} />
        <DateInput label="From" value={filters.from} onChange={(e) => updateFilter("from", e.target.value)} />
        <DateInput label="To" value={filters.to} onChange={(e) => updateFilter("to", e.target.value)} />
        <Select
          label="Limit"
          value={String(limit)}
          onChange={(v) => { setLimit(Number(v)); setOffset(0); }}
          options={LIMIT_OPTIONS}
        />
      </div>

      <div className="mt-4 flex gap-3">
        <Button onClick={() => search(0)} loading={loading}>Search</Button>
        <Button variant="outline" onClick={() => { setFilters({ tenant_id: "", action: "", target_type: "", target_id: "", actor_id: "", from: "", to: "" }); setOffset(0); search(0); }}>Reset</Button>
      </div>

      {logs.length === 0 && !loading ? (
        <EmptyState message="No audit logs match the current filters." />
      ) : (
        <Table columns={columns} data={logs} rowKey={(l) => l.id} />
      )}

      <Pagination
        offset={offset}
        limit={limit}
        hasMore={logs.length === limit}
        onChange={(next) => search(next)}
      />

      <Modal
        open={!!selected}
        title="Audit Log Details"
        message={
          selected
            ? `${selected.action} at ${new Date(selected.created_at).toLocaleString()}`
            : ""
        }
        confirmLabel="Close"
        variant="primary"
        onConfirm={() => setSelected(null)}
        onCancel={() => setSelected(null)}
      >
        {selected && (
          <pre className="mt-4 max-h-96 overflow-auto rounded-lg bg-gray-900 p-4 text-xs text-gray-100">
            {JSON.stringify(selected.metadata, null, 2)}
          </pre>
        )}
      </Modal>
    </div>
  );
}
