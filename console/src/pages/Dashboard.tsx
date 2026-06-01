import { Link } from "react-router-dom";
import { Building2, Users, Key } from "lucide-react";

export function Dashboard() {
  return (
    <div>
      <h1 className="text-2xl font-bold text-gray-900">Dashboard</h1>
      <p className="mt-1 text-gray-500">Aspectus Admin Console v0.8.0</p>
      <div className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-3">
        {[
          { label: "Tenants", desc: "Manage tenants", icon: Building2, path: "/tenants" },
          { label: "Users", desc: "Manage users", icon: Users, path: "/users" },
          { label: "API Keys", desc: "Create and revoke keys", icon: Key, path: "/api-keys" },
        ].map((card) => {
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
