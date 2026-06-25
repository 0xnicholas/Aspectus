import { BrowserRouter, Routes, Route, Link, useLocation } from "react-router-dom";
import { LayoutDashboard, Building2, UsersIcon, Key, Shield, Globe, ScrollText, UserCog, Lock, BookOpen } from "lucide-react";
import { Dashboard } from "./pages/Dashboard";
import { Tenants } from "./pages/Tenants";
import { Users } from "./pages/Users";
import { ServiceAccounts } from "./pages/ServiceAccounts";
import { ApiKeys } from "./pages/ApiKeys";
import { Roles } from "./pages/Roles";
import { Clients } from "./pages/Clients";
import { AuditLogs } from "./pages/AuditLogs";
import { ServiceTokens } from "./pages/ServiceTokens";
import { TenantDetail } from "./pages/TenantDetail";
import { ServiceAccountDetail } from "./pages/ServiceAccountDetail";
import { UserDetail } from "./pages/UserDetail";

const NAV = [
  { path: "/", label: "Dashboard", icon: LayoutDashboard },
  { path: "/tenants", label: "Tenants", icon: Building2 },
  { path: "/users", label: "Users", icon: UsersIcon },
  { path: "/service-accounts", label: "Service Accounts", icon: UserCog },
  { path: "/api-keys", label: "API Keys", icon: Key },
  { path: "/roles", label: "Roles", icon: Shield },
  { path: "/clients", label: "OAuth2 Clients", icon: Globe },
  { path: "/service-tokens", label: "Service Tokens", icon: Lock },
  { path: "/audit-logs", label: "Audit Logs", icon: ScrollText },
  { path: "/docs", label: "API Docs", icon: BookOpen, external: true },
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
          const active = !item.external && (location.pathname === item.path || (item.path !== "/" && location.pathname.startsWith(item.path)));
          const Icon = item.icon;
          const className = `flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors ${
            active ? "bg-sidebar-active text-white" : "hover:bg-sidebar-hover hover:text-white"
          }`;
          return item.external ? (
            <a key={item.path} href={item.path} target="_blank" rel="noreferrer" className={className}>
              <Icon size={18} />
              {item.label}
            </a>
          ) : (
            <Link
              key={item.path}
              to={item.path}
              className={className}
            >
              <Icon size={18} />
              {item.label}
            </Link>
          );
        })}
      </nav>
      <div className="border-t border-white/10 p-4 text-xs text-gray-500">v{import.meta.env.PACKAGE_VERSION || "dev"}</div>
    </aside>
  );
}

function NotFound() {
  return (
    <div className="flex h-full flex-col items-center justify-center text-center">
      <h1 className="text-4xl font-bold text-gray-900">404</h1>
      <p className="mt-2 text-gray-500">This page does not exist.</p>
      <Link to="/" className="mt-6 text-sm font-medium text-primary hover:underline">← Back to Dashboard</Link>
    </div>
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
            <Route path="/tenants/:id" element={<TenantDetail />} />
            <Route path="/users" element={<Users />} />
            <Route path="/users/:id" element={<UserDetail />} />
            <Route path="/service-accounts" element={<ServiceAccounts />} />
            <Route path="/service-accounts/:id" element={<ServiceAccountDetail />} />
            <Route path="/api-keys" element={<ApiKeys />} />
            <Route path="/roles" element={<Roles />} />
            <Route path="/clients" element={<Clients />} />
            <Route path="/service-tokens" element={<ServiceTokens />} />
            <Route path="/audit-logs" element={<AuditLogs />} />
            <Route path="*" element={<NotFound />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}
