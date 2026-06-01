import { BrowserRouter, Routes, Route, Link, useLocation } from "react-router-dom";
import { Dashboard } from "./pages/Dashboard";
import { Tenants } from "./pages/Tenants";
import { Users } from "./pages/Users";
import { ApiKeys } from "./pages/ApiKeys";
import { Roles } from "./pages/Roles";
import { AuditLogs } from "./pages/AuditLogs";

const NAV = [
  { path: "/", label: "Dashboard" },
  { path: "/tenants", label: "Tenants" },
  { path: "/users", label: "Users" },
  { path: "/api-keys", label: "API Keys" },
  { path: "/roles", label: "Roles" },
  { path: "/audit-logs", label: "Audit Logs" },
];

function Sidebar() {
  const location = useLocation();
  return (
    <nav style={{ width: 220, background: "#1a1a2e", color: "#eee", minHeight: "100vh", padding: 16 }}>
      <h2 style={{ fontSize: 18, marginBottom: 24 }}>🔐 Aspectus</h2>
      {NAV.map((item) => (
        <Link
          key={item.path}
          to={item.path}
          style={{
            display: "block",
            padding: "8px 12px",
            margin: "4px 0",
            borderRadius: 6,
            textDecoration: "none",
            color: location.pathname === item.path ? "#fff" : "#aaa",
            background: location.pathname === item.path ? "#16213e" : "transparent",
          }}
        >
          {item.label}
        </Link>
      ))}
    </nav>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <div style={{ display: "flex" }}>
        <Sidebar />
        <main style={{ flex: 1, padding: 24, background: "#f5f5f5", minHeight: "100vh" }}>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/tenants" element={<Tenants />} />
            <Route path="/users" element={<Users />} />
            <Route path="/api-keys" element={<ApiKeys />} />
            <Route path="/roles" element={<Roles />} />
            <Route path="/audit-logs" element={<AuditLogs />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}
