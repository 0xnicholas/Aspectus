-- System tenant for global administrative audit events.
--
-- Operations that are not scoped to a real tenant (e.g. service token
-- creation/rotation/revocation) are logged under this reserved tenant so
-- that the audit_logs.tenant_id foreign key remains valid.

INSERT INTO tenants (id, name)
VALUES ('system', 'Aspectus System')
ON CONFLICT (id) DO NOTHING;

COMMENT ON TABLE tenants IS
    'Top-level namespaces. The reserved tenant id ''system'' is used for global administrative audit events and cannot be created through the management API.';
