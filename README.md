# Agentive Inversion

The hosted Google connector for [Agentive Workspace](https://github.com/meawoppl/agentive-workspace).
Google login and credentials stay here. Agents, project knowledge, tasks, and
human review live in a Git repository operated through Agent Portal.

## Responsibilities

- Google OAuth, connected-account status, and revocable Portal connections.
- Independent Gmail ingestion and history backfill.
- Account-scoped source reads and live calendar reads.
- Expiring work claims so overlapping Portal cron runs do not process the same mail.
- Approval-gated archive, receipt-forwarding, and calendar effects with durable receipts.
- A read-only export of the former application's project records.

There is no hosted agent runtime or project writer. Portal cron runs agents in
the workspace repository; those agents commit knowledge and propose provider
actions. You review proposals through Portal. Google tokens never leave this
service. PostgreSQL remains the operational store for credentials, mail, leases,
and effect receipts; Git is the authority for project knowledge.

![Architecture](docs/diagrams/project-service-architecture.svg)

## Develop

Requires stable Rust, Trunk, the `wasm32-unknown-unknown` target, PostgreSQL, and
native libpq libraries. Configure `.env` from `.env.example`, including a Google
OAuth web client, allowed login emails, and a generated JWT secret.

```sh
(cd crates/frontend && NO_COLOR=true trunk build)
cargo run --bin backend -- --dev-mode
```

Open http://localhost:3000. Dev mode disables ingestion and the effect worker;
OAuth and the database are still required. Omit the flag for background work.
The server applies embedded migrations before accepting requests. Build the
frontend before compiling the backend, and rebuild the backend after asset changes.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
./scripts/check-migration-names.sh
cargo audit
# Destructive fixtures: use a dedicated disposable database only.
TEST_DATABASE_URL=postgres://localhost/agentive_connector_test \
  cargo test -p backend --locked connector_database_contract -- --ignored
```

CI runs both the normal tests and the database contract test against an isolated
Postgres service. The contract covers account scopes, separate review authority,
competing claims, lease fencing, request replay, revocation, interrupted work,
and the retirement of legacy writes.

## Read more

- [API contract](docs/CONNECTOR_API.md): pairing, grants, claims, proposals, receipts.
- [Architecture](ARCHITECTURE.md): authority and implementation boundaries.
- [Project outline](docs/PROJECT_SERVICE_PLAN.md): end-to-end workflow and migration.
- [Local setup](QUICKSTART.md) and [container setup](DOCKER.md).

Merges to main build `ghcr.io/meawoppl/agentive-inversion:main`; production
Watchtower automatically replaces the container. The image contains the backend,
embedded frontend, and migrations. It contains no Claude CLI or other agent runtime.

[MIT licensed](LICENSE).
