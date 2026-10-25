# 08 — Roadmap (draft)

Milestones are scoped by outcome, not date. Each phase should be usable on its own.

## Phase 0: Foundations *(current)*

- [x] Plan folder, research, README, AGENTS.md
- [x] Decide name (keep Atlys), license (Apache-2.0), repo visibility (public)
- [x] Core language: Rust
- [x] Local database: SQLite + SQLCipher
- [x] Managed cloud stack: Rust + PostgreSQL + S3-compatible ciphertext storage (built later) (see [research/managed-cloud-stack.md](research/managed-cloud-stack.md))
- [ ] Confirm Rust core + TypeScript connector split and SQLCipher storage with a spike (see [research/language-choice.md](research/language-choice.md))
- [ ] Decide relationship to PDPP (see [research/pdpp-and-dataconnect.md](research/pdpp-and-dataconnect.md))
- [ ] Community files: CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, GOVERNANCE, issue/PR templates
- [ ] CI skeleton: lint, secret scanning (gitleaks), CodeQL, Dependabot
- [ ] RFC-0001: Atlys Data Model envelope and first namespaces (`location`, `health`, `device`)
- [ ] RFC-0002: Vault format and key hierarchy
- [ ] RFC-0003: Connector protocol and manifest

## Phase 1: v0, iOS app with local vault and on-device data

Detailed plan: [v0/](v0/README.md).

- [ ] M0 spike: Rust core on iOS, SQLCipher raw key, UniFFI, HPKE sealed inbox, Argon2id benchmark
- [ ] M1 vault + onboarding: master password, recovery phrase, Face ID, lock/unlock, change password, recovery
- [ ] M2 collectors: HealthKit, location visits, motion activity, device telemetry; background collection through the sealed inbox
- [ ] M3 app UI: Today, Timeline, Sources, Explorer, Settings; encrypted backup and JSON export
- [ ] M4 hardening: tests, battery profiling, crypto RFC review, TestFlight beta

## Phase 2: Agent access + more sources

- [ ] `cli` and desktop app (Tauri) on the same Rust core: open an exported or synced vault
- [ ] `mcp`: local stdio MCP server, read-only, with grant scopes and an audit log
- [ ] Export importers (TypeScript connectors): Apple Health `export.zip`, Google Takeout, Spotify extended history
- [ ] iOS v0.1 sources: calendar, contacts, photo metadata, and screen time for EU users (iOS 26.4+ app-and-website-usage entitlement)
- [ ] Mac connector: Screen Time stores (Mac + synced iPhone usage), with Full Disk Access
- [ ] Android app: Health Connect, location, usage stats (screen time), calendar
- [ ] Device-to-device sync (local network / shared folder)
- [ ] Demo: "Ask your agent about your own year"

## Phase 3: Storage options + sync

- [ ] BYO backends: S3-compatible, WebDAV, Postgres (all ciphertext only)
- [ ] Multi-device E2EE sync
- [ ] Desktop/web app (Tauri)
- [ ] Independent security audit

## Phase 4: Ecosystem

- [ ] Connector SDKs (TS, Python) + template + registry + certification levels
- [ ] Remote MCP with OAuth grants; Access API for third-party apps
- [ ] Data requests and deletion: DSAR letter generator, tracking, "delete at source"
- [ ] Privacy audit tools: breach check, account inventory, app permissions
- [ ] Managed vault service (zero-knowledge) beta

## Phase 5: Protocol / standard

- [ ] Spec v1.0 (ADM + Access protocol) with a conformance suite
- [ ] Third-party adopters; neutral governance for the spec
- [ ] P2P selective sharing between vaults; consented data donation
