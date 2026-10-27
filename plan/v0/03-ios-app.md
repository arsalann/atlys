# v0: iOS app architecture

## Overview

```
┌──────────────────────────── iPhone app (Swift, SwiftUI) ────────────────────────────┐
│                                                                                      │
│  UI: Onboarding · Lock · Today · Timeline · Sources · Explorer · Settings            │
│        │                                                                             │
│        ▼                                                                             │
│  VaultSession (unlocked only) ──── query, merge inbox, settings, export             │
│        │                                                                             │
│  Collectors: Health · Location · Motion · Device ──▶ IngestSealer ──▶ inbox/*.sealed │
│        (run in foreground and background; never hold the vault key)                  │
│                                                                                      │
├────────────────────────── UniFFI bindings (generated Swift) ─────────────────────────┤
│                                                                                      │
│  Rust core (XCFramework)                                                             │
│   atlys-crypto   Argon2id, HKDF, XChaCha20-Poly1305, HPKE, zeroize                   │
│   atlys-vault    header + envelopes, unlock, SQLCipher store, inbox merge, export    │
│   atlys-adm      record envelope + v0 types, validation                              │
│   atlys-ffi      UniFFI surface: Vault, VaultSession, IngestSealer                   │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

## The write-only ingest inbox

**The problem:** collectors run in the background, often while the vault is locked and sometimes while the phone itself is locked. If they needed the vault key to write, the vault would have to stay unlocked (weak security), or collection would stop whenever it's locked (poor data).

**The solution:** asymmetric "sealed box" writes.

1. When the vault is created, the core generates an **X25519 ingest keypair**. The **public** key goes in the vault header in plaintext. The **private** key is stored only as an envelope in the header, wrapped by a subkey of the vault key. Formats: [RFC-0002](../../spec/rfcs/0002-vault-format.md).
2. Collectors serialize a batch of ADM records ([RFC-0001](../../spec/rfcs/0001-data-model.md)) and call `IngestSealer.seal(batch)`. The core encrypts it with **HPKE** to the public key and writes `inbox/<ulid>.sealed`. The collector can *add* data but can't *read* anything, including its own previous batches.
3. On unlock, `VaultSession.mergeInbox()` decrypts each file with the private key, validates and dedupes the records, inserts them into SQLCipher in one transaction, then deletes the file.
4. Inbox files use the file protection class *Complete Until First User Authentication*, so writes succeed while the phone is locked (after the first unlock since boot). `vault.db` uses *Complete* and is never touched by background code.

**What a thief with the file system learns from the inbox:** the number and sizes of pending batches, nothing else. Batches are padded to fixed size buckets.

**Collector state** (HealthKit anchors, last query times) is kept in a small separate file, also *Complete Until First User Authentication*. It reveals only *when* collection happened, not what was collected. Later it can move inside the sealed scheme.

This pattern carries over to Android, desktop, and connectors: anything that produces data only needs the public key.

## Rust ↔ Swift boundary (UniFFI)

```text
// Implemented in M1 (core/crates/atlys-ffi/src/vault.rs) as free functions
// (createVault, unlockWithPassword, unlockWithDevice, recoverVault, ...)
// plus the VaultSession object. Merge, query, and export are M2/M3.
object Vault {
  create(dir, password) -> (VaultSession, RecoveryPhrase)
  unlockWithPassword(dir, password) -> VaultSession
  unlockWithDevice(dir, deviceEnvelope, sharedSecret) -> VaultSession  // ECDH output from the Secure Enclave
  recover(dir, phrase, newPassword) -> VaultSession  // recovery always sets a new password
  passwordStrength(password) -> Strength
}
object VaultSession {               // exists only while unlocked
  mergeInbox() -> MergeReport
  query(filter) -> [Record]         // by type, time range, source; paginated
  dailySummary(date) -> Summary
  changePassword(current, new)
  regenerateRecovery() -> RecoveryPhrase
  exportEncrypted(dest) / exportJson(dest)
  deviceEnvelope(sharedSecret, ephemeralPub, enclavePub) -> Bytes  // Rust wraps; Swift stores it in the Keychain
  delete()                          // only from a fresh unlock
  lock()
}
object IngestSealer {               // only needs the public key; safe in background
  open(dir) -> IngestSealer         // reads vault ID and ingest key from the unverified header
  seal(collector, records: [RecordDraft]) -> fileName   // RFC-0001 batch, validated before sealing
  sealWithDeletions(collector, records, deletions: [RecordDeletion]) -> fileName
}
```

- Swift owns iOS concerns: HealthKit, Core Location, Core Motion, Keychain/Secure Enclave, background tasks, and UI.
- Rust owns everything about data and keys: crypto, storage, validation, merging, queries, and export.
- Raw key bytes never cross into Swift. For Face ID, Swift asks the Secure Enclave for a P-256 ECDH shared secret and passes it to Rust, which derives the key-encryption key and wraps or unwraps the vault key. Driving the Secure Enclave from Rust was considered in M0 and rejected: it needs a third-party Security.framework binding and prompt handling, and the split already keeps the vault key out of Swift. See [spike-report.md](spike-report.md#07-secure-enclave-wrap-of-the-vault-key).

## Screens

| Screen | Purpose |
| --- | --- |
| Onboarding | See [01-account-setup.md](01-account-setup.md) |
| Lock | Face ID / password |
| **Today** | Cards: steps and activity, sleep last night, places visited, heart summary, collection status. The "this is my data" moment. |
| **Timeline** | Day-by-day list mixing visits, workouts, sleep, and activity segments. Coordinates and locally clustered "Place 1, Place 2" names, since there's no map or geocoding in v0. |
| **Sources** | Per collector: permission state, last collected, records total, backfill progress, pending inbox count, "Collect now". In v0.2 this is a section of the home screen. |
| **Explorer** | Browse by record type, filter by date, and open any record to see its JSON and provenance. v0.2 ships it as "Browse your data" (types → days → record JSON), read-only with no copy or share. |
| **Settings** | Auto-lock, Face ID, change password, regenerate recovery phrase, encrypted export, JSON export, storage used, delete vault, about/licenses (including the SQLCipher attribution), privacy statement |

## Project layout

```
core/                         Rust workspace
├── Cargo.toml
├── crates/
│   ├── atlys-crypto/
│   ├── atlys-vault/
│   ├── atlys-adm/
│   └── atlys-ffi/            UniFFI; builds the XCFramework
apps/ios/
├── Atlys.xcodeproj
├── Atlys/                    app target (SwiftUI)
├── Packages/
│   ├── AtlysCore/            generated Swift bindings + binary XCFramework target
│   ├── AtlysCollectors/      Health, Location, Motion, Device
│   └── AtlysUI/              screens and components
├── Config/
│   ├── Shared.xcconfig       committed
│   └── Local.xcconfig        gitignored: signing team, bundle ID overrides
└── AtlysTests/, AtlysUITests/
spec/adm/v0/                  JSON Schemas for the v0 record types
scripts/build-ios-core.sh     cargo build (device + simulator) → uniffi-bindgen → xcframework
```

## Engineering choices

| Area | Choice |
| --- | --- |
| Swift | Swift 6 language mode (strict concurrency), SwiftUI, Observation |
| Xcode project | Generated by XcodeGen from `apps/ios/project.yml`; the generated project is committed and CI checks it is up to date |
| Minimum iOS | 18.0 |
| Rust targets | `aarch64-apple-ios`, `aarch64-apple-ios-sim` |
| SQLCipher build | `rusqlite` with bundled SQLCipher, **Apple CommonCrypto** as the crypto provider (decided in M0). No OpenSSL ships in the app; `cargo deny` bans `openssl-sys`. |
| Dependencies | **No third-party Swift packages.** Rust crates vetted with `cargo deny` (licenses, advisories) and pinned in `Cargo.lock`. |
| Networking | None. CI fails if `URLSession`, `Network` connections, or WebKit appear outside an allowlist. `NWPathMonitor` is read-only and allowed. |
| Logging | `os.Logger` with privacy annotations; no record contents ever logged; no analytics or crash SDKs |
| Localization | English first, with strings in String Catalogs from day one |
| Accessibility | Dynamic Type, VoiceOver labels, and contrast checks as part of M4 |

## Testing

| Layer | Approach |
| --- | --- |
| Rust crypto | Unit tests, known-answer tests for HPKE/XChaCha/Argon2id, and property tests (envelope round trips, tamper rejection) |
| Rust vault | Integration tests: create, unlock, merge, query, change password, recover, export/import, corrupted files |
| FFI | Swift tests calling the real XCFramework on the simulator |
| Collectors | Protocol-based fakes for HealthKit, Core Location, and Core Motion; simulator HealthKit seeded with synthetic samples; GPX files for simulated locations |
| UI | XCUITest for onboarding, lock/unlock, and settings flows |
| Real device | Battery profiling (Xcode Energy Organizer / Instruments) over multi-day runs; background-delivery checks with the phone locked |

**All test data is synthetic. No real personal data is ever committed** (see AGENTS.md).

## CI (GitHub Actions)

1. `core`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`
2. `ios`: build the XCFramework, then `xcodebuild test` on an iOS Simulator
3. `hygiene`: gitleaks secret scan, the no-network check, link check for docs

CI needs no signing secrets, since simulator builds are unsigned. TestFlight uploads come in M4, using GitHub encrypted secrets, never files in the repo.

## App Store and privacy

- App Privacy "nutrition label": **Data Not Collected**. Everything stays on the device and Atlys (the project) receives nothing.
- Purpose strings for Health, Location (When In Use and Always), and Motion, written in plain language.
- A privacy policy page is required for HealthKit. A short, plain one goes in the docs site and the app.
- Review notes explaining visit monitoring and why "Always" location is requested.
