# Aspectus — Project Status

> 日期：2026-06-01
> 版本：v0.8.0

## Overview

Aspectus is the unified identity and multi-tenant management service for the Pandaria ecosystem. It provides a single source of truth for `tenant_id`, user authentication, API key management, token introspection, and tenant quota configuration.

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + axum + tokio |
| Database | PostgreSQL (13 tables, 10 migrations) |
| Cache | Redis (introspection, revocation, Service Token) |
| Auth | argon2id (passwords), RS256 (JWT), SHA256 (API Keys) |
| Frontend | React 19 + TypeScript + Vite |
| Docs | OpenAPI 3.0 (spec-first) |
| Deploy | Docker (multi-stage), docker-compose |

## Feature Matrix

| Feature | Status | Version |
|---------|:------:|:-------:|
| Token Introspection (RFC 7662) | ✅ | v0.2 |
| Multi-tenant Isolation | ✅ | v0.2 |
| API Key Management | ✅ | v0.2 |
| Service Token Authentication | ✅ | v0.2 |
| Audit Logging | ✅ | v0.2 |
| Multi-project Scope Definitions | ✅ | v0.3 |
| Tenant Quota Configuration | ✅ | v0.3 |
| Redis Caching | ✅ | v0.2 |
| JWT (RS256) Signing & Verification | ✅ | v0.4 |
| Opaque Token Support | ✅ | v0.4 |
| User Management (CRUD) | ✅ | v0.5 |
| Role-Based Access Control | ✅ | v0.5 |
| Scope Expansion from Roles | ✅ | v0.5 |
| OAuth2 Authorization Code Flow | ✅ | v0.6 |
| Refresh Token Rotation | ✅ | v0.7 |
| OAuth2 Client Registration | ✅ | v0.7 |
| Prometheus Metrics | ✅ | v0.8 |
| OpenAPI Documentation | ✅ | v0.8 |
| Admin Console (React SPA) | ✅ | v0.3-console |
| Rust Client Library | ✅ | v0.8 |
| Docker Support | ✅ | v0.8 |

## API Endpoints (18)

| # | Method | Path | Auth |
|---|--------|------|:----:|
| 1 | GET | `/health` | — |
| 2 | GET | `/metrics` | — |
| 3 | POST | `/introspect` | Service Token |
| 4 | POST | `/tenants` | Service Token |
| 5 | GET | `/tenants/{id}` | Service Token |
| 6 | PUT | `/tenants/{id}/quotas` | Service Token |
| 7 | POST | `/service-accounts` | Service Token |
| 8 | GET | `/service-accounts` | Service Token |
| 9 | GET | `/service-accounts/{id}` | Service Token |
| 10 | POST | `/users` | Service Token |
| 11 | GET | `/users` | Service Token |
| 12 | GET | `/users/{id}` | Service Token |
| 13 | PUT | `/users/{id}/suspend` | Service Token |
| 14 | POST | `/api-keys` | Service Token |
| 15 | GET | `/api-keys` | Service Token |
| 16 | GET | `/api-keys/{id}` | Service Token |
| 17 | DELETE | `/api-keys/{id}` | Service Token |
| 18 | GET | `/roles` | Service Token |
| 19 | POST | `/users/{id}/roles` | Service Token |
| 20 | DELETE | `/users/{id}/roles/{role_id}` | Service Token |
| 21 | POST | `/authorize` | — |
| 22 | POST | `/oauth/token` | — |
| 23 | POST | `/clients` | Service Token |
| 24 | GET | `/clients` | Service Token |
| 25 | GET | `/.well-known/jwks.json` | — |

## Test Coverage

| Suite | Tests | Status |
|-------|:-----:|:------:|
| aspectus-auth (unit) | 9 | ✅ |
| aspectus-client (unit) | 1 | ✅ |
| integration_test | 6 | ✅ |
| feature_test (User/Role/OAuth2) | 6 | ✅ |
| benchmark | 1 | ✅ |
| **Total** | **23** | ✅ |

## Database

```
13 Tables: tenants, service_accounts, users, api_keys, roles,
           roles_scopes, users_roles, scopes, audit_logs,
           service_tokens, authorization_codes, refresh_tokens,
           oauth2_clients

6 Enums:   identity_type, project, role_type,
           password_encryption_method

10 Migrations: 001 through 010 (fully reproducible)
```

## Console (Admin UI)

```
8 Pages:    Dashboard, Tenants, Service Accounts, Users,
            API Keys, Roles, OAuth2 Clients, Audit Logs

6 Components: Button, Input, Table, Badge, Modal, Toast

Stack:      React 19 + TypeScript + Vite + React Router
Build:      248KB JS (78KB gzipped)
```

## Documentation

| Document | Description |
|----------|------------|
| [README.md](../README.md) | Project overview + quick start |
| [ROADMAP.md](../ROADMAP.md) | Version history + future |
| [AGENTS.md](../AGENTS.md) | Architecture principles + ADR |
| [docs/openapi.yaml](openapi.yaml) | API specification (OpenAPI 3.0) |
| [docs/adr/](adr/) | 15 Architecture Decision Records |
| [docs/specs/](specs/) | 7 Technical specifications |
| [docs/comparison-with-logto.md](comparison-with-logto.md) | Logto comparison |
| [docs/console-comparison.md](console-comparison.md) | Console comparison |

## Design Principles

1. **Single identity source** — Aspectus is the only service that issues and verifies identity
2. **Auth vs AuthZ separation** — Aspectus = door access; Heirloom = room access
3. **Multi-tenant by design** — tenant_id on every table, cross-tenant inexpressible
4. **Introspect-first** — /introspect is the hot path (p95 < 5ms)
5. **Ecosystem-first, not general-purpose** — no SAML, LDAP, Social Login

## Next Steps

| Priority | Item |
|:--------:|------|
| P1 | Pandaria api-gateway integration |
| P1 | Tavern / Constell / Tokencamp integration |
| P2 | Heirloom data-level authorization |
| P2 | Emerald entity_id migration |
| P3 | v1.0 API stability freeze |
| P3 | Production load testing |
