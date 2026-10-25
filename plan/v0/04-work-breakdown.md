# v0: work breakdown

Concrete tasks per milestone, each with exit criteria. Every milestone ends with a merged PR (or small series of PRs) and green CI. These tasks become GitHub issues when implementation starts.

## M0: Spike (1–2 weeks)

Goal: remove the technical unknowns before building features.

Status (2026-10-25): simulator and CI phase done; device phase pending an Apple Developer account. Results: [spike-report.md](spike-report.md).

| # | Task | Exit criteria |
| --- | --- | --- |
| 0.1 | Repo scaffolding: `core/` Rust workspace, `apps/ios/` Xcode project, `spec/adm/v0/`, `scripts/` | `cargo build` and an empty app build locally and in CI |
| 0.2 | Community files and CI skeleton: CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, PR template, gitleaks, `cargo deny` | Workflows run on PRs; secret scanning active |
| 0.3 | SQLCipher via `rusqlite` on `aarch64-apple-ios` and the simulator; choose CommonCrypto vs vendored OpenSSL | Encrypted DB created with a raw key on a real iPhone; wrong key fails; file is unreadable without the key |
| 0.4 | UniFFI bindings → XCFramework → local Swift package | Swift calls a Rust function and gets a typed result on device and simulator |
| 0.5 | HPKE sealed inbox round trip | Swift seals a batch through Rust; Rust opens it with the private key; tampered files are rejected |
| 0.6 | Argon2id calibration | Timing table on the oldest supported iPhone and a current one; final parameters chosen (target ≈ 1 s) |
| 0.7 | Secure Enclave wrap of the vault key | Face ID unlock works; key invalidated when biometrics change; decide whether the wrap runs in Swift or Rust |
| 0.8 | Background feasibility check | Visit monitoring and HealthKit background delivery wake a test app and write an inbox file while the phone is locked |

**Exit:** a short spike report in `plan/v0/spike-report.md` with the decisions above. Any change to the plan is made in the docs before M1.

## M1: Vault and onboarding

Status (2026-10-26): 1.1 done ([RFC-0002](../../spec/rfcs/0002-vault-format.md)
accepted, vectors committed). 1.2 done pending review (`atlys-crypto` matches
RFC-0002: Argon2id range, header MAC, envelope vectors). Current state and next steps:
[handoff.md](handoff.md).

| # | Task | Exit criteria |
| --- | --- | --- |
| 1.1 | **RFC-0002: vault format and key hierarchy** (header, envelopes, HKDF labels, inbox format, versioning) | RFC merged; test vectors committed |
| 1.2 | `atlys-crypto`: Argon2id, HKDF, XChaCha20-Poly1305, HPKE, zeroizing key types | Known-answer and property tests pass |
| 1.3 | `atlys-vault`: create, unlock (password / recovery / key), lock, change password, regenerate recovery, delete | Integration tests cover every flow, including corrupted and tampered files |
| 1.4 | Password strength check (zxcvbn) and 24-word recovery phrase (BIP-39 wordlist) | Weak passwords rejected; phrase round-trips |
| 1.5 | Onboarding UI: welcome, explainer, password, confirm, recovery (show, save, confirm 3 words), Face ID | XCUITest completes onboarding in the simulator |
| 1.6 | Lock screen, auto-lock, app-switcher blur | Vault locks after the timeout and on device lock; keys are zeroized (verified in tests) |
| 1.7 | Settings: change password, regenerate recovery, Face ID toggle, delete vault | All flows tested end to end |

**Exit:** a user can create, lock, unlock, recover, and delete a vault on a real device.

## M2: Collectors

