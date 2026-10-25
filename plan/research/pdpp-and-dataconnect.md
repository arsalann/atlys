# PDPP and DataConnect: closest prior art

Reviewed 2026-10-23. This is the project most similar to Atlys's protocol ambitions, so it gets its own note.

## What it is

- **PDPP (Personal Data Portability Protocol)**: an open protocol "for user-controlled, purpose-bound access to personal data." Developed at **PDP-Connect, a lab at LF Decentralized Trust (Linux Foundation)**. It began as Vana's Data Portability Protocol and moved to the Linux Foundation in 2026. Principles v1.0 were published Sept 2026. The core spec is a *normative draft*.
  - Spec repo: [PDP-Connect/pdpp](https://github.com/PDP-Connect/pdpp). Code is Apache-2.0; spec text is under the Community Specification License 1.0. Site: [pdpp.dev](https://pdpp.dev).
- **DataConnect**: the reference desktop app. Tauri (Rust shell) + TypeScript front end + a bundled local "Personal Server" + a Playwright runner. Local-first, with local PDPP and MCP endpoints. [PDP-Connect/data-connect](https://github.com/PDP-Connect/data-connect), Apache-2.0.
- **Data connectors**: ~60 TypeScript connectors, including Amazon, Apple Health, ChatGPT, GitHub, Gmail, Google Takeout, Google Maps (Data Portability API), iMessage, LinkedIn, Netflix, Oura, Reddit, Signal, Slack, Spotify, Strava, Uber, WhatsApp, and Whoop. Each has a manifest, JSON schemas, scrubbed fixtures, and tests. Releases are signed OCI artifacts. [PDP-Connect/data-connectors](https://github.com/PDP-Connect/data-connectors), Apache-2.0.

## Key design points

- **Grants:** a client (app or AI agent) requests a purpose-bound, field-, stream-, and time-scoped slice using OAuth Rich Authorization Requests (RFC 9396 `authorization_details`). The personal server enforces the grant on every query. Grants are revocable and recorded.
- **Sources:** `provider_native` (a platform speaks PDPP itself) or `connector` (a "polyfill" connector bridges a platform that doesn't).
- **Collection Profile:** a language-neutral connector protocol over JSONL on stdin/stdout (`START → RECORD / STATE / INTERACTION → DONE`). Connectors declare **bindings** they need: `browser_automation` (a CDP WebSocket the runtime provides), `browser_profile`, `filesystem`, `network`, `interactive`, `loopback_listen`.
- **Browser decision:** they explicitly rejected proxying browser commands through messages. The runtime gives the connector a real CDP endpoint instead.
- **Mobile:** connectors can ship a mobile "pageshim" (for example, Spotify), the same TypeScript logic injected into a mobile WebView.
- **MCP:** the reference implementation exposes an MCP surface for agent hosts, gated by grants.
- **Encryption at rest:** the protocol is deliberately **not opinionated**. Personal-server implementations decide.

## Overlap with Atlys

| Atlys plan | PDPP equivalent | Overlap |
| --- | --- | --- |
| Connector protocol (JSONL, manifest, fixtures) | Collection Profile | **Very high.** Nearly identical design. |
| Access layer (grants, scopes, audit, MCP) | PDPP core grants + MCP surface | **High** |
| Atlys Data Model (normalized cross-source schema) | Per-connector stream schemas; no strong cross-source normalization | **Partial.** PDPP streams are mostly source-specific. |
| Zero-knowledge encrypted vault (local / BYO / managed) | Not specified | **Gap.** This is the Atlys differentiator. |
| Mobile on-device collectors (HealthKit, Health Connect, location, screen time) | Limited (apple_health connector, mobile pageshims) | **Gap.** Legacy Atlys strength. |
| Delete-at-source, DSAR tracking, privacy audit | Not in scope | **Gap** |

## Strategic options

1. **Build independently.** Duplicates the connector and grant work and competes with a Linux Foundation-backed spec. Not recommended.
2. **Adopt PDPP as Atlys's access and connector protocol, and differentiate on the vault, mobile, normalization, and privacy tools.** Atlys becomes a PDPP-compatible personal server with a zero-knowledge encrypted store and native mobile apps. Its connectors are PDPP Collection Profile connectors, contributed upstream where general.
3. **Wait and see.** Design Atlys's interfaces to be PDPP-compatible, but don't commit until the spec stabilizes.

**Leaning: option 2, starting as option 3.** Track the spec, keep connector and grant interfaces compatible, and contribute the vault encryption and normalization work upstream as proposals. Being a strong PDPP implementation serves the "become the standard" goal better than launching a rival standard.

## Things to verify before committing

- Spec stability and governance pace (normative draft today, frequent breaking changes in connector repos).
- Whether the reference implementation can sit on top of an encrypted store without deep changes.
- Mobile story maturity.
- How much of the ecosystem is still tied to Vana's commercial or token interests versus neutral LF governance.
