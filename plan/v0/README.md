# v0: iOS app, local vault, on-device data

Status: **Final** (2026-10-25). Changes from here on go through PRs that update these docs.

The first shippable Atlys. A simple iPhone app that sets up an encrypted vault protected by a master password, then quietly collects the user's own on-device data (location visits, motion, health, and device telemetry) into that vault. No accounts, no servers, no network.

| Doc | Contents |
| --- | --- |
| [01-account-setup.md](01-account-setup.md) | Onboarding, master password, recovery key, Face ID, lock/unlock, key hierarchy |
| [02-data-sources.md](02-data-sources.md) | The v0 collectors: what each collects, permissions, background behavior, iOS limits |
| [03-ios-app.md](03-ios-app.md) | App architecture, Rust core over UniFFI, the write-only ingest inbox, screens, build, CI, testing |
| [04-work-breakdown.md](04-work-breakdown.md) | Tasks and exit criteria per milestone, and the risk register |
| [handoff.md](handoff.md) | Current implementation state, work in progress, and the next step. Read first when resuming |
| [spike-report.md](spike-report.md) | M0 spike results and decisions: SQLCipher, UniFFI, sealed inbox, Argon2id, Secure Enclave, background collection |

## Goals

1. **Prove the vault.** Real people's data, encrypted end to end on device with the Rust core + SQLCipher, using the key hierarchy that later supports sync and the managed service.
2. **Prove passive collection.** Data keeps arriving for days without the user opening the app, within a reasonable battery budget.
3. **Show immediate value.** A "Today" view and a timeline that make a user's own data visible and understandable.
4. **Set the engineering foundation.** The monorepo layout, Rust core, FFI, CI, and the testing approach that every later phase builds on.

## Non-goals (v0)

- Accounts, sign-in, cloud sync, or the managed service. The vault key hierarchy is designed for them (see [../research/managed-cloud-stack.md](../research/managed-cloud-stack.md)) but nothing is built.
- AI agent / MCP access. That arrives in the next phase through the desktop app/CLI. v0 offers an encrypted backup and a JSON export.
- Third-party connectors, export importers, or browser automation.
- Android. It comes after v0, reusing the same Rust core.
- Insights beyond simple daily summaries.
- **Any network access.** v0 makes zero network requests. That also means no maps, which can come later as an opt-in.

## Success criteria

- A new user sets up a vault and enables sources in **under 2 minutes**.
- With default settings, data from all v0 sources keeps arriving for **7 days** even if the app is opened only occasionally.
- Extra battery use in default mode stays within **~2–3% per day** (measured with Xcode energy tools on a real device).
- The vault survives app termination, device reboot, iOS backup and restore (with the master password), a password change, and recovery-key restore.
- **Zero network requests and zero third-party SDKs**, verified in CI and in code review.
- The crypto design (vault format and key hierarchy) is written up as an RFC and reviewed before TestFlight.

## Milestones

Task-level detail: [04-work-breakdown.md](04-work-breakdown.md).

| # | Milestone | Outcome |
| --- | --- | --- |
| M0 | **Spike** (1–2 weeks; results in [spike-report.md](spike-report.md)) | The Rust core builds for iOS. SQLCipher opens with a raw key on a device. UniFFI bindings work from Swift. An HPKE-sealed inbox file round-trips. Argon2id is benchmarked on the oldest supported iPhone. |
| M1 | **Vault + onboarding** | Create vault, master password, recovery key, Face ID, lock/unlock, change password, recover, delete vault |
| M2 | **Collectors** | HealthKit, location visits, motion activity, and device telemetry write to the sealed inbox, including in the background. Merge on unlock. |
| M3 | **App UI** | Today, Timeline, Sources, Explorer, and Settings screens; encrypted backup and JSON export |
| M4 | **Hardening + TestFlight** | Tests, battery profiling, security review of the RFC, accessibility, App Store privacy details, TestFlight beta |

## Decisions

| Topic | Recommendation |
| --- | --- |
| Minimum iOS version | **iOS 18** (Apple's June 2026 data: ~93% of iPhones that can run iOS 26 are on iOS 26 or 18). Gives modern SwiftUI, Observation, and `CLServiceSession`. |
| UI framework | SwiftUI, Swift 6 strict concurrency |
| Rust ↔ Swift | UniFFI-generated bindings, shipped as an XCFramework inside a local Swift package |
| Background writes while the vault is locked | A **write-only sealed inbox** (see [03-ios-app.md](03-ios-app.md#the-write-only-ingest-inbox)) |
| Continuous GPS tracking | **Not in v0.** Visits and significant location changes only. A "detailed tracking" mode can come later. |
| Screen time | **Not in v0.** Exportable per-app usage on iOS exists only for EU users on iOS 26.4+ with a special Apple entitlement. Plan: v0.1 for EU users, a Mac connector, and Android (see [02-data-sources.md](02-data-sources.md#screen-time-and-app-usage)). |

## Prerequisites outside the repo

- An Apple Developer Program membership (an organization account if the app should be published under a project or company name), HealthKit capability, and a bundle identifier. These are account decisions, not repo content. Signing settings stay in a local, gitignored config file.
- Test devices: one older iPhone that supports iOS 18 (for performance) and one current iPhone, ideally paired with an Apple Watch for richer HealthKit data.
