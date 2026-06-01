const BASE = "http://localhost:3100";
const TOKEN = "aspectus-dev-pandaria-service-token";

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${TOKEN}`,
      ...options.headers,
    },
  });
  if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
  return res.json();
}

export const api = {
  // Tenants
  createTenant: (name: string) =>
    request<any>("/tenants", { method: "POST", body: JSON.stringify({ name }) }),
  getTenant: (id: string) => request<any>(`/tenants/${id}`),

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
  createApiKey: (data: any) =>
    request<any>("/api-keys", { method: "POST", body: JSON.stringify(data) }),
  revokeApiKey: (id: string) =>
    request<void>(`/api-keys/${id}`, { method: "DELETE" }),

  // Roles
  listRoles: () => request<any[]>("/roles"),
  assignRole: (userId: string, roleId: string) =>
    request<any>(`/users/${userId}/roles`, { method: "POST", body: JSON.stringify({ role_id: roleId }) }),
  removeRole: (userId: string, roleId: string) =>
    request<void>(`/users/${userId}/roles/${roleId}`, { method: "DELETE" }),
};