| # | Task | Exit criteria |
| --- | --- | --- |
| 2.1 | **RFC-0001: Atlys Data Model v0** (envelope + `health.*`, `location.*`, `device.*` types) with JSON Schemas in `spec/adm/v0/` | RFC merged; schemas validate the synthetic fixtures |
| 2.2 | `atlys-adm`: types, validation, dedupe keys | Invalid records rejected at merge with clear errors |
| 2.3 | Inbox merge: decrypt, validate, dedupe, single transaction, delete file | Crash during merge loses nothing and duplicates nothing (tested) |
| 2.4 | Collector framework in Swift: protocol, saved state, time budget, scheduling (app open, BGAppRefresh, BGProcessing) | Fake collectors run under every trigger in tests |
| 2.5 | Health collector: permissions, anchored queries with deletions, background delivery, chunked backfill, locked-device deferral | Synthetic HealthKit data in the simulator lands in the vault; a real device keeps collecting for 3 days |
| 2.6 | Location collector: When In Use → Always upgrade, visits, significant changes, `CLServiceSession` | GPX simulation produces visits; a real device records a day of visits in the background |
| 2.7 | Motion collector: 7-day catch-up, segment compression, gap warning | Segments match Core Motion output; warning appears when the gap nears 7 days |
| 2.8 | Device collector: battery, connectivity, device info snapshots | Snapshots recorded on each wake, no permission prompt |

**Exit:** with the app closed, all four collectors keep filling the vault on a real device for 3+ days.

## M3: App UI

| # | Task | Exit criteria |
| --- | --- | --- |
| 3.1 | Query API in the core: by type, time range, source; pagination; daily summary | Queries over 1M synthetic records return in < 200 ms on device |
| 3.2 | Today screen: steps/activity, sleep, places, heart, collection status | Correct numbers against synthetic fixtures |
| 3.3 | Timeline: day view mixing visits, workouts, sleep, activity; local place clustering | Readable day view with no network access |
| 3.4 | Sources screen: permission state, last collected, counts, backfill progress, pending inbox, collect now | Every state reachable in UI tests |
| 3.5 | Explorer: browse by type and date; record detail with JSON and provenance | Any record can be found and inspected |
| 3.6 | Exports: encrypted `.atlysvault` (re-importable) and plaintext JSON (with warning) | Encrypted export imports into a fresh install with the password or recovery phrase |
| 3.7 | About/licenses (including the SQLCipher notice) and privacy statement | Shown in Settings |

**Exit:** the app is usable day to day by the core team.

## M4: Hardening and TestFlight

| # | Task | Exit criteria |
| --- | --- | --- |
| 4.1 | Security review of RFC-0002 and the implementation (internal, plus external reviewers if available) | Findings fixed or documented |
| 4.2 | No-network check in CI; dependency audit (`cargo deny`, `cargo audit`) | CI blocks violations |
| 4.3 | Battery profiling over multi-day real-device runs | Default mode stays within ~2–3% extra per day |
| 4.4 | Resilience: reboot, app kill, low storage, iOS backup/restore, password change mid-collection | All scenarios pass a written test script |
| 4.5 | Accessibility (Dynamic Type, VoiceOver) and String Catalogs | Accessibility audit has no blockers |
| 4.6 | App Store materials: purpose strings, privacy label ("Data Not Collected"), privacy policy page, review notes | Ready for TestFlight external review |
| 4.7 | Signed builds through CI with encrypted GitHub secrets; TestFlight upload | A TestFlight build installs for testers |

**Exit:** TestFlight beta live, meeting the success criteria in [README.md](README.md#success-criteria).

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Background wakes are rarer than expected (iOS throttling) | Gaps in data | Catch up on every wake; Motion's 7-day buffer; clear status on the Sources screen; measured in M0.8 |
| HealthKit locked while the phone is locked | Delayed health data | Defer and resume on unlock (by design) |
| SQLCipher build friction on iOS | Schedule slip | **Resolved in M0.3:** builds with CommonCrypto and no configuration. SQLite3MultipleCiphers remains the fallback |
| First SQLCipher open in a process is slow (~1.3 s on the Simulator, ms afterwards) | Slow first unlock | Measure on device in M0; if real, warm the core while the lock screen shows |
| Simulator can't verify Data Protection, biometric Secure Enclave keys, visits, or (without a team) HealthKit | False confidence from green CI | Device test steps in the spike report; protection tests are strict on devices |
| App Review pushback on "Always" location or HealthKit use | Launch delay | Plain purpose strings, review notes, no continuous GPS in v0 |
| Weak master passwords | Vault brute-forceable if the device is stolen | Strength enforcement + Argon2id; recovery phrase is high entropy |
| Users lose password and recovery phrase | Permanent data loss | Clear onboarding warning, mandatory phrase confirmation, encrypted export reminder |
| Apple Developer account and entitlements | Blocks device testing and TestFlight | Set up during M0. Still open: blocks the M0 device phase, and HealthKit even on the Simulator |
