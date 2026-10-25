# v0: M0 spike report

Status: **Simulator and CI phase complete (2026-10-25). Device phase pending** (no Apple Developer account or test iPhone yet).

This report covers tasks 0.3–0.8 from [04-work-breakdown.md](04-work-breakdown.md). Tasks 0.1 and 0.2 (scaffolding, community files, CI) are done and need no decisions beyond those recorded in [Other decisions](#other-decisions-made-during-m0).

Code: `core/` (Rust workspace) and `apps/ios/` (Xcode project, packages, and the throwaway spike app in `apps/ios/Spike/`).

## Summary

| Task | Simulator / CI | Device | Decision |
| --- | --- | --- | --- |
| 0.3 SQLCipher | ✅ Raw key, wrong-key rejection, no plaintext on disk; CommonCrypto provider on iOS | ⏳ Pending | **Use Apple CommonCrypto.** No OpenSSL in the app. |
| 0.4 UniFFI → XCFramework | ✅ Swift calls Rust and gets typed records, objects, and errors; Swift 6 strict concurrency clean | ⏳ Pending (device build links) | Keep UniFFI proc-macros; no UDL |
| 0.5 Sealed inbox | ✅ Swift → Rust round trip; tampered and foreign files rejected; **CryptoKit-sealed files open in Rust** | Not needed beyond 0.8 | Keep HPKE in Rust; format provisional until RFC-0002 |
| 0.6 Argon2id | ✅ Harness on host and in the app; host table below | ⏳ **Pending, blocks the final choice** | **Proposed m = 256 MiB, t = 3, p = 1**, fixed (not per-device), confirmed on the oldest iPhone |
| 0.7 Secure Enclave | ✅ Wrap/unwrap through the Secure Enclave without biometry; tamper and wrong-vault rejection | ⏳ Face ID gating and invalidation need a device | **Swift does the ECDH; Rust does KDF + AEAD.** The vault key never enters Swift. |
| 0.8 Background | ✅ Significant location changes wake and relaunch the app and write sealed files | ⏳ Visits, HealthKit, and the locked phone need a device and a team | No change to the plan; two implementation rules below |

## 0.3 SQLCipher via `rusqlite`

**What we tried.** `rusqlite` 0.41 with `bundled-sqlcipher` (via `libsqlite3-sys` 0.39.1), opened in raw-key mode (`PRAGMA key = "x'<64 hex>'"`). The amalgamation now ships as a separate crate, `sqlcipher-src` (SQLCipher 4.19.0, SQLite 3.53.4). It also bundles libtomcrypt for targets without a system crypto library.

**What worked.**

- With no `OPENSSL_*` variables set, `libsqlite3-sys` compiles SQLCipher with `-DSQLCIPHER_CRYPTO_CC` on Apple host/target pairs and links `Security.framework`. No configuration is needed. `scripts/build-ios-core.sh` unsets those variables so a developer's Homebrew OpenSSL can never be picked up, and `cargo deny` bans `openssl-sys` and `openssl-src`.
- `PRAGMA cipher_provider` reports `commoncrypto` on macOS and on the iOS Simulator. The linked binary imports `CCCryptor*`, `CCHmac*`, `CCKeyDerivationPBKDF`, and `SecRandomCopyBytes`, and has no OpenSSL symbols.
- Tests (`atlys-vault` `store` tests, `AtlysSpikeTests`):
  - data written with the key reads back after close and reopen;
  - a different key fails with `WrongKeyOrCorrupt`;
  - a plain `SQLite format 3` file is rejected;
  - the file has no plaintext header, no schema text, and no known plaintext marker, and neither does the WAL.

**Measurements.**

| Where | Full self-test (create, write, close, reopen, wrong key, scan) |
| --- | --- |
| macOS host (Apple M4), release | 3–12 ms |
| iOS Simulator, first call in the process | **~1.3 s** |
| iOS Simulator, later calls | 2–3 ms |

The first-call cost is one-time per process. Its cause is not yet isolated (likely first use of CommonCrypto/Security in the process). **Measure it on a device.** If it is real there, warm the core (open a throwaway connection) while the lock screen is shown, before the user authenticates.

**Decision: CommonCrypto, not vendored OpenSSL.** It builds cleanly with zero configuration, adds no binary size, and uses the OS crypto library (backed by Apple's corecrypto), which Apple maintains and patches. Nothing to vendor or patch.

**Notes and risks.**

- `PRAGMA cipher_provider_version` returns `unknown` for CommonCrypto. That is cosmetic.
- On a wrong key, SQLCipher logs `hmac check failed for pgno=1` through its own logger. The message contains no secrets, but M1 should set `PRAGMA cipher_log_level = NONE` (or route it to `os.Logger` at debug level) to keep logs quiet.
- `cipher_memory_security` is off by default in SQLCipher 4.5+. Turning it on wipes freed memory at a performance cost. Decide in RFC-0002, together with the zeroization story.
- **Android/desktop (later):** Linux CI uses the system OpenSSL provider. For Android, the choice is now vendored OpenSSL or the bundled **libtomcrypt** provider, which avoids OpenSSL entirely. Evaluate when Android starts.
- Background tasks that hold a SQLite lock in an **App Group** container during suspension get killed (`0xdead10cc`). v0 keeps `vault.db` in the app's own container and never opens it in the background, so this does not apply. Revisit before adding extensions (widgets, share extension).

## 0.4 UniFFI → XCFramework → local Swift package

**What we did.**

- `uniffi` 0.32.2 with proc-macros, and a pinned `uniffi-bindgen` binary inside the workspace (`core/tools/uniffi-bindgen`).
- `scripts/build-ios-core.sh`: `cargo build` for `aarch64-apple-ios` and `aarch64-apple-ios-sim` → `uniffi-bindgen generate --library` → `xcodebuild -create-xcframework` → `apps/ios/Packages/AtlysCore/AtlysCoreFFI.xcframework` plus the generated `AtlysCore.swift`.
- The `uniffi.toml` names the modules `AtlysCore` (Swift) and `AtlysCoreFFI` (C).

**What worked.**

- Records, objects, `Option`, `Vec<u8>` (`Data` in Swift), and typed errors all bridge.
- The generated Swift compiles in **Swift 6 language mode with complete strict concurrency and warnings as errors**, unmodified. Generated objects are `@unchecked Sendable`, so they can be used from background collectors.
- Tests run on the Simulator in both a plain package test bundle and app-hosted bundles. The unsigned device build (`generic/platform=iOS`) links.

**Notes.**

- In uniffi 0.32, `--config` means a *global* config file. The crate's own `uniffi.toml` is picked up automatically in library mode, so the script does not pass `--config`.
- A clean build takes ~50 s for both targets with LTO on an M4. Incremental builds take a few seconds.
- Building only `aarch64-apple-ios-sim` means Intel Macs can't run the Simulator build. That matches the plan.

**Decision.** Keep this pipeline. The XCFramework and generated Swift are build outputs (gitignored), and CI builds them on every PR.

## 0.5 HPKE sealed inbox round trip

**What we did.**

- `atlys-crypto::hpke` wraps the `hpke` crate (0.14) with a single fixed suite: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305, base mode.
- `atlys-vault::inbox` defines a provisional file format:

  ```
  "ATLYSINB" | version | suite | HPKE enc (32) | ciphertext + tag
  ```

  The 10-byte header is the AAD, and `info` is `atlys/v1/inbox`. The plaintext is length-prefixed and padded to buckets (1, 4, 16, 64, 256 KiB, 1 MiB, then whole MiB). Files are written atomically: temporary file, fsync, rename. Names are ULIDs, so files sort by write time.

**What worked (tests).**

- RFC 9180 Appendix A.2.1 known-answer test.
- Swift seals through `IngestSealer` (public key only), and Rust opens with the private key.
- Tampered files, files sealed to another vault, and malformed records are rejected. Rejected files are left in place.
- Property tests sample ciphertext bit flips to confirm tamper rejection; deterministic inbox tests also cover header and encapsulated-key positions.
- **An inbox file built entirely with Apple CryptoKit's HPKE (`Curve25519_SHA256_ChachaPoly`) opens in Rust**, and flipping the suite byte in the authenticated header is rejected. Two independent implementations agree on the format, so a native-only writer remains possible later if ever needed.

**Decision.** Keep sealing in Rust, as planned (one implementation everywhere). The interop test stays as a guard.

**For RFC-0002.**

- Bind the vault ID into HPKE `info` so a file can't be replayed into another vault with the same key.
- Define how rejected files are quarantined.
- Note that ULID names reveal write times. The plan already accepts that collector state reveals when collection happened. Use random names if that is not acceptable.

## 0.6 Argon2id calibration

**What we did.**

- A host benchmark (`cargo run --release -p atlys-crypto --example argon2_bench`) and the same grid in the spike app (Argon2 tab, Markdown output), both calling `derive_key_unchecked`.
- The KDF itself is checked against the **reference C implementation** (`argon2` CLI) with a known-answer vector.

**Host measurement** (Apple M4, single thread, p = 1, median of 5, ms). The Simulator runs on the host CPU, so it measures the same thing (~99 ms at 64 MiB/t=3 under test-runner load).

| m \ t | t=1 | t=2 | t=3 | t=4 | t=6 |
|---|---|---|---|---|---|
| 19 MiB | 9 | 13 | 19 | 26 | 38 |
| 46 MiB | 17 | 34 | 63 | 71 | 105 |
| 64 MiB | 22 | 45 | 68 | 91 | 137 |
| 128 MiB | 47 | 95 | 145 | 192 | 290 |
| 192 MiB | 72 | 146 | 222 | 303 | 447 |
| 256 MiB | 98 | 198 | 300 | 403 | 603 |
| 384 MiB | 150 | 303 | 464 | 611 | 923 |

As a cross-check, the reference C CLI takes ~104 ms at 64 MiB/t=3 on the same machine.

**Finding.** The plan's starting point (64 MiB, t = 3) costs only ~70 ms on current Apple silicon, an order of magnitude below the ~1 s target. Even a phone several times slower than an M4 core would stay well under 1 s. Memory cost is the parameter that hurts GPU and ASIC attackers most, so raise memory before iterations.

**Proposed parameters (to confirm on devices):**

- **m = 256 MiB, t = 3, p = 1** as the v0 default. This is ~0.3 s on an M4. The oldest iOS 18 iPhones (A12) are expected to land near the 1 s target, but **that is an estimate, not a measurement**.
- Use **fixed parameters, not per-device calibration at vault creation.** A vault created on a fast phone must still unlock in reasonable time after a restore onto an older phone. Parameters stay in the header and can be raised later.
- Adjustment rule after the device run: if the oldest supported iPhone takes more than 1.5 s, use m = 192 MiB. If it takes less than 0.7 s, use t = 4.
- Keep `Argon2Params::OWASP_MINIMUM` (19 MiB, t = 2) as the floor accepted from a header. Keep the existing caps (1 GiB, t ≤ 64, p ≤ 16), so a tampered header can't hang or exhaust memory.
- p = 1. The `argon2` crate's `parallel` feature (rayon) could use two performance cores, but it adds a dependency and helps attackers equally. Not worth it for v0.

**Risk.** A 256 MiB allocation in the foreground is well within iOS limits on 3 GB+ devices, but it must be verified on the oldest device. The Argon2 tab can include 384 MiB to probe headroom.

## 0.7 Secure Enclave wrap of the vault key

**Design tried.** The Secure Enclave only does P-256 operations on keys it holds, and it never exports private keys. The plan had Rust hand the raw vault key to Swift for ECIES wrapping. The spike instead splits the work:

1. **Swift (CryptoKit)** creates a `SecureEnclave.P256.KeyAgreement.PrivateKey` with access control `[.privateKeyUsage, .biometryCurrentSet]` and `WhenUnlockedThisDeviceOnly`.
   - To **wrap**, Swift generates an ephemeral P-256 key and computes ECDH with the enclave's public key.
   - To **unwrap**, the enclave computes ECDH with the stored ephemeral public key, which triggers Face ID.
   - In both cases Swift holds only the 32-byte shared secret.
2. **Rust** derives a KEK with HKDF-SHA256 (salt = both public keys, label `atlys/v1/envelope/secure-enclave`) and wraps or unwraps the vault key with XChaCha20-Poly1305. The envelope is 203 bytes:

   ```
   version | ephemeral pub | enclave pub | wrapped key
   ```

   Its header is the AAD (`atlys-vault::envelope`).

**What worked on the Simulator.**

- `SecureEnclave.isAvailable` is `true` on the Simulator, and wrap/unwrap through it works when biometry is not required.
- A tampered envelope fails authentication.
- Another vault's envelope does not unlock this vault (the key comparison is constant-time and done in Rust).

**What the Simulator cannot do.** Creating a key with `.biometryCurrentSet` fails with `LAError -1020` ("not supported on iOS Simulator"). Face ID gating and invalidation therefore need a device.

**Decision: drive the Secure Enclave from Swift, keep all key material handling in Rust.**

- With this split, **the vault key never crosses the FFI boundary**, so the "one tightly scoped call" exception in the plan goes away. Swift sees a shared secret that is only useful together with the envelope.
- Driving the enclave from Rust would need a third-party binding crate (`security-framework`) plus `LocalAuthentication` prompt handling, which is UI work. The split already removes the exposure that motivated the idea.
- The FFI API changes accordingly (see the [03-ios-app.md](03-ios-app.md#rust--swift-boundary-uniffi) sketch, updated in this PR).

**Open items.**

- Where the envelope and the enclave key's `dataRepresentation` live. The spike uses a Keychain item. The product could put the envelope in the vault header and only the key blob in the Keychain. Decide in RFC-0002.
- Confirm on a device that the Keychain item (`ThisDeviceOnly`) is absent after an iOS backup restore. The app then shows "re-enable Face ID".

## 0.8 Background feasibility

**What we built.** The spike app's Background tab:

- Visit monitoring and significant location changes, holding a `CLServiceSession(authorization: .always)`.
- A HealthKit `HKObserverQuery` with `enableBackgroundDelivery(.immediate)` for step count, followed by an anchored query from a saved anchor.

Each wake seals records with an `IngestSealer` built from `ingest.pub` alone, and appends a line to a wake log. The log records time, trigger, app state, protected-data availability, and outcome, never coordinates or health values. Inbox and log files use *Complete Until First User Authentication*.

**What worked on the Simulator** (Always location granted with `simctl privacy`, drive simulated with `simctl location start`):

- With the app in the background, significant-change events woke it about every 30 s. Each wake sealed a `location.point` batch.
- After `simctl terminate`, Core Location **relaunched the app in the background** within ~30 s, and it kept sealing.
- All 9 background-written files opened in Rust with 0 rejected.

**What the Simulator could not show.**

- **Visits.** The Simulator does not synthesize `CLVisit`s.
- **HealthKit.** Without a development team, Xcode 27 embeds only `application-identifier` in Simulator builds, even with ad hoc signing. HealthKit then fails with "Missing com.apple.developer.healthkit entitlement". Once a team is set in `Local.xcconfig`, the Simulator path (synthetic samples → observer) can be retested.
- **Locked-phone behavior and file protection.** The Simulator does not implement Data Protection: protection attributes read back as `nil`. The protection tests are strict only on a device.

**Implementation rules found (carry into M2):**

1. In a scene-based app, `launchOptions[.location]` was **nil** on a Core Location background relaunch. Don't branch on it. Start location services on every launch.
2. In `application(_:didFinishLaunchingWithOptions:)`, `applicationState` is `.background` even for user launches (the scene isn't active yet). Gate "foreground only" work on `isProtectedDataAvailable` or scene activation instead.
3. Register HealthKit observer queries during launch, before returning from `didFinishLaunching`, so background deliveries are handled after a relaunch.

**Decision.** No change to the plan. The write-only inbox works for background wakes as designed. The 7-day device run in M2 remains the real proof.

## Manual on-device test steps

Prerequisites:

- An Apple Developer team (an organization account if publishing under a project name).
- `apps/ios/Config/Local.xcconfig` with `DEVELOPMENT_TEAM` and `ATLYS_BUNDLE_ID_PREFIX`.
- The HealthKit capability (with background delivery) enabled for the spike's App ID.
- Two iPhones on iOS 18 or later: the oldest supported (A12 class, for example iPhone XS/XR) and a current one.

Use **Release** builds for timing:

```sh
scripts/build-ios-core.sh
```

Then run the `AtlysSpike` scheme on the device with Product → Scheme → Edit Scheme → Run → Build Configuration: Release.

Record results in a follow-up PR to this report, as summaries. Do not commit exported wake logs or screenshots of real locations or health data.

### 0.3 SQLCipher on device
1. Core tab → **Run SQLCipher self-test**. Expect `provider: commoncrypto`, and all four checks `true`.
2. Record the first-run and second-run elapsed times.
3. Run the hosted tests on the device: `xcodebuild test -scheme AtlysSpike -destination 'platform=iOS,name=<device>'`. On a device, `filesInheritProtection` checks the protection class strictly.

### 0.4 UniFFI on device
1. Confirm the Core tab shows `core 0.0.1 · ADM 0-draft`. The `Atlys` app shows the same.

### 0.6 Argon2id
1. Charge the device to at least 50 %, unplug it, let it cool, and close other apps.
2. Argon2 tab → **Run grid**. On the current iPhone, also enable 384 MiB.
3. **Copy as Markdown** and paste the result into this report. The output includes the model identifier, iOS version, thermal state, and Low Power Mode.
4. Repeat once to check variance. Apply the adjustment rule in 0.6.

### 0.7 Face ID
1. Enroll Face ID on the device. Face ID tab → **Enable Face ID unlock**.
2. **Unlock with Face ID** → expect `unlocked`.
3. Cancel the prompt → expect a failure, with no crash.
4. Settings → Face ID & Passcode → **Set Up an Alternate Appearance** (or reset Face ID and re-enroll). Return and tap **Unlock** → expect a failure. The key is invalidated by `biometryCurrentSet`.
5. Remove the envelope, re-enroll, and unlock again → expect `unlocked`.
6. Back up and restore the device (or migrate to the second phone), then open the Face ID tab → expect `Envelope: none`.

### 0.8 Background while locked
1. Background tab: **Request When In Use** → **Upgrade to Always** → **Request HealthKit**. Grant everything.
2. Open the Health app and confirm step data exists (carry the phone or pair a Watch).
3. Lock the phone and leave the app in the background. Don't force-quit it.
4. Over a day: walk or drive to at least two places and stay ~20 minutes at each. Keep the phone **locked** for long stretches.
5. Unlock, open the Background tab, and check the wake log for:
   - `visit` and `significant-change` events with `protected data: no` and `sealed …`. These are inbox writes while the phone was locked, the core claim.
   - `health-observer` events. While locked, expect `deferred: health data locked`. After unlock, expect `sealed …` (catch-up).
6. Core tab → **Open inbox (keep files)** → expect `rejected 0`.
7. Reboot the phone, but **don't unlock** it. Wait, then unlock. Expect no writes before the first unlock (that's the *Complete Until First User Authentication* class), and normal writes afterwards.
8. Record the counts and gaps per trigger, and Settings → Battery usage for the spike app, in this report.

## Other decisions made during M0

| Decision | Choice | Why |
| --- | --- | --- |
| Xcode project generation | **XcodeGen** (`apps/ios/project.yml`); the generated `Atlys.xcodeproj` is committed; CI fails on drift | Reviewable YAML, no merge conflicts in `project.pbxproj`; contributors can open the project without installing anything. XcodeGen is a build tool, not a Swift package. |
| Spike API isolation | `atlys-ffi` cargo feature `spike`; app code under `apps/ios/Spike/` | Keeps throwaway code (including unencrypted spike keys) out of the product API. Remove before M1 features. |
| Rust MSRV | 1.89 (toolchain pinned to 1.99.1) | `rusqlite` 0.41 needs 1.89. A lower MSRV made Cargo's resolver hold back a security fix (RUSTSEC-2026-0385). |
| Ignored advisory | RUSTSEC-2026-0374 (`basic-toml` unmaintained) | Reaches us only through uniffi's build-time templating; not linked into the app |
| Placeholder bundle ID | `org.example.atlys` + `Local.xcconfig` override | No identifier is chosen yet; `example` makes the placeholder obvious |
| Draft ADM shapes | Minimal `RecordDraft` / `Batch` in `atlys-adm` | Enough for the inbox; RFC-0001 replaces them in M2 |

## Follow-ups

- [ ] **Device phase** of 0.3, 0.6, 0.7, 0.8 (steps above). Final Argon2id parameters go into [01-account-setup.md](01-account-setup.md).
- [x] Repository settings: **private vulnerability reporting** enabled during merge review (2026-10-26).
- [ ] Replace the `security@<project-domain>` and `conduct@<project-domain>` placeholders.
- [ ] Test on an **iOS 18.x** Simulator runtime in CI. So far it has run on iOS 26.5 (CI) and iOS 27.0 (local) only, with an iOS 18.0 deployment target.
- [x] RFC-0002 inputs from this report: vault ID in HPKE `info`, the inbox quarantine rule, envelope storage location, `cipher_memory_security`, and SQLCipher log level. Captured in [RFC-0002](../../spec/rfcs/0002-vault-format.md) (accepted); it narrows the accepted Argon2id range below the spike caps.
- [ ] Before M1 features: delete `apps/ios/Spike/`, the `AtlysSpike` targets, and the `spike` feature.

## Recommended start for M1

1. **RFC-0002 (task 1.1)** first, using this report's inputs. The primitives it needs already exist and are tested in `atlys-crypto`, so the RFC can include real test vectors from day one.
2. Then **`atlys-vault` create/unlock (1.3)** on top of the existing `Store`, `envelope`, and `inbox` modules, replacing `SpikeVault`.
3. In parallel, once the Apple account exists, run the device phase. The Argon2id numbers and the Face ID invalidation result feed directly into 1.3 and 1.5.
