# Aspectus — Project Status

> 日期：2026-06-21
> 版本：v0.9.0

## Overview

Aspectus is the unified identity and multi-tenant management service for the Pandaria ecosystem. It provides a single source of truth for `tenant_id`, user authentication, API key management, token introspection, and tenant quota configuration.

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + axum + tokio |
| Database | PostgreSQL (14 tables, 12 migrations) |
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
| Login / Logout (simplified) | ✅ | v0.9 |
| User Registration (public) | ✅ | v0.9 |
| Password Reset Flow | ✅ | v0.9 |
| Authentication Audit Logging | ✅ | v0.9 |
| JWKS Real Public Key | ✅ | v0.9 |
| JWT identity_type Claim | ✅ | v0.9 |
| Local JWT Verification (client) | ✅ | v0.9 |
| Two-Step Login Flow (ADR-016) | ✅ | v0.9 |
| Cross-Tenant Login Routing | ✅ | v0.9 |
| Enriched Login Response (user/tenant/projects) | ✅ | v0.9 |
| JWT tenant_name Claim | ✅ | v0.9 |
| Tenant logo_url Support | ✅ | v0.9 |
| Default Role on Registration | ✅ | v0.9 |
| Admin Console (React SPA) | ✅ | v0.3-console |
| Rust Client Library | ✅ | v0.8 |
| Docker Support | ✅ | v0.8 |

## API Endpoints (25)

| # | Method | Path | Auth |
|---|--------|------|:----:|
| 1 | GET | `/health` | — |
| 2 | GET | `/metrics` | — |
| 3 | POST | `/introspect` | Service Token |
| 4 | POST | `/login` | — |
| 5 | POST | `/register` | — |
| 6 | POST | `/logout` | — |
| 7 | POST | `/forgot-password` | — |
| 8 | POST | `/reset-password` | — |
| 9 | POST | `/tenants` | Service Token |
| 10 | GET | `/tenants/{id}` | Service Token |
| 11 | PUT | `/tenants/{id}/quotas` | Service Token |
| 12 | POST | `/service-accounts` | Service Token |
| 13 | GET | `/service-accounts` | Service Token |
| 14 | GET | `/service-accounts/{id}` | Service Token |
| 15 | POST | `/users` | Service Token |
| 16 | GET | `/users` | Service Token |
| 17 | GET | `/users/{id}` | Service Token |
| 18 | PUT | `/users/{id}/suspend` | Service Token |
| 19 | POST | `/api-keys` | Service Token |
| 20 | GET | `/api-keys` | Service Token |
| 21 | GET | `/api-keys/{id}` | Service Token |
| 22 | DELETE | `/api-keys/{id}` | Service Token |
| 23 | GET | `/roles` | Service Token |
| 24 | POST | `/users/{id}/roles` | Service Token |
| 25 | DELETE | `/users/{id}/roles/{role_id}` | Service Token |
| 26 | POST | `/authorize` | — |
| 27 | POST | `/oauth/token` | — |
| 28 | POST | `/clients` | Service Token |
| 29 | GET | `/clients` | Service Token |
| 30 | GET | `/.well-known/jwks.json` | — |
| 31 | POST | `/login/lookup` | — |

## Test Coverage

| Suite | Tests | Status |
|-------|:-----:|:------:|
| aspectus-auth (unit) | 21 | ✅ |
| aspectus-client (unit) | 4 | ✅ |
| aspectus-core (unit) | 19 | ✅ |
| aspectus-server (unit) | 29 | ✅ |
| integration_test | 6 | ✅ |
| feature_test (User/Role/OAuth2) | 6 | ✅ |
| benchmark | 1 | ✅ |
| **Total** | **86** | ✅ |

## Database

```
14 Tables: tenants, service_accounts, users, api_keys, roles,
           roles_scopes, users_roles, scopes, audit_logs,
           service_tokens, authorization_codes, refresh_tokens,
           oauth2_clients, password_reset_tokens

6 Enums:   identity_type, project, role_type,
           password_encryption_method

11 Migrations: 001 through 011 (fully reproducible)
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
| [docs/adr/](adr/) | 16 Architecture Decision Records |
| [docs/specs/](specs/) | 7 Technical specifications |
| [docs/comparison-with-logto.md](comparison-with-logto.md) | Logto comparison |
| [docs/console-comparison.md](console-comparison.md) | Console comparison |
| [docs/consumer-integration.md](consumer-integration.md) | **生态项目接入指南** — 面向 Pandaria / Tavern / Constell / Tokencamp / Heirloom 等消费者项目的完整接入参考（中间件代码、错误处理矩阵、灰度与回滚） |

## Design Principles

1. **Single identity source** — Aspectus is the only service that issues and verifies identity
2. **Auth vs AuthZ separation** — Aspectus = door access; Heirloom = room access
3. **Multi-tenant by design** — tenant_id on every table, cross-tenant inexpressible
4. **Introspect-first** — /introspect is the hot path (p95 < 5ms)
5. **Ecosystem-first, not general-purpose** — no SAML, LDAP, Social Login

## Next Steps

| Priority | Item |
|:--------:|------|
| ✅ | Pandaria api-gateway 接入参考（consumer-integration.md 完成 2026-06-21） |
| ⬜ | Pandaria api-gateway 生产灰度与完全切换 |
| P1 | Tavern / Constell / Tokencamp integration |
| P2 | Heirloom data-level authorization |
| P2 | Emerald entity_id migration |
| P3 | v1.0 API stability freeze |
| P3 | Production load testing |
| ✅ | ADR-016 two-step login UX (completed 2026-06-20) |
