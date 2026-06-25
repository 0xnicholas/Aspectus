import { Link } from "react-router-dom";
import { Building2, Users, Key, Shield, Globe, ScrollText, UserCog, Lock } from "lucide-react";

const CARDS = [
  { label: "Tenants", desc: "Create tenants and edit quotas", icon: Building2, path: "/tenants" },
  { label: "Users", desc: "Manage human users and roles", icon: Users, path: "/users" },
  { label: "Service Accounts", desc: "Machine accounts and their keys", icon: UserCog, path: "/service-accounts" },
  { label: "API Keys", desc: "Create and revoke scoped keys", icon: Key, path: "/api-keys" },
  { label: "Roles", desc: "View roles and assign to users", icon: Shield, path: "/roles" },
  { label: "OAuth2 Clients", desc: "Register authorization code clients", icon: Globe, path: "/clients" },
  { label: "Service Tokens", desc: "Rotate ecosystem introspect tokens", icon: Lock, path: "/service-tokens" },
  { label: "Audit Logs", desc: "Search the append-only audit trail", icon: ScrollText, path: "/audit-logs" },
];

export function Dashboard() {
  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900">Dashboard</h1>
      <p className="mt-1 text-gray-500">
        Aspectus Admin Console {import.meta.env.PACKAGE_VERSION || "dev"}
      </p>
      <div className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
        {CARDS.map((card) => {
          const Icon = card.icon;
          return (
            <Link
              key={card.path}
              to={card.path}
              className="flex flex-col gap-3 rounded-xl border border-border bg-white p-6 transition-shadow hover:shadow-md"
            >
              <Icon size={28} className="text-primary" />
              <div>
                <h3 className="font-semibold text-gray-900">{card.label}</h3>
                <p className="text-sm text-gray-400">{card.desc}</p>
              </div>
            </Link>
          );
        })}
      </div>
    </div>
  );
}
