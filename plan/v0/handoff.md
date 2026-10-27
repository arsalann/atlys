# v0: handoff

This is the working state of v0 implementation, kept current so that anyone (person or agent) can resume after an interruption. Update it in the same commit as every change: what was done, what is in progress, and the exact next step.

Last updated: 2026-10-27

## Resume here

1. Read this file, then [04-work-breakdown.md](04-work-breakdown.md) for task status.
2. Check the open PR and branch listed under [In progress](#in-progress). If its branch has commits that are not described here, read `git log origin/main..` first.
3. Run the [checks](#checks) before changing anything, so you know the starting state.

## Status

| Milestone | State |
| --- | --- |
| M0 spike | Simulator and CI phase done. Device phase still to run (the paid developer account is now available) ([spike-report.md](spike-report.md#follow-ups)). |
| M1 vault + onboarding | **Done.** PRs #6–#9 merged. The app runs on the maintainer's iPhone; the full [device checklist](#device-checklist) is still to be recorded. |
| M2 collectors + raw data viewer | **In progress**: step 1 of 7 (below). |

## In progress

**Next app version: data collection (M2) + raw data viewer.** Scope agreed with the maintainer on 2026-10-27:

- **Collectors.** All four v0 collectors (Device, Motion, Location, Health) seal records into the inbox, including in the background. The vault merges them on unlock.
- **Viewer.** A raw data viewer to browse records by type and day and open any record's JSON and provenance. Browse only: no copy or share out of the app, and no inbox debugging screens.
- **Sources status.** Per-source status on the home screen with "Collect now".
- **Health history.** The backfill imports all Apple Health history, chunked by year with progress.
- **Signing.** Builds use the paid Apple Developer Program, so the HealthKit entitlements are available.
- **Out of scope** (still M3/M4): Today and Timeline screens, place clustering, exports, the 1M-record performance target, battery profiling.

One PR per step:

1. [ ] **RFC-0001 + `atlys-adm` (2.1, 2.2).** Branch `arsalann/m2-data-model`.
   - [x] `atlys-adm` rewritten:
     - Type table (`types.rs`, 21 types).
     - Strict JSON parsing (duplicate keys rejected).
     - Record drafts, deletions, batches (`adm_version` `0.1`), and record JSON with RFC 3339 times.
     - Error codes with paths.
     - The M0 format moved to `atlys_adm::draft` for the spike.
   - [x] `spec/adm/v0/`:
     - Envelope, batch, and record schemas.
     - `types/` generated from the type table, with a drift test.
     - Synthetic `fixtures.json`.
     - `scripts/adm-schemas.py --check` (stdlib) and the CI job `adm-schemas`.
   - [x] [RFC-0001](../../spec/rfcs/0001-data-model.md) (Proposed), including the merge semantics that step 2 implements.
   - [x] Vault inbox: a batch from a newer ADM version stays pending (`NeedsUpdate`).
   - [x] FFI:
     - `RecordDraft` gains optional `tz` and `source_*` fields.
     - New `RecordDeletion` and `IngestSealer.sealWithDeletions`.
     - `SpikeSealer` keeps the draft format.
   - [x] iOS core rebuilt. `Atlys` (unit + UI) and `AtlysSpike` tests pass in the Simulator.
   - [ ] PR open and in review. On merge, set RFC-0001 to Accepted and start step 2.
2. [ ] **Merge and query (2.3, minimal 3.1).**
   - Store schema v2: `records` (unique type + source_id), `tombstones`, and `merge_log` (sealed-file SHA-256).
   - `UnlockedVault::merge_inbox`, per RFC-0001 "Merge", with crash tests.
   - Read API: `record_types`, `records(type, before, limit)`, `record(id)`, and `stats`.
   - FFI: `VaultSession.mergeInbox` plus the read functions.
3. [ ] **Collector framework + Device collector (2.4, 2.8).**
   - `Collector` protocol, state file, `CollectionCoordinator` with a time budget.
   - Triggers: foreground, `BGAppRefreshTask`, `BGProcessingTask`.
   - Merge on unlock and after foreground runs.
   - Device collector: battery, connectivity, info.
4. [ ] **Raw data viewer + sources status (3.5 and 3.4 slices).**
   - Sources section on home.
   - Types → day-grouped list → record detail (JSON + provenance).
   - XCUITest with seeded synthetic records.
5. [ ] **Motion collector (2.7).**
   - 7-day catch-up and segment compression.
   - Gap warning.
   - `NSMotionUsageDescription`.
6. [ ] **Location collector (2.6).**
   - When In Use → Always.
   - Visits + significant changes, holding a `CLServiceSession`.
   - GPX for the Simulator.
7. [ ] **Health collector (2.5).**
   - Entitlements.
   - Anchored queries with deletions.
   - Background delivery.
   - Full-history backfill by year.
   - Deferral while the phone is locked.

M2 exit (device only): with the app closed, all four collectors keep filling the vault for 3+ days. Device-only checks for each step are added to the checklist below as they land.

## Device checklist

The Simulator cannot verify these (see AGENTS.md). Run them on a real iPhone with a signed build of the **Atlys** scheme (`scripts/build-ios-core.sh` first) and record results here.

Recorded so far (2026-10-27): the maintainer installed the M1 build on an iPhone and completed onboarding (item 1 apart from the timing). Items 2–9 are not yet recorded.

### M1


1. Fresh install: complete onboarding and enable Face ID. Note how long "Create vault" takes (Argon2id, 256 MiB, t = 3).
2. Lock, then unlock with Face ID. Cancel the Face ID prompt: the password field still works. Note password unlock time.
3. In iOS Settings, add an alternate appearance (or reset Face ID). Atlys must refuse Face ID unlock and fall back to the password; turning the toggle off and on in Atlys Settings re-enrolls.
4. Auto-lock: set "After 1 minute", leave the app for over a minute, return: locked. Lock the phone while Atlys is open: locked on return.
5. The app switcher shows the privacy cover, not vault contents.
6. "Forgot password?": recover with the written phrase and a new password; the old password no longer works.
7. Settings → New recovery phrase: the old phrase stops working.
8. Delete vault with the password: the app returns to the welcome screen; reinstall-free relaunch shows onboarding.
9. Data Protection: with a development build, check that `vault/vault.db` reads back as `complete` and the container as `completeUntilFirstUserAuthentication`.

## Next steps after M2

1. M0 device phase on a real iPhone, then delete the spike app, the `spike` feature, `atlys_vault::m0`, and `atlys_adm::draft`.
2. M3: Today, Timeline, exports.

## Blockers and open items

- Apple Developer account: now available (paid program). The M0 device phase, final Argon2id parameters, and the Face ID invalidation and Data Protection checks still need to be run and recorded.
- `security@` and `conduct@` contact placeholders still need a project domain.
- Remove `apps/ios/Spike/`, the `AtlysSpike` targets, the `spike` feature, `atlys_vault::m0`, and `atlys_adm::draft` once the M0 device phase is done. Kept until then because the spike app is the device test tool.

## Checks

```sh
python3 -I scripts/vault-format-vectors.py --check
python3 -I scripts/adm-schemas.py --check
cd core && cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features --locked -- -D warnings && cargo test --workspace --locked
scripts/check-no-network.sh
scripts/build-ios-core.sh   # before building the iOS app in Xcode
```

## Conventions

- Branch per task, PR to `main`, every commit signed off (`git commit -s`) with a publishable identity (see [AGENTS.md](../../AGENTS.md)).
- Keep the plan, the docs, and this file current with every change.
