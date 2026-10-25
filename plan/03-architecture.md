# 03 — Architecture (draft)

## Layers

```
┌───────────────────────────────────────────────────────────────────────┐
│  Consumers:  Reference apps (iOS · Android · Desktop/Web · CLI)        │
│              User's AI agent (Claude, local LLM, …) · third-party apps │
└───────────────▲───────────────────────────────▲───────────────────────┘
                │ UI / SDK                       │ MCP / Atlys Access API
┌───────────────┴───────────────────────────────┴───────────────────────┐
│ 4. ACCESS LAYER   grants · scopes · consent prompts · audit log ·      │
│                   query API · MCP server · redaction/aggregation       │
├───────────────────────────────────────────────────────────────────────┤
│ 3. NORMALIZATION  parsers · Atlys Data Model (ADM) · dedupe · entity   │
│                   resolution · enrichment (geocode, categorize) ·      │
│                   optional local embeddings / search index             │
├───────────────────────────────────────────────────────────────────────┤
│ 2. VAULT          encrypted record store · raw blob store · key mgmt · │
│                   storage backends (local / BYO / managed) · sync      │
├───────────────────────────────────────────────────────────────────────┤
│ 1. INGESTION      connectors: on-device sensors · APIs (OAuth) ·       │
│                   export/takeout importers · DSAR requests · manual    │
└───────────────────────────────────────────────────────────────────────┘
```

### 1. Ingestion (connectors)

A **connector** is a self-contained module that knows how to get data from one source and emit records. Ingestion modes:

| Mode | Example | Notes |
| --- | --- | --- |
| **On-device** | iOS HealthKit, Android Health Connect, location, screen time, app usage, calendar, contacts | Runs inside the mobile app and writes straight to the local vault |
| **API (pull)** | Google Data Portability API, Spotify, GitHub, Strava, Oura, Fitbit | OAuth tokens stored in the vault; incremental sync |
| **Export import** | Google Takeout, Meta "Download Your Information", Apple privacy export, Amazon, Netflix, X/Twitter archive | User drops a ZIP; connector parses it |
| **Data request (DSAR)** | GDPR Art. 15 / CCPA "right to know" requests to services without APIs | Atlys drafts and tracks the request; the result is imported later |
| **Manual / custom** | Counters, journals, CSV import | Same as the legacy "Thing Counter" |

**Connector contract (proposal):** language-agnostic and process-based, similar in spirit to Singer/Airbyte. A connector:

- declares a **manifest**: id, version, auth method, the ADM record types it emits, the permissions it needs, and sync mode (full/incremental)
- emits a stream of messages (`RECORD`, `RAW_BLOB`, `STATE`, `LOG`) as JSON Lines on stdout, or through an in-process SDK on mobile
- never talks to the network except to its declared source hosts. The host runtime should enforce this where possible.
- ships with **synthetic fixtures** and golden-file tests. Real personal data is never used in tests.

First-class SDK: **TypeScript**. Python and other languages work through the same JSONL protocol. Browser-based connectors receive a CDP endpoint from the host and never handle the user's password: the user logs in themselves in a visible browser or WebView. The design closely matches the PDPP Collection Profile, and compatibility with it is a goal (see [research/pdpp-and-dataconnect.md](research/pdpp-and-dataconnect.md)). Mobile on-device collectors are native (Swift/Kotlin) because the OS APIs require it.

### 2. Vault

- **Record store:** normalized ADM records, encrypted at rest.
- **Blob store:** original export files and attachments (photos, PDFs), content-addressed and encrypted. "Raw is sacred": every normalized record links back to its raw source.
- **Key management:** see [06-security-privacy.md](06-security-privacy.md).
- **Storage backends** (pluggable through one interface):
  1. **Local (default):** a single encrypted file or folder on the device, password-protected.
  2. **Bring-your-own (BYO):** the user's S3-compatible bucket, WebDAV, Postgres, or a synced folder (iCloud Drive, Dropbox, Syncthing). The data stays client-side encrypted, so the backend only sees ciphertext.
  3. **Managed (optional, paid):** a hosted backend run by the project or a third party. It is zero-knowledge in the same way as Bitwarden's hosted vault, and it remains self-hostable with the same server code. The server stores only ciphertext and can't answer queries, so agent queries in managed mode are relayed, end-to-end encrypted, to one of the user's own devices (see [research/managed-cloud-stack.md](research/managed-cloud-stack.md)).
- **Sync:** multi-device sync of encrypted change sets. Options to evaluate include Evolu-style E2EE CRDT sync, Any-Sync, or a simple append-only encrypted log (see [research/technology-options.md](research/technology-options.md)).

