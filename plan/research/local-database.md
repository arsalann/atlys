# Local database options

Status: Recommendation (2026-10-23). Feeds open question #4 in [../09-open-questions.md](../09-open-questions.md).

## Requirements

The on-device database sits inside the Rust vault core and must:

1. **Encrypt everything at rest**, including indexes, metadata, temp files, and the write-ahead log, using a key from the Atlys key hierarchy (Argon2id → vault key), not a password handled by the database.
2. Run on **iOS, Android, macOS, Windows, and Linux** from one Rust codebase. Web/WASM is a bonus.
3. Handle **millions of records**: time-range queries ("my sleep in March"), filtering by type and source, JSON fields, and aggregates.
4. Support **full-text search** and, later, **vector search** for agents.
5. Be **mature and well understood**, because it holds people's most sensitive data.
6. Have a **license compatible with Apache-2.0**.
7. Not block **sync, BYO storage, and selective sharing**, which need record-level encryption once data leaves the device.

## Options

| Option | What it is | Encryption | Platforms | Maturity | Verdict |
| --- | --- | --- | --- | --- | --- |
| **SQLite + SQLCipher** (via `rusqlite` with `bundled-sqlcipher`) | SQLite with transparent page-level encryption | AES-256 per page with HMAC; covers DB, WAL, and temp files; accepts a raw 256-bit key | All native platforms (WASM possible but awkward) | Very high: used by Signal, many password managers, healthcare apps | ✅ **Recommended** |
| SQLite + SQLite3MultipleCiphers | Alternative SQLite encryption extension; default cipher ChaCha20-Poly1305; can read SQLCipher files | Several ciphers | All native platforms | High, but no ready-made Rust binding (custom build of `libsqlite3-sys` needed) | Good fallback if SQLCipher becomes a problem |
| SQLite + app-level record encryption only | Plain SQLite; each record's payload encrypted by Atlys code | Per-record AEAD | All, including WASM | We'd write it ourselves | ❌ Leaks metadata (types, timestamps, row counts) unless indexes are also encrypted, which breaks efficient queries. Use it for sync and sharing, not as the local store. |
| **libSQL** (Turso's fork of SQLite) | SQLite fork with extra features and embedded replicas | Encryption at rest available | Native platforms | Production-grade per Turso | Possible, but the fork adds dependency risk without a clear benefit over SQLCipher for us |
| **Turso Database** (Rust rewrite of SQLite) | Native Rust, async, MVCC, vector search | AEGIS-256 / AES-256-GCM, **experimental, unaudited** | Native + WASM | Beta (v0.6, May 2026); maintainers point to libSQL for production | 👀 **Watch.** Very attractive long term (pure Rust, WASM, modern ciphers). Revisit when encryption is stable and audited. |
| **DuckDB** | Embedded analytics (columnar) database | AES-256-GCM since v1.4 (Sept 2025), not yet NIST-compliant | Desktop strong; mobile support unclear | High for analytics | Optional **analytics side-car** on desktop later (it can read SQLite), not the primary store |
| Pure-Rust key-value stores (redb, fjall, heed/LMDB) | Embedded key-value storage | None built in | All | Medium–high | ❌ No SQL, no full-text search, and we'd have to build encryption, indexes, and queries ourselves |
| SurrealDB (embedded) | Multi-model database written in Rust | Not a built-in, audited at-rest scheme | Native + WASM | Medium | ❌ Heavy; licensing (BSL) is a poor fit for an Apache-2.0 project |
| Realm (legacy Atlys) | Mobile object database | AES-256 | iOS/Android; no Rust | Device Sync shut down Sept 2025; local DB only | ❌ No Rust support, uncertain future |
| Core Data / SwiftData / Room | Platform-native databases | OS file protection | One platform each | High | ❌ Not cross-platform; would mean two vault implementations |

## Recommendation

**SQLite + SQLCipher as the local store, plus record-level encryption for anything that leaves the device.**

```
┌──────────────────────────── Device ────────────────────────────┐
│                                                                 │
│  vault.db  (SQLite + SQLCipher, raw 256-bit key from vault key) │
│   ├─ records        ADM envelope; JSON data (JSON1)             │
│   ├─ records_fts    full-text index (FTS5)                      │
│   ├─ embeddings     optional, later (sqlite-vec or similar)     │
│   ├─ sources, sync_state, grants, audit_log                     │
│   └─ blob_index     pointers to encrypted blobs                 │
│                                                                 │
│  blobs/            raw exports, photos, attachments             │
│   └─ <sha256>       each file XChaCha20-Poly1305 encrypted       │
│                                                                 │
└──────────────┬──────────────────────────────────────────────────┘
               │ sync / backup / BYO storage / managed service
               ▼
   encrypted change log: each record or batch sealed with its own
   data key (wrapped by the vault key) → the host sees ciphertext only
```

**Why:**

- **Encrypts everything.** SQLCipher covers indexes, the WAL, and temp files. A stolen phone or laptop leaks nothing, not even which record types exist.
- **Fits our key hierarchy.** SQLCipher can take a **raw key**, so we skip its built-in password stretching and feed it a key derived from the Atlys vault key (Argon2id). Changing the master password re-wraps the vault key without re-encrypting the database.
- **Proven.** It is the most widely deployed encrypted embedded database: Signal and many password managers use it. Security reviewers already know it.
- **The full SQLite toolbox.** FTS5 for search, JSON1 for flexible records, window functions for time-series summaries, and a vector extension later. It's all inside the encrypted file.
- **Works with Rust today.** `rusqlite` has first-class `bundled-sqlcipher` features (compiled from source, with vendored crypto so builds are reproducible on every platform).
- **License.** BSD-style. The only requirement is attribution on an about/licenses screen.

**What it doesn't do:** sync or sharing. A single database file encrypted with one key can't be partially shared or merged across devices. That's why data leaving the device goes through a separate **record-level encrypted change log** (see [technology-options.md](technology-options.md)). The local database is a fast, private working copy, and the change log is the portable, shareable form.

## Design rules

1. **Storage behind a trait.** The vault core talks to a `Store` interface, not to SQLite directly, so a Turso Database or SQLite3MultipleCiphers backend can be swapped in later without touching connectors or the access layer.
2. **Raw key mode only.** Never pass the user's password to SQLCipher. Keys come from the Atlys key hierarchy and are wiped from memory after use.
3. **Blobs live outside the database.** Large raw exports (multi-GB Takeout ZIPs) go to encrypted, content-addressed files, which keeps the database small and fast to back up.
4. **Derived data is rebuildable.** FTS indexes, embeddings, and daily summaries can always be regenerated from records, so they can be dropped or migrated freely.
5. **Migrations are versioned and tested** with synthetic fixtures on every platform in CI.
6. **Web comes later.** The desktop app (Tauri) uses the native core. A browser-only vault, using SQLite WASM with OPFS storage plus app-level encryption, or Turso in WASM, is a later decision.

## To validate in the spike

> **M0 result (2026-10-25):** on iOS, `rusqlite` + `bundled-sqlcipher` builds through UniFFI with Apple **CommonCrypto** as the provider, so no OpenSSL is needed. Raw-key open, wrong-key rejection, and no plaintext on disk are verified on the Simulator. Android is not yet tried; there the options are vendored OpenSSL or the libtomcrypt provider bundled in `sqlcipher-src`. See [../v0/spike-report.md](../v0/spike-report.md#03-sqlcipher-via-rusqlite).

- `rusqlite` + `bundled-sqlcipher-vendored-openssl` builds for iOS and Android targets through UniFFI.
- Insert and query throughput with 5M synthetic records on a mid-range Android phone.
- FTS5 search latency inside an encrypted database.
- Key rotation: re-wrapping the vault key without touching the database.
