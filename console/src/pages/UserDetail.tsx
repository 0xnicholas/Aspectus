import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Button, Badge, Card, PageHeader, Select, Table, Modal, toast, EmptyState } from "../components/ui";
import { api } from "../api/client";

export function UserDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [user, setUser] = useState<any>(null);
  const [scopes, setScopes] = useState<string[]>([]);
  const [roles, setRoles] = useState<any[]>([]);
  const [allRoles, setAllRoles] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedRole, setSelectedRole] = useState("");
  const [removeTarget, setRemoveTarget] = useState<any>(null);

  const load = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const [u, s, r] = await Promise.all([
        api.getUser(id),
        api.getUserScopes(id),
        api.getUserRoles(id),
      ]);
      setUser(u);
      setScopes(s.scopes || []);
      setRoles(r);
    } catch (e: any) {
      toast(e.message, "error");
    }
    setLoading(false);
  };

  const loadAllRoles = async () => {
    try {
      const res = await api.listRoles();
      setAllRoles(res.filter((r: any) => r.type === "user" || r.type === "both"));
    } catch {
      toast("Failed to load roles", "error");
    }
  };

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load(); loadAllRoles(); }, [id]);

  const assignRole = async () => {
    if (!id || !selectedRole) return toast("Select a role", "error");
    try {
      await api.assignRole(id, selectedRole);
      toast("Role assigned");
      setSelectedRole("");
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  const removeRole = async () => {
    if (!id || !removeTarget) return;
    try {
      await api.removeRole(id, removeTarget.id);
      toast("Role removed");
      setRemoveTarget(null);
      load();
    } catch (e: any) {
      toast(e.message, "error");
    }
  };

  if (loading) return <EmptyState message="Loading..." />;
  if (!user) return <EmptyState message="User not found" />;

  const roleColumns = [
    { key: "name", header: "Role", render: (r: any) => <span className="font-medium">{r.name}</span> },
    { key: "type", header: "Type", render: (r: any) => <Badge variant="info">{r.type}</Badge> },
    { key: "description", header: "Description", render: (r: any) => <span className="text-sm text-gray-500">{r.description || "—"}</span> },
    { key: "actions", header: "", render: (r: any) => (
      <Button size="sm" variant="destructive" onClick={() => setRemoveTarget(r)}>Remove</Button>
    )},
  ];

  return (
    <div>
      <PageHeader title={user.display_name || user.email || "User"} subtitle={`User ID: ${user.id}`} />

      <Card className="mb-6">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div>
            <div className="text-xs text-gray-500">ID</div>
            <code className="text-sm text-gray-900">{user.id}</code>
          </div>
          <div>
            <div className="text-xs text-gray-500">Tenant</div>
            <code className="text-sm text-gray-900">{user.tenant_id}</code>
          </div>
          <div>
            <div className="text-xs text-gray-500">Email</div>
            <div className="text-sm text-gray-900">{user.email || "—"}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Status</div>
            <div className="text-sm text-gray-900">
              {user.is_suspended ? (
                <Badge variant="destructive">Suspended</Badge>
              ) : (
                <Badge variant="success">Active</Badge>
              )}
            </div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Created</div>
            <div className="text-sm text-gray-900">{new Date(user.created_at).toLocaleString()}</div>
          </div>
          <div>
            <div className="text-xs text-gray-500">Last Sign In</div>
            <div className="text-sm text-gray-900">{user.last_sign_in_at ? new Date(user.last_sign_in_at).toLocaleString() : "—"}</div>
          </div>
        </div>
      </Card>

      <Card title="Effective Scopes" className="mb-6">
        {scopes.length === 0 ? (
          <EmptyState message="No scopes resolved for this user." />
        ) : (
          <div className="flex flex-wrap gap-2">
            {scopes.map((s) => (
              <code key={s} className="rounded bg-gray-100 px-2 py-1 text-xs text-gray-700">{s}</code>
            ))}
          </div>
        )}
      </Card>

      <Card title="Assigned Roles" className="mb-6">
        <div className="mb-4 flex flex-wrap items-end gap-3">
          <Select
            label="Assign Role"
            value={selectedRole}
            onChange={setSelectedRole}
            placeholder="Select role..."
            options={allRoles
              .filter((r) => !roles.some((ur) => ur.id === r.id))
              .map((r) => ({ value: r.id, label: r.name }))}
          />
          <Button onClick={assignRole}>Assign</Button>
        </div>
        {roles.length === 0 ? (
          <EmptyState message="No roles assigned to this user." />
        ) : (
          <Table columns={roleColumns} data={roles} rowKey={(r) => r.id} />
        )}
      </Card>

      <div className="mt-6">
        <Button variant="ghost" onClick={() => navigate("/users")}>← Back to Users</Button>
      </div>

      <Modal
        open={!!removeTarget}
        title="Remove Role"
        message={`Remove role "${removeTarget?.name}" from this user?`}
        confirmLabel="Remove"
        variant="destructive"
        onConfirm={removeRole}
        onCancel={() => setRemoveTarget(null)}
      />
    </div>
  );
}
