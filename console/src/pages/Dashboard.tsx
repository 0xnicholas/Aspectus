export function Dashboard() {
  return (
    <div>
      <h1>Dashboard</h1>
      <p style={{ color: "#666" }}>Aspectus Admin Console v0.8.0</p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 16, marginTop: 24 }}>
        {[
          { label: "Tenants", path: "/tenants" },
          { label: "Users", path: "/users" },
          { label: "API Keys", path: "/api-keys" },
        ].map((card) => (
          <a
            key={card.path}
            href={card.path}
            style={{
              padding: 24, background: "#fff", borderRadius: 8,
              textDecoration: "none", color: "#333", border: "1px solid #e0e0e0",
            }}
          >
            <h3>{card.label}</h3>
            <p style={{ color: "#888", fontSize: 14 }}>Manage {card.label.toLowerCase()}</p>
          </a>
        ))}
      </div>
    </div>
  );
}
