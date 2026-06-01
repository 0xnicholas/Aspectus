import { useState } from "react";
import { api } from "../api/client";

export function Tenants() {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  const create = async () => {
    if (!name) return;
    setLoading(true);
    try {
      await api.createTenant(name);
      setName("");
      alert("Tenant created!");
    } catch (e: any) {
      alert(e.message);
    }
    setLoading(false);
  };

  return (
    <div>
      <h1>Tenants</h1>
      <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
        <input
          placeholder="Tenant name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          style={{ padding: "8px 12px", borderRadius: 6, border: "1px solid #ccc", flex: 1 }}
        />
        <button
          onClick={create}
          disabled={loading}
          style={{ padding: "8px 16px", borderRadius: 6, background: "#1a1a2e", color: "#fff", border: "none", cursor: "pointer" }}
        >
          Create
        </button>
      </div>
    </div>
  );
}
