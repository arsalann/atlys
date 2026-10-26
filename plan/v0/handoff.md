# v0: handoff

This is the working state of v0 implementation, kept current so that anyone (person or agent) can resume after an interruption. Update it in the same commit as every change: what was done, what is in progress, and the exact next step.

Last updated: 2026-10-26

## Resume here

1. Read this file, then [04-work-breakdown.md](04-work-breakdown.md) for task status.
2. Check the open PR and branch listed under [In progress](#in-progress). If its branch has commits that are not described here, read `git log origin/main..` first.
3. Run the [checks](#checks) before changing anything, so you know the starting state.

## Status

| Milestone | State |
| --- | --- |
| M0 spike | Simulator and CI phase done. Device phase blocked on an Apple Developer account ([spike-report.md](spike-report.md#follow-ups)). |
| M1.1 RFC-0002 | **Done.** [RFC-0002](../../spec/rfcs/0002-vault-format.md) accepted and merged (PR #6) with synthetic vectors. |
| M1.2 `atlys-crypto` | **Done.** PR #7 merged: RFC-0002 Argon2id range, header MAC, envelope vectors. |
| M1.3a vault format (Rust) | **Done.** PR #8 merged. |
| M1.3b – M1.7 first working app | **In progress**, see below. |

## In progress

**First working version of the app: M1.3b – M1.7.** Branch `arsalann/m1-app`, one PR, one commit per step. Goal (M1 exit): create, lock, unlock, recover, and delete a vault on a real iPhone.

- [x] **1.4 Rust**: `atlys_vault::password` (zxcvbn score >= 3 and >= 12 characters, enforced on create, change, and recover) and `atlys_vault::recovery` (BIP-39 English 24-word phrase from 32 bytes of entropy, checksum validated, wordlist hash checked).
- [x] **1.3b FFI**: export `VaultSession` and vault functions over UniFFI (create, unlock with password or device envelope, recover, change password, regenerate recovery, device envelope, lock, delete, password strength, phrase validation). `IngestSealer.open(vaultRoot)` writes inbox v2. The spike keeps M0 formats through a separate `SpikeSealer`; the spike app stays until the device phase is done (it is the device test tool).
- [x] **Swift core layer** (`AtlysCore`): `VaultLocation` (container paths, file protection), `BiometricUnlock` (Secure Enclave key + device envelope in the Keychain, scoped by vault ID), `AppModel` state machine.
- [x] **1.5 Onboarding UI**: welcome, how it works, password with strength meter, confirm, recovery phrase (show, confirm 3 words), Face ID.
- [x] **1.6 Lock**: lock screen (password, Face ID, forgot password → recovery), auto-lock timeout, lock on device lock, app-switcher privacy cover.
- [x] **1.7 Settings**: change password, regenerate recovery phrase, Face ID toggle, auto-lock, delete vault (fresh authentication).
- [x] Tests: Rust unit/integration, Swift FFI round trip, XCUITest onboarding → lock → unlock in the Simulator.
- [x] Docs (work breakdown, 03-ios-app) and this file. Open PR.

### Device checklist

The Simulator cannot verify these (see AGENTS.md). Run them on a real iPhone with a signed build of the **Atlys** scheme (`scripts/build-ios-core.sh` first) and record results here:

1. Fresh install: complete onboarding and enable Face ID. Note how long "Create vault" takes (Argon2id, 256 MiB, t = 3).
2. Lock, then unlock with Face ID. Cancel the Face ID prompt: the password field still works. Note password unlock time.
3. In iOS Settings, add an alternate appearance (or reset Face ID). Atlys must refuse Face ID unlock and fall back to the password; turning the toggle off and on in Atlys Settings re-enrolls.
4. Auto-lock: set "After 1 minute", leave the app for over a minute, return: locked. Lock the phone while Atlys is open: locked on return.
5. The app switcher shows the privacy cover, not vault contents.
6. "Forgot password?": recover with the written phrase and a new password; the old password no longer works.
7. Settings → New recovery phrase: the old phrase stops working.
8. Delete vault with the password: the app returns to the welcome screen; reinstall-free relaunch shows onboarding.
9. Data Protection: with a development build, check that `vault/vault.db` reads back as `complete` and the container as `completeUntilFirstUserAuthentication`.

## Next steps after M1

1. M0 device phase on a real iPhone, then delete the spike app, the `spike` feature, and `atlys_vault::m0`.
2. M2: RFC-0001 data model, inbox merge, collectors.

## Blockers and open items

- Apple Developer account: blocks M0 device phase, final Argon2id parameters, Face ID invalidation and Data Protection checks, and HealthKit even on the Simulator.
- `security@` and `conduct@` contact placeholders still need a project domain.
- Remove `apps/ios/Spike/`, the `AtlysSpike` targets, the `spike` feature, and `atlys_vault::m0` once the M0 device phase is done. Kept until then because the spike app is the device test tool.

## Checks

```sh
python3 -I scripts/vault-format-vectors.py --check
cd core && cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features --locked -- -D warnings && cargo test --workspace --locked
scripts/check-no-network.sh
scripts/build-ios-core.sh   # before building the iOS app in Xcode
```

## Conventions

- Branch per task, PR to `main`, every commit signed off (`git commit -s`) with a publishable identity (see [AGENTS.md](../../AGENTS.md)).
- Keep the plan, the docs, and this file current with every change.
