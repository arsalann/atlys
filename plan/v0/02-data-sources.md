# v0: on-device data sources

Four collectors ship in v0: **Health**, **Location**, **Motion & Activity**, and **Device**. All read only from the phone and all write sealed records to the inbox (see [03-ios-app.md](03-ios-app.md#the-write-only-ingest-inbox)).

## Summary

| Collector | iOS framework | Permission | Background trigger | History available | ADM record types |
| --- | --- | --- | --- | --- | --- |
| Health | HealthKit | Per-type read access | HealthKit background delivery | Everything in Apple Health (often years) | `health.*` |
| Location | Core Location | When In Use → Always | Visit monitoring + significant location change (the app is relaunched) | None (only from install onward) | `location.visit`, `location.point` |
| Motion & Activity | Core Motion | Motion & Fitness | Catch-up on any wake | **~7 days** kept by iOS | `device.motion_activity` |
| Device | UIKit, Network, Foundation | None | Opportunistic on any wake | None | `device.battery`, `device.connectivity`, `device.info` |

## 1. Health (HealthKit)

**v0 read types:**

| Category | HealthKit types | ADM type |
| --- | --- | --- |
| Activity | step count, walking/running distance, flights climbed, active energy, basal energy, exercise minutes, stand hours | `health.steps`, `health.distance`, `health.floors`, `health.energy`, `health.exercise_time`, `health.stand_hour` |
| Heart | heart rate, resting heart rate, heart rate variability (SDNN), walking heart rate average | `health.heart_rate`, `health.resting_heart_rate`, `health.hrv` |
| Sleep | sleep analysis (in bed, asleep core/deep/REM, awake) | `health.sleep_session`, `health.sleep_stage` |
| Workouts | workouts with type, duration, energy, distance | `health.workout` |
| Body | body mass, height | `health.body_measurement` |
| Mindfulness | mindful sessions | `health.mindful_session` |

**How collection works:**

- `HKAnchoredObjectQuery` per type for incremental sync, including **deletions**. Anchors are saved as collector state.
- `HKObserverQuery` + `enableBackgroundDelivery` wake the app when new samples arrive (hourly for most types; HealthKit decides the exact timing).
- **Initial backfill** of all history in chunks (by year), with progress shown on the Sources screen. It may take a while for long-time Apple Watch users.
- Each sample keeps its HealthKit UUID as `source_id` (for dedupe) plus source app and device in provenance.

**iOS constraints:**

- **HealthKit can't be read while the phone is locked** (`errorDatabaseInaccessible`). Background wakes that happen while locked defer collection until protected data becomes available, then resume.
- Apps can't tell whether a read permission was denied (Apple hides it for privacy). The UI shows "no data received" with guidance rather than "denied".
- Requires the HealthKit capability, the background-delivery entitlement, `NSHealthShareUsageDescription`, and a privacy policy. Apple forbids using HealthKit data for advertising or selling it. Atlys records the HealthKit origin on every record so later sharing features can enforce Apple's rules.
- v0 is **read-only**: Atlys never writes to Apple Health.

## 2. Location (Core Location)

**Default mode (battery-friendly):**

- **Visit monitoring** (`startMonitoringVisits`): iOS reports arrivals and departures at places the user spends time. Very low power, and iOS relaunches the app to deliver visits.
- **Significant location changes**: coarse updates (~500 m+) when the user moves, which fill gaps between visits.
- A `CLServiceSession` is held where iOS 18 requires one.

**Records:**

- `location.visit`: arrival time, departure time, coordinates, horizontal accuracy
- `location.point`: timestamp, coordinates, accuracy, speed and course when available

**Later (not v0):** a "detailed tracking" mode using `CLLocationUpdate.liveUpdates` in the background for route-level data. It costs a lot of battery and draws App Review scrutiny, so it would be opt-in only. Place naming (reverse geocoding) needs the network, so v0 shows coordinates and clusters frequently visited places locally.

**Permissions:** "When In Use" first, then an explained upgrade to "Always". Without "Always", only foreground data is collected, and the Sources screen says so. Info.plist: `NSLocationWhenInUseUsageDescription`, `NSLocationAlwaysAndWhenInUseUsageDescription`. Background location mode only becomes needed with detailed tracking.

## 3. Motion & Activity (Core Motion)

- `CMMotionActivityManager.queryActivityStarting(from:to:to:)`: stationary, walking, running, cycling, automotive, unknown, with confidence. Consecutive identical samples are compressed into segments.
- **iOS keeps only about the last 7 days.** The collector runs on every wake (app open, background refresh, location or HealthKit wakes) to catch up, and the Sources screen warns if the gap approaches 7 days.
- Record: `device.motion_activity`: start, end, activity, confidence.
- The pedometer (`CMPedometer`) overlaps HealthKit step data, so it's skipped in v0.
- Permission: Motion & Fitness (`NSMotionUsageDescription`).

## 4. Device telemetry

Low-frequency snapshots taken whenever the app is awake for any reason:

| Record | Fields | Source |
| --- | --- | --- |
| `device.battery` | level, state (charging/full/unplugged), Low Power Mode | `UIDevice`, `ProcessInfo` |
| `device.connectivity` | path status, interface type (Wi-Fi/cellular/wired), expensive/constrained flags | `NWPathMonitor` (no network requests; Wi-Fi network names need a special entitlement and are out of scope) |
| `device.info` | iOS version, device model, app version, free storage, locale/time zone changes | `UIDevice`, `FileManager`, `TimeZone` |

There is no permission prompt. The value is modest (charging habits, time-zone changes when traveling), but it exercises the whole pipeline cheaply.

## Screen time and app usage

There is no single answer: it depends on region, OS version, and device.

| Path | What you get | Who it works for | Status |
| --- | --- | --- | --- |
| **App & website usage entitlement** (`com.apple.developer.family-controls.app-and-website-usage`, `DeviceActivityData.activityData(filteredBy:using:)`, iOS 26.4+) | Real per-app and per-website usage data readable by the app | **Customers only in the EU** (EU device + EU Apple Account); development works anywhere. Needs Apple's entitlement approval and user consent. **Only one app at a time** can hold this access. | Best option where available. Target v0.1 behind a feature check. |
| Standard Screen Time API (`DeviceActivityReport`) | Usage can be *displayed* inside a sandboxed extension, but not stored or exported | Everyone | Not usable for a vault. Apple confirmed this is intended. |
| `DeviceActivityMonitor` thresholds | "Used app X (or all apps) for at least N minutes today" callbacks. A ladder of thresholds approximates totals. | Everyone (Family Controls entitlement) | Coarse, and developers report premature or unreliable callbacks on iOS 17 and 26. Experimental only. |
| **Mac connector** (Phase 2) | Per-app usage from the Mac's Screen Time stores (`knowledgeC.db`, Biome), including iPhone usage synced through "Share across devices" | Users with a Mac and Full Disk Access granted | Undocumented and changes between macOS versions, but widely used by existing tools. Treat as best-effort. |
| **Android** (Phase 2) | Full per-app usage through `UsageStatsManager` | All Android users (special permission) | Reliable |
| Shortcuts automations | "App opened" events for apps the user chooses, logged through an Atlys App Intent | Everyone, with manual setup per app | Possible add-on; fragile |

## Not possible on iOS (documented on purpose)

| Data | Why |
| --- | --- |
| Call and SMS history | No public API |
| Other apps' data | Sandboxed. Comes through exports or connectors in later phases. |

## Next candidates (v0.1+)

Calendar events (EventKit), contacts (Contacts), photo metadata without pixels (PhotoKit), and the Apple Health full export file importer (`export.zip`) for data HealthKit queries don't expose.

## Collection scheduling

| Trigger | What runs |
| --- | --- |
| App opened | All collectors catch up; the inbox merges if the vault is unlocked |
| HealthKit background delivery | Health collector (if protected data is available), then motion and device |
| Location visit or significant change | Location record, then motion and device catch-up |
| `BGAppRefreshTask` (iOS decides when) | All collectors, ~25-second budget |
| `BGProcessingTask` (charging, idle) | Long HealthKit backfills; inbox compaction |

Every run is idempotent: collectors resume from saved state and dedupe by `source_id`, so overlapping triggers are harmless.
