# v0: handoff

This is the working state of v0 implementation, kept current so that anyone (person or agent) can resume after an interruption. Update it in the same commit as every change: what was done, what is in progress, and the exact next step.

Last updated: 2026-10-31

## Resume here

1. Read this file, then [04-work-breakdown.md](04-work-breakdown.md) for task status.
2. Check the open PR and branch listed under [In progress](#in-progress). If its branch has commits that are not described here, read `git log origin/main..` first.
3. Run the [checks](#checks) before changing anything, so you know the starting state.

## Status

| Milestone | State |
| --- | --- |
| M0 spike | Simulator and CI phase done. Device phase still to run (the paid developer account is now available) ([spike-report.md](spike-report.md#follow-ups)). |
| M1 vault + onboarding | **Done.** PRs #6–#9 merged. The app runs on the maintainer's iPhone; the full [device checklist](#device-checklist) is still to be recorded. |
| M2 collectors + raw data viewer | **Implemented** (PRs #10–#12). Remaining: device verification ([M2 checklist](#m2)) and the 3-day exit run. |

## Apple Health activation fix (2026-10-31)

- Crash evidence: base authorization failed with a missing HealthKit entitlement, then clinical authorization raised an uncaught Objective-C exception. The generated app entitlement plist was empty.
- Added HealthKit, Health Records, and background-delivery properties to `project.yml` and regenerated the plist. CI checks the generated capabilities and purpose strings with `scripts/check-ios-health-entitlements.py`.
- The collector stops after base authorization fails and checks `supportsHealthRecords()` before clinical authorization or collection.
- Validation: Rust XCFramework built; unsigned iPhone build and 14 collector Simulator tests passed (Xcode 27.1); regeneration is stable; capability check passes and rejects an empty entitlement fixture; no-network and diff checks passed.
- Signed-device verification is still required: refresh provisioning with the configured team, rebuild/reinstall, tap “Turn on Apple Health”, and verify the permission sheet and collection. Also exercise a device/region without clinical support; ordinary Health data must still collect.

## In progress

**Next app version: data collection (M2) + raw data viewer.** Scope agreed with the maintainer on 2026-10-27:

- **Collectors.** All four v0 collectors (Device, Motion, Location, Health) seal records into the inbox, including in the background. The vault merges them on unlock.
- **Viewer.** A raw data viewer to browse records by type and day and open any record's JSON and provenance. Browse only: no copy or share out of the app, and no inbox debugging screens.
- **Sources status.** Per-source status on the home screen with "Collect now".
- **Health history.** The backfill imports all Apple Health history, chunked by year with progress.
- **Signing.** Builds use the paid Apple Developer Program, so the HealthKit entitlements are available.
- **Data rules** (maintainer, 2026-10-27, also in AGENTS.md):
  - Collect everything accessible.
  - Store raw values and units.
  - Deletions at the source set a flag; records are never removed.
- **Out of scope** (still M3/M4): Today and Timeline screens, place clustering, exports, the 1M-record performance target, battery profiling.

One PR per step:

1. [x] **RFC-0001 + `atlys-adm` (2.1, 2.2).** PR #10, merged.
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
   - [x] [RFC-0001](../../spec/rfcs/0001-data-model.md) (Accepted), including the merge semantics that step 2 implements.
   - [x] Vault inbox: a batch from a newer ADM version stays pending (`NeedsUpdate`).
   - [x] FFI:
     - `RecordDraft` gains optional `tz` and `source_*` fields.
     - New `RecordDeletion` and `IngestSealer.sealWithDeletions`.
     - `SpikeSealer` keeps the draft format.
   - [x] iOS core rebuilt. `Atlys` (unit + UI) and `AtlysSpike` tests pass in the Simulator.
   - [x] Merged; RFC-0001 accepted.
2. [x] **Data model revision + merge and query (2.3, minimal 3.1).** PR #11, branch `arsalann/m2-merge`.
   - RFC-0001 revised for the data rules:
     - Generic Apple Health families: `health.quantity`, `health.category`, `health.workout`, routes, ECG, heartbeat series, audiogram, State of Mind, clinical records, characteristics, and `health.sample`.
     - Raw Core Motion updates plus `device.pedometer`.
     - Every Core Location field.
     - Deletions flag records (`deleted_at_source`) and never remove them.
   - Store schema 2 (`records`, `merge_log`), migrated from schema 1.
   - `atlys_vault::records::merge_inbox`: deadline, crash replay via the merge log, quarantine, update-needed.
   - Read API: `record_types` (by type and subtype), `collector_summaries`, `records` (keyset pages), `record` (JSON), and previews.
   - FFI: `VaultSession.mergeInbox(maxSeconds)`, `recordCount`, `recordTypes`, `collectorSummaries`, `records`, `recordJson`, `inboxStatus`.
3. [x] **Collectors, viewer, and app wiring (2.4–2.8, 3.4 and 3.5 slices).** PR #12, branch `arsalann/m2-collectors`.
   - **`AtlysCollectors` framework**
     - `Collector` protocol, `CollectorState` files in `<vault root>/collectors/`, and `CollectionCoordinator`, which serializes runs and enforces a time budget.
     - `CollectionService`, started from the app delegate at launch, registers HealthKit observers, the location manager, `BGAppRefreshTask` (`<bundle id>.refresh`), and `BGProcessingTask` (`<bundle id>.processing`).
     - Batches are sealed in chunks of at most 2000 records. If the core rejects a batch, its records are sealed one at a time and only invalid ones are skipped, so one bad sample can't stall an anchor.
     - While the app is open, collection repeats in rounds while a backfill has more to read (up to 30 rounds), merging in between.
   - **Device:** battery, network path (`NWPathMonitor`, no requests), device info. At most one snapshot per 15 minutes, unless "Collect now".
   - **Motion:** raw activity updates and hourly pedometer totals, catching up over the 7 days iOS keeps, with a gap warning after 5 days.
   - **Location**
     - Visits and significant changes are sealed as they arrive, even while the vault is locked.
     - A fix is taken when the app opens.
     - Optional detailed tracking (Settings) uses `CLLocationUpdate.liveUpdates` and `CLBackgroundActivitySession`.
     - Permissions go When In Use, then Always.
   - **Health**
     - Every readable type: about 115 quantity types, about 70 category types, workouts (statistics, events, activities, routes), ECG voltages, heartbeat series, audiograms, State of Mind, GAD-7/PHQ-9, characteristics, and clinical records.
     - Anchored queries with deletions, paging through all history 2000 samples at a time, so the backfill resumes across runs.
     - Values are stored in Apple Health's preferred unit for each type.
     - Runs while the phone is locked are deferred.
     - One observer query covers all types, with background delivery.
   - **App**
     - Entitlements: HealthKit, Health Records, background delivery.
     - Purpose strings and background modes (`fetch`, `processing`, `location`); version 0.2.0 (2).
     - `AppModel` collects on launch and foreground, merges in 1-second slices after unlock, and exposes the browser queries.
   - **UI**
     - Home has "Browse your data" (record count, pending batches) and Sources (permission, last collected, count, history import progress, Turn on / Allow Always / Open Settings, Collect now, Turn on all).
     - The browser goes from types (grouped by family and subtype) to a day-grouped list to the record JSON. It is read-only, with no copy or share.
     - Lock moved to the toolbar.
   - **Tests**
     - Coordinator with fake collectors under every trigger, sealing in chunks, state, record builders, and display names.
     - The UI test now goes onboarding → Device record collected, merged, browsed, and opened → lock and unlock → relaunch.

M2 exit (device only): with the app closed, all four collectors keep filling the vault for 3+ days. See the [M2 checklist](#m2).

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

### M2

1. **Install.** Pull `main`, run `scripts/build-ios-core.sh`, and build the **Atlys** scheme signed with your team. If Xcode reports a missing capability, open Signing & Capabilities and let it add HealthKit (with Clinical Health Records and Background Delivery) and Background Modes.
2. **Your existing vault.** Unlock the M1 vault. It migrates to schema 2, and "Browse your data" shows Device records within seconds.
3. **Permissions.** Tap "Turn on all sources":
   - Health: enable every category in the sheet. Health Records gets its own sheet if a provider is connected.
   - Motion: allow.
   - Location: While Using, then Change to Always Allow.
4. **History import.** Health's history import progresses ("x of y types"). Note how long a full import takes and whether it finishes while charging overnight (`BGProcessingTask`).
5. **Raw values.** In the browser, check Health measurements against Apple Health: values and units, a workout with its route, and sleep stages.
6. **Deleted at source.** Delete a sample in Apple Health, then Collect now. The record stays in Atlys and shows "Deleted at source".
7. **Locked phone.** Lock the phone, walk somewhere, and come back:
   - Visits and location points arrive.
   - Health runs while locked show "Waiting for the iPhone to be unlocked" and catch up after unlock.
   - Batches sealed while the vault was locked merge on unlock.
8. **Motion.** Activity and hourly pedometer records cover the last 7 days after the first run.
9. **Detailed location tracking** (Settings): points arrive continuously in the background. Note the battery cost over a few hours.
10. **3-day run.** Open the app only occasionally. All four sources keep filling the vault (the M2 exit criterion). Record battery use from Settings → Battery.

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
