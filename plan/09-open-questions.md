# 09 — Open questions

Decisions to make, each with a current leaning. Resolve these through discussion or RFCs, then record the outcome here.

| # | Question | Options | Current leaning |
| --- | --- | --- | --- |
| 1 | **License** | Apache-2.0 everywhere vs. AGPL for apps/server | ✅ **Decided (2026-10-23): Apache-2.0** for the whole project. Spec text may later use a spec-specific license (for example, the Community Specification License, as PDPP does). |
| 2 | **Languages** | Rust · Go · TypeScript · mix | ✅ **Decided (2026-10-23): Rust** for the core. Recommended split: Rust core (vault, CLI, MCP, desktop) + TypeScript connectors (APIs, parsers, browser automation) + language-neutral JSONL connector protocol, pending a spike. See [research/language-choice.md](research/language-choice.md). |
| 3 | **Mobile approach** | Native Swift/Kotlin · Flutter · React Native | **Recommended: native.** v0 is SwiftUI (iOS 18+) over the Rust core through UniFFI. See [v0/03-ios-app.md](v0/03-ios-app.md). |
| 4 | **Local database** | SQLCipher · SQLite3MultipleCiphers · app-level encryption · Turso · DuckDB · key-value stores | ✅ **Decided (2026-10-25): SQLite + SQLCipher** (raw-key mode) for the on-device store; encrypted blob files; record-level encrypted change log for sync and sharing. Watch Turso Database. See [research/local-database.md](research/local-database.md). |
| 5 | **Sync model** | CRDT (Automerge/Evolu-style) · encrypted append-only log · object-store diffing | Leaning encrypted append-only log with device IDs and hybrid logical clocks (records are mostly immutable facts). This is the same format the managed service stores. |
| 6 | **Build vs. adopt connectors** | Write our own · wrap HPI / Timelinize / Open Wearables parsers where licenses allow | Reuse where license-compatible (HPI is MIT; Timelinize is AGPL, so it can be used as a reference only if we stay Apache) |
| 7 | **Data model base** | Invent ADM · adopt Solid/RDF · adopt Open mHealth/FHIR wholesale | Pragmatic JSON-Schema ADM that *maps to* those standards; avoid making RDF a hard requirement |
| 8 | **Embeddings / semantic search** | None · local embeddings · pluggable | Pluggable, local by default; off unless enabled |
| 9 | **Name** | Keep "Atlys" · rename | ✅ **Decided (2026-10-23): keep "Atlys".** Still worth a trademark check in the relevant classes before launch. This project comes from Atlys Networks (2019). A separate, unrelated company, Atlys (a visa-application startup founded in 2021), operates under the same name. Don't confuse the two in docs or research. Decide before public launch. |
| 10 | **Spec home** | This repo · separate `atlys-spec` repo · foundation | This repo (`spec/`) for now; split out when there are external adopters |
| 11 | **Managed service operator** | Project-run · partners · none at first | None until after the audit; design so anyone can host (`docker compose up` runs the full stack) |
| 12 | **Write access for agents** | Read-only forever · scoped writes (notes, tags) | Read-only in v1; consider scoped writes to `note`/`custom` later |
| 13 | **Relationship to Solid / AT Protocol** | Ignore · interoperate · build on | Interoperate through export/bridge later; don't build on top of them |
| 14 | **Relationship to PDPP** (Personal Data Portability Protocol, LF Decentralized Trust) | Ignore · stay compatible · adopt as the access/connector protocol | **Leaning: stay compatible now, likely adopt.** Differentiate on the encrypted vault, mobile collectors, normalization, and privacy tools. See [research/pdpp-and-dataconnect.md](research/pdpp-and-dataconnect.md). |
| 15 | **Repo visibility** | Private until launch · public | ✅ **Decided (2026-10-23): public from day one**, not promoted until Phase 1 is usable. |
| 16 | **Managed cloud stack** | Hosted DB with records · zero-knowledge ciphertext store | ✅ **Decided (2026-10-25): Rust (axum) API + PostgreSQL metadata (SQLite for self-host) + S3-compatible ciphertext storage + OPAQUE login + passkeys (PRF) + E2EE agent relay to the user's own device.** Not built in v1; v1 must use the same encrypted log format. See [research/managed-cloud-stack.md](research/managed-cloud-stack.md). |
