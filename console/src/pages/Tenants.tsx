import { useState } from "react";
import { Button, Input } from "../components/ui";
import { api } from "../api/client";
import { toast } from "../components/ui";

export function Tenants() {
  const [name, setName] = useState("");

  const create = async () => {
    if (!name) return toast("Name required", "error");
    try { const t = await api.createTenant(name); toast(`Created: ${t.id}`); setName(""); }
    catch (e: any) { toast(e.message, "error"); }
  };

  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900">Tenants</h1>
      <p className="mt-1 text-gray-500">Create and manage tenants in the Pandaria ecosystem.</p>
      <div className="mt-6 flex items-end gap-3">
        <Input label="Tenant Name" value={name} onChange={e => setName(e.target.value)} placeholder="Acme Corp" />
        <Button onClick={create}>Create</Button>
      </div>
    </div>
  );
}
