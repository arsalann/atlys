# Atlys Spike (M0, throwaway)

A test app for milestone M0 (see [plan/v0/04-work-breakdown.md](../../../plan/v0/04-work-breakdown.md)). Results are in [plan/v0/spike-report.md](../../../plan/v0/spike-report.md).

**Delete this directory, the `AtlysSpike` targets in `project.yml`, and the `spike` feature of `atlys-ffi` before M1 feature work.** Nothing here is product code. In particular, `SpikeVault` stores its private keys **unencrypted** on disk.

| Tab | M0 task | What it does |
| --- | --- | --- |
| Core | 0.3, 0.4, 0.5 | SQLCipher self-test (raw key, wrong key, no plaintext on disk); core version through UniFFI; seal and open synthetic inbox batches |
| Argon2 | 0.6 | Runs the Argon2id calibration grid and copies a Markdown table |
| Face ID | 0.7 | Wraps the stand-in vault key to a Secure Enclave key (`biometryCurrentSet`) and unlocks with Face ID |
| Background | 0.8 | Visit monitoring, significant location changes, and a HealthKit step observer that seal records into the inbox, plus a wake log |

The wake log (`Application Support/spike/wake-log.jsonl`) records only times, triggers, app state, and outcomes. It never records coordinates or health values. Even so, **do not commit exported wake logs or screenshots of real data**. Summarize them in the spike report instead.

## Running

```sh
scripts/build-ios-core.sh
cd apps/ios
xcodebuild test -project Atlys.xcodeproj -scheme AtlysSpike -destination "$(../../scripts/ios-sim-destination.sh)"
```

The Simulator covers SQLCipher, UniFFI, the inbox, Argon2 on the host CPU, Secure Enclave wrap/unwrap without biometry, and significant location changes (`xcrun simctl location <udid> start ...`). Device-only steps are listed in the spike report.

For a device or HealthKit run, set `DEVELOPMENT_TEAM` in `Config/Local.xcconfig` (see `Local.xcconfig.example`). Without a team, Xcode does not embed the HealthKit entitlements, even in Simulator builds.
