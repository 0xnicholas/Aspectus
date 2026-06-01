import { BrowserRouter, Routes, Route, Link, useLocation } from "react-router-dom";
import { LayoutDashboard, Building2, UsersIcon, Key, Shield, Globe, ScrollText, UserCog } from "lucide-react";
import { Dashboard } from "./pages/Dashboard";
import { Tenants } from "./pages/Tenants";
import { Users } from "./pages/Users";
import { ServiceAccounts } from "./pages/ServiceAccounts";
import { ApiKeys } from "./pages/ApiKeys";
import { Roles } from "./pages/Roles";
import { Clients } from "./pages/Clients";
import { AuditLogs } from "./pages/AuditLogs";

const NAV = [
  { path: "/", label: "Dashboard", icon: LayoutDashboard },
  { path: "/tenants", label: "Tenants", icon: Building2 },
  { path: "/users", label: "Users", icon: UsersIcon },
  { path: "/service-accounts", label: "Service Accounts", icon: UserCog },
  { path: "/api-keys", label: "API Keys", icon: Key },
  { path: "/roles", label: "Roles", icon: Shield },
  { path: "/clients", label: "OAuth2 Clients", icon: Globe },
  { path: "/audit-logs", label: "Audit Logs", icon: ScrollText },
];

function Sidebar() {
  const location = useLocation();
  return (
    <aside className="flex h-screen w-60 flex-col bg-sidebar text-gray-300">
      <div className="flex h-16 items-center gap-3 px-6 border-b border-white/10">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary-foreground/10 text-lg">🔐</div>
        <span className="text-lg font-semibold text-white">Aspectus</span>
      </div>
      <nav className="flex-1 space-y-1 p-3">
        {NAV.map((item) => {
          const active = location.pathname === item.path;
          const Icon = item.icon;
          return (
            <Link
              key={item.path}
              to={item.path}
              className={`flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors ${
                active ? "bg-sidebar-active text-white" : "hover:bg-sidebar-hover hover:text-white"
              }`}
            >
              <Icon size={18} />
              {item.label}
            </Link>
          );
        })}
      </nav>
      <div className="border-t border-white/10 p-4 text-xs text-gray-500">v0.1</div>
    </aside>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <div className="flex h-screen overflow-hidden">
        <Sidebar />
        <main className="flex-1 overflow-auto bg-muted p-6">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/tenants" element={<Tenants />} />
            <Route path="/users" element={<Users />} />
            <Route path="/service-accounts" element={<ServiceAccounts />} />
            <Route path="/api-keys" element={<ApiKeys />} />
            <Route path="/roles" element={<Roles />} />
            <Route path="/clients" element={<Clients />} />
            <Route path="/audit-logs" element={<AuditLogs />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}
