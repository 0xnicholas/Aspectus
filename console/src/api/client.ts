/// SECURITY WARNING: Production deployment
///
/// This admin console uses VITE_SERVICE_TOKEN baked into the client bundle at
/// build time, which exposes the service token to anyone with browser devtools
/// access. This is acceptable ONLY when:
/// 1. The console is served on an internal network (not public internet), OR
/// 2. The console is behind an authenticating reverse proxy that injects the
///    Authorization header (e.g., oauth2-proxy, Tailscale Serve, Cloudflare Access).
///
/// For public-facing production deployments, replace this with a BFF (Backend
/// For Frontend) that proxies API calls and adds the service token server-side,
/// or use session-based auth with httpOnly cookies.

const BASE = (import.meta.env.VITE_API_BASE || "http://localhost:3100").replace(/\/+$/, "");
const TOKEN = import.meta.env.VITE_SERVICE_TOKEN || "";

/// Base URL for the Aspectus API. Must be HTTPS in production.
export const API_BASE = BASE;

function qs(params: Record<string, string | number | undefined>) {
  const parts: string[] = [];
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== "") parts.push(`${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`);
  }
  return parts.length ? `?${parts.join("&")}` : "";
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  // In production, prefer the reverse proxy setting the Authorization header.
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string> || {}),
  };
  if (TOKEN) {
    headers["Authorization"] = `Bearer ${TOKEN}`;
  }

  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers,
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`${res.status}: ${body}`);
  }
  // 204 No Content responses have no body
  if (res.status === 204) return undefined as T;
  return res.json();
}

export const api = {
  // Tenants
  createTenant: (name: string) =>
    request<any>("/tenants", { method: "POST", body: JSON.stringify({ name }) }),
  listTenants: () => request<any[]>("/tenants"),
  getTenant: (id: string) => request<any>(`/tenants/${id}`),
  updateTenantQuotas: (id: string, quotas: Record<string, any>) =>
    request<void>(`/tenants/${id}/quotas`, { method: "PUT", body: JSON.stringify(quotas) }),

  // Users
  listUsers: (tenant_id: string) =>
    request<any[]>(`/users?tenant_id=${tenant_id}`),
  createUser: (data: any) =>
    request<any>("/users", { method: "POST", body: JSON.stringify(data) }),
  suspendUser: (id: string, suspended: boolean) =>
    request<any>(`/users/${id}/suspend`, { method: "PUT", body: JSON.stringify({ suspended }) }),

  // API Keys
  listApiKeys: (service_account_id: string) =>
    request<any[]>(`/api-keys?service_account_id=${service_account_id}`),
  createApiKey: (data: { owner_type: string; owner_id: string; project: string; scopes: string[]; expires_at?: string }) =>
    request<any>("/api-keys", { method: "POST", body: JSON.stringify(data) }),
  revokeApiKey: (id: string) =>
    request<void>(`/api-keys/${id}`, { method: "DELETE" }),

  // Clients
  listClients: () => request<any[]>("/clients"),
  createClient: (data: any) =>
    request<any>("/clients", { method: "POST", body: JSON.stringify(data) }),

  // Service Accounts
  listServiceAccounts: (tenant_id: string) =>
    request<any[]>(`/service-accounts?tenant_id=${tenant_id}`),
  getServiceAccount: (id: string) => request<any>(`/service-accounts/${id}`),
  createServiceAccount: (data: any) =>
    request<any>("/service-accounts", { method: "POST", body: JSON.stringify(data) }),

  // Service Tokens
  listServiceTokens: () => request<any[]>("/service-tokens"),
  createServiceToken: (project: string) =>
    request<any>("/service-tokens", { method: "POST", body: JSON.stringify({ project }) }),
  rotateServiceToken: (project: string) =>
    request<any>(`/service-tokens/${project}/rotate`, { method: "POST" }),
  revokeServiceToken: (project: string) =>
    request<void>(`/service-tokens/${project}`, { method: "DELETE" }),

  // Audit Logs
  listAuditLogs: (filter: {
    tenant_id?: string;
    action?: string;
    target_type?: string;
    target_id?: string;
    actor_id?: string;
    from?: string;
    to?: string;
    limit?: number;
    offset?: number;
  }) => request<any[]>(`/audit-logs${qs(filter)}`),

  // Roles
  listRoles: () => request<any[]>("/roles"),
  assignRole: (userId: string, roleId: string) =>
    request<any>(`/users/${userId}/roles`, { method: "POST", body: JSON.stringify({ role_id: roleId }) }),
  removeRole: (userId: string, roleId: string) =>
    request<void>(`/users/${userId}/roles/${roleId}`, { method: "DELETE" }),
};
