import { useState } from "react";
import { Button, Input, toast } from "../components/ui";
import { api } from "../api/client";

export function Tenants() {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  const create = async () => {
    if (!name) return toast("Tenant name required", "error");
    setLoading(true);
    try {
      const t = await api.createTenant(name);
      toast(`Tenant created: ${t.id}`);
      setName("");
    } catch (e: any) { toast(e.message, "error"); }
    setLoading(false);
  };

  return (
    <div>
      <h1>Tenants</h1>
      <p style={{ color: "#666", marginTop: 8 }}>Create and manage tenants in the Pandaria ecosystem.</p>
      <div style={{ display: "flex", gap: 12, marginTop: 24, alignItems: "flex-end" }}>
        <Input label="Tenant Name" value={name} onChange={e => setName(e.target.value)} placeholder="e.g. Acme Corp" />
        <Button onClick={create} loading={loading}>Create Tenant</Button>
      </div>
    </div>
  );
}
