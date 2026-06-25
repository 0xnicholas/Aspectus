import { useState, useEffect } from "react";
import { Button, Input, PageHeader, Select, Table, Badge, toast, EmptyState, Modal } from "../components/ui";
import { api } from "../api/client";

const ROLE_TYPES = [
  { value: "user", label: "User" },
  { value: "service_account", label: "Service Account" },
  { value: "both", label: "Both" },
];

interface Role {
  id: string;
  name: string;
  description?: string;
  type: string;
  is_default: boolean;
  is_system: boolean;
  scopes: string[];
}

export function Roles() {
  const [roles, setRoles] = useState<Role[]>([]);
  const [userId, setUserId] = useState("");
  const [selectedRole, setSelectedRole] = useState("");
  const [loading, setLoading] = useState(false);

  const [showForm, setShowForm] = useState(false);
  const [editingRole, setEditingRole] = useState<Role | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [roleType, setRoleType] = useState("user");
  const [scopesText, setScopesText] = useState("");
  const [formLoading, setFormLoading] = useState(false);

  const [deletingRole, setDeletingRole] = useState<Role | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      setRoles(await api.listRoles());
    } catch {
      toast("Failed to load roles", "error");
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  useEffect(() => {
    if (editingRole) {
      setName(editingRole.name);
      setDescription(editingRole.description || "");
      setRoleType(editingRole.type);
      setScopesText((editingRole.scopes || []).join(" "));
    } else {
      setName("");
      setDescription("");
      setRoleType("user");
      setScopesText("");
    }
  }, [editingRole, showForm]);

  const parseScopes = (text: string): string[] =>
    text.split(/[\s,]+/).map(s => s.trim()).filter(Boolean);

  const save = async () => {
    const scopes = parseScopes(scopesText);
    if (!editingRole && !name.trim()) return toast("Role name is required", "error");
    if (scopes.length === 0) return toast("At least one scope is required", "error");

    setFormLoading(true);
    try {
      if (editingRole) {
        await api.updateRole(editingRole.id, {
          description: description.trim() || undefined,
          type: roleType,
          scopes,
        });
        toast("Role updated");
      } else {
        await api.createRole({
          name: name.trim(),
          description: description.trim() || undefined,
          type: roleType,
          scopes,
        });
        toast("Role created");
      }
      setShowForm(false);
      setEditingRole(null);
      load();
    } catch (e: any) {
      toast(e.message || "Failed to save role", "error");
    }
    setFormLoading(false);
  };

  const remove = async () => {
    if (!deletingRole) return;
    setDeleteLoading(true);
    try {
      await api.deleteRole(deletingRole.id);
      toast("Role deleted");
      setDeletingRole(null);
      load();
    } catch (e: any) {
      toast(e.message || "Failed to delete role", "error");
    }
    setDeleteLoading(false);
  };

  const assign = async () => {
    if (!userId || !selectedRole) return toast("Select user and role", "error");
    try {
      await api.assignRole(userId, selectedRole);
      toast("Role assigned!");
      setSelectedRole("");
    } catch (e: any) { toast(e.message, "error"); }
  };

  const typeBadge = (t: string) => <Badge variant="info">{t}</Badge>;

  const columns = [
    { key: "name", header: "Name", render: (r: Role) => <span className="font-medium">{r.name}</span> },
    { key: "type", header: "Type", render: (r: Role) => typeBadge(r.type) },
    { key: "default", header: "Default", render: (r: Role) => r.is_default ? "✅" : "" },
    { key: "system", header: "System", render: (r: Role) => r.is_system ? "🔒" : "" },
    { key: "description", header: "Description", render: (r: Role) => <span className="text-sm text-gray-500">{r.description || "—"}</span> },
    { key: "scopes", header: "Scopes", render: (r: Role) => (
      <div className="flex flex-wrap gap-1">
        {(r.scopes || []).length === 0 ? (
          <span className="text-xs text-gray-400">No scopes</span>
        ) : (
          r.scopes.map((s: string) => (
            <code key={s} className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-700">{s}</code>
          ))
        )}
      </div>
    )},
    { key: "actions", header: "Actions", render: (r: Role) => (
      <div className="flex gap-2">
        <Button size="sm" variant="outline" onClick={() => { setEditingRole(r); setShowForm(true); }} disabled={r.is_system}>
          Edit
        </Button>
        <Button size="sm" variant="destructive" onClick={() => setDeletingRole(r)} disabled={r.is_system}>
          Delete
        </Button>
      </div>
    )},
  ];

  return (
    <div>
      <div className="flex items-start justify-between">
        <PageHeader title="Roles" subtitle="View, create, and manage role definitions and assign roles to users." />
        <Button onClick={() => { setEditingRole(null); setShowForm(true); }}>Create Role</Button>
      </div>

      <div className="mb-6 rounded-xl border border-border bg-white p-5">
        <h3 className="mb-3 text-sm font-semibold text-gray-900">Assign Role to User</h3>
        <div className="flex flex-wrap items-end gap-3">
          <Input label="User ID" value={userId} onChange={e => setUserId(e.target.value)} placeholder="user_..." />
          <Select
            label="Role"
            value={selectedRole}
            onChange={setSelectedRole}
            placeholder="Select role..."
            options={roles.map(r => ({ value: r.id, label: r.name }))}
          />
          <Button onClick={assign}>Assign</Button>
        </div>
      </div>

      {roles.length === 0 && !loading ? (
        <EmptyState message="No roles found." />
      ) : (
        <Table columns={columns} data={roles} rowKey={r => r.id} />
      )}

      <Modal
        open={showForm}
        title={editingRole ? "Edit Role" : "Create Role"}
        message=""
        confirmLabel={editingRole ? "Save" : "Create"}
        variant="primary"
        onConfirm={save}
        onCancel={() => setShowForm(false)}
        loading={formLoading}
      >
        <div className="mt-4 space-y-4">
          <Input
            label="Name"
            value={name}
            onChange={e => setName(e.target.value)}
            disabled={!!editingRole}
            placeholder="custom_role"
          />
          <Input
            label="Description"
            value={description}
            onChange={e => setDescription(e.target.value)}
            placeholder="What this role is for"
          />
          <Select label="Type" value={roleType} onChange={setRoleType} options={ROLE_TYPES} />
          <div>
            <label className="text-sm font-medium text-gray-600">Scopes</label>
            <textarea
              value={scopesText}
              onChange={e => setScopesText(e.target.value)}
              placeholder="project:resource:action (space or comma separated)"
              rows={4}
              className="mt-1 w-full rounded-md border border-border px-3 py-2 text-sm outline-none focus:border-primary focus:ring-1 focus:ring-primary"
            />
            <p className="mt-1 text-xs text-gray-400">Separate scopes with spaces or commas.</p>
          </div>
        </div>
      </Modal>

      <Modal
        open={!!deletingRole}
        title="Delete Role"
        message={`Are you sure you want to delete the role "${deletingRole?.name}"? This cannot be undone.`}
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={remove}
        onCancel={() => setDeletingRole(null)}
        loading={deleteLoading}
      />
    </div>
  );
}