### 3. Normalization

- **Parsers** turn raw source data into **Atlys Data Model** records (see [05-data-model.md](05-data-model.md)).
- **Pipelines:** deduplication across sources (for example, steps from both phone and watch), entity resolution (the same person across contacts, email, and messages), timezone normalization, and enrichment (reverse geocoding with an offline/local option, merchant categorization).
- **Derived data:** daily summaries, stays/visits from raw GPS, and sessions from app usage. These are recomputable and flagged as derived.
- **Search index:** full-text search, plus optional local vector embeddings for semantic search by agents. The index is encrypted like everything else.

### 4. Access layer

The part that makes Atlys a **platform**.

- **Grants:** a user grants a client (agent or app) access to specific **scopes** (record types, time ranges, fields) for a duration. Grants can be revoked at any time.
- **MCP server:** the first-class interface for AI agents. It exposes tools like `search`, `query_records`, `get_timeline`, `summarize_period`, and `list_sources`, all filtered by the active grant. It runs locally (stdio) or as a remote endpoint with OAuth, following the MCP authorization spec.
- **Atlys Access API:** an HTTP/JSON (and SDK) equivalent for non-agent apps.
- **Privacy transforms:** return aggregates instead of rows, coarsen location, redact fields, and apply differential-privacy noise for aggregate queries (later).
- **Audit log:** an append-only record of who accessed what and when, visible to the user.
- **Consent UX:** high-sensitivity scopes (health, messages, precise location) require explicit, per-grant confirmation.

## Technology choices

✅ = decided (see [09-open-questions.md](09-open-questions.md)). Others are current recommendations.

| Concern | Choice | Why | Alternatives |
| --- | --- | --- | --- |
| Vault core ✅ | **Rust** library with bindings (UniFFI for Swift/Kotlin, WASM for web, N-API/PyO3 for desktop/servers) | One audited crypto/storage core on every platform; memory safety | Go (gomobile), TypeScript-only |
| Local DB ✅ | **SQLite + SQLCipher** (raw-key mode) via `rusqlite`; FTS5 + JSON1; blobs as encrypted files | Encrypts everything at rest, proven, every native platform, full SQL + search | Turso Database (watch), SQLite3MultipleCiphers, DuckDB side-car for analytics. See [research/local-database.md](research/local-database.md) |
| Crypto | libsodium / RustCrypto primitives: **Argon2id** KDF, **XChaCha20-Poly1305** AEAD | Modern, misuse-resistant, widely reviewed | AES-256-GCM |
| Connectors | **TypeScript SDK** (primary) on a language-neutral JSONL protocol; browser automation through a host-provided CDP endpoint (Playwright on desktop, injected WebView scripts on mobile) | Best browser-automation ecosystem, runs in mobile WebViews, largest contributor pool, PDPP-compatible | Rust-only (weak browser tooling); Python as secondary |
| Mobile apps | Native Swift / Kotlin shells over the Rust core | Needed for HealthKit, Health Connect, and background collection | React Native / Flutter + native modules |
| Desktop/Web | Tauri (Rust core plus web UI) | Shares the core; small footprint | Electron |
| Agent interface | **MCP** (official Rust SDK `rmcp`) | De facto standard, now under Linux Foundation governance | Custom REST only |
| Server (sync/managed) ✅ | **Rust (axum) + PostgreSQL (metadata) + S3-compatible object storage (ciphertext)**; OPAQUE login; SQLite + local disk for self-host | Zero knowledge; shares core types; provider-neutral. See [research/managed-cloud-stack.md](research/managed-cloud-stack.md) | Go |

Background: [research/language-choice.md](research/language-choice.md), [research/local-database.md](research/local-database.md), [research/managed-cloud-stack.md](research/managed-cloud-stack.md).

## Monorepo layout (proposed)

```
atlys/
├── spec/                # Atlys Data Model schemas + Access protocol spec (versioned)
├── core/                # Rust: vault, crypto, storage backends, query engine
├── connectors/          # one folder per connector (+ shared fixtures)
│   ├── _template/
│   ├── apple-health/
│   └── google-takeout/
├── sdk/
│   ├── typescript/      # connector SDK + client SDK
│   └── python/
├── mcp/                 # MCP server
├── apps/
│   ├── ios/
│   ├── android/
│   ├── desktop/         # Tauri (also serves web build)
│   └── cli/
├── server/              # sync + managed-vault server (ciphertext only)
├── docs/                # user + developer docs site
└── plan/                # this folder
```
