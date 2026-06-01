import { useState, useEffect } from "react";
import { Button, Input, Table, toast } from "../components/ui";
import { api } from "../api/client";

export function Clients() {
  const [clients, setClients] = useState<any[]>([]);
  const [name, setName] = useState("");
  const [uris, setUris] = useState("");

  useEffect(() => { load(); }, []);

  const load = async () => {
    try { setClients(await api.listClients()); } catch { toast("Failed to load clients", "error"); }
  };

  const create = async () => {
    if (!name || !uris) return toast("Name and redirect URIs required", "error");
    try {
      await api.createClient({ name, redirect_uris: uris.split(",").map(s => s.trim()) });
      toast("Client created!"); setName(""); setUris(""); load();
    } catch (e: any) { toast(e.message, "error"); }
  };

  const columns = [
    { key: "client_id", header: "Client ID", render: (c: any) => <code style={{ fontSize: 12 }}>{c.client_id}</code> },
    { key: "name", header: "Name" },
    { key: "redirect_uris", header: "Redirect URIs", render: (c: any) => <span style={{ fontSize: 12 }}>{(c.redirect_uris || []).join(", ")}</span> },
  ];

  return (
    <div>
      <h1>OAuth2 Clients</h1>
      <p style={{ color: "#666", marginTop: 8 }}>Register OAuth2 clients for authorization code flow.</p>
      <div style={{ display: "flex", gap: 12, marginTop: 20, alignItems: "flex-end", flexWrap: "wrap" }}>
        <Input label="Client Name" value={name} onChange={e => setName(e.target.value)} placeholder="e.g. Pandaria Web" />
        <Input label="Redirect URIs (comma separated)" value={uris} onChange={e => setUris(e.target.value)} placeholder="https://pandaria.io/cb" />
        <Button onClick={create}>Register</Button>
      </div>
      <Table columns={columns} data={clients} rowKey={c => c.client_id} />
    </div>
  );
}
