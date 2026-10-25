# Technology options

Building blocks to evaluate. Feeds into [../03-architecture.md](../03-architecture.md) and [../09-open-questions.md](../09-open-questions.md).

## Vault encryption reference: Bitwarden

- Keys are generated and managed on the client. The server never holds the master password or keys.
- KDF: PBKDF2-SHA256 (600k iterations, historically salted with email) or **Argon2id**.
- Data: AES-256-CBC + HMAC. The master password is hashed client-side before authentication, and the server hashes it again.
- **Takeaway:** a proven zero-knowledge pattern for the managed service. Modernize primitives with Argon2id by default and an AEAD (XChaCha20-Poly1305 / AES-GCM) instead of CBC+HMAC.

## Encrypted local storage

| Option | Pros | Cons |
| --- | --- | --- |
| **SQLCipher** | Transparent full-DB encryption; mature; mobile support | Whole-DB key, so it is harder to share subsets or sync record-level |
| **App-level record encryption on SQLite** | Per-record/collection keys; enables selective sharing and E2EE sync; host sees only ciphertext | Can't index or query encrypted fields directly, so a local decrypted index is needed in memory or in an encrypted side DB |
| **Hybrid** | SQLCipher for the local working DB plus record-level encrypted change log for sync and sharing | More moving parts |
| **DuckDB** (side-car) | Fast analytics for insights | Not the primary store; encryption support is newer |

**Decision:** the hybrid approach, SQLCipher locally plus a record-level encrypted change log. See [local-database.md](local-database.md).

## Sync engines (local-first, E2EE)

| Option | Notes |
| --- | --- |
| **[Evolu](https://github.com/evoluhq/evolu)** (MIT) | SQLite + CRDT + E2EE sync; self-hostable relay; TS-focused |
| **[Any-Sync](https://github.com/anyproto/any-sync)** (MIT) | Anytype's CRDT protocol; E2EE; nodes store ciphertext; Go |
| **Automerge / Yjs** | General CRDT libraries; would need an encryption layer on top |
| **ElectricSQL / PowerSync** | Postgres ↔ SQLite sync; no E2EE by default, so they don't fit the zero-knowledge requirement |
| **Custom encrypted append-only log** | Most personal data is immutable facts (a step count doesn't get edited), so a simple signed, encrypted log with last-writer-wins for the few mutable records may be enough |

## Raw blob storage

- Content-addressed (SHA-256) and encrypted, in the style of Perkeep. Backends: local FS, S3-compatible, WebDAV.
- Chunking for large exports (multi-GB Takeout ZIPs). Encrypt each chunk with a key derived from the vault key.

## Cross-platform core

| Option | Notes |
| --- | --- |
| **Rust + UniFFI** | Generates Swift/Kotlin bindings; WASM for web; used by Mozilla, Bitwarden SDK, and 1Password core |
| Go + gomobile | Simpler; weaker iOS story; bigger binaries |
| TypeScript everywhere | Fast to build; weaker for crypto and background mobile work |

## Mobile data APIs

| Platform | APIs | Notes |
| --- | --- | --- |
| iOS | HealthKit, CoreLocation (significant-change, visits), EventKit, Contacts, Photos, DeviceActivity (limited) | Background execution limits; HealthKit data has strict usage rules |
| Android | **Health Connect** (Google Fit APIs deprecated), Fused Location, UsageStatsManager, CalendarProvider | More open for usage and screen time; background location needs strong justification for Play review |

## Agent interface

- **MCP official SDKs** (TypeScript, Python, Rust, Kotlin, Swift…). Local stdio for desktop agents; streamable HTTP + OAuth for remote.
- Define a small, stable tool surface (`search`, `query`, `timeline`, `summarize`, `sources`) instead of one tool per record type.
- Optional local embedding models (for example, small sentence-transformer models through ONNX) for semantic search without sending data out.

## Supply chain and repo hygiene

- gitleaks / GitHub push protection; CodeQL; Dependabot or Renovate; OpenSSF Scorecard; Sigstore/cosign signing; SLSA provenance on releases.
