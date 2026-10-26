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
| M1.3 `atlys-vault` | **In progress**, see below. |
| M1.4 – M1.7 | Not started. |

## In progress

**Task 1.3: `atlys-vault` product format and lifecycle.** Branch `arsalann/m1-vault-lifecycle`. Split into two PRs:

- **1.3a (this branch): Rust only.** The Swift app and FFI keep using the M0 formats until 1.3b, so the M0 inbox and Secure Enclave envelope move to `atlys_vault::m0` unchanged.
  - [ ] `header`: strict two-layer JSON (64 KiB limit, lowercase hex, no unknown/duplicate fields, exact versions), encode/parse, MAC verify before trust.
  - [ ] `envelope`: password/recovery/ingest wraps with RFC-0002 AAD; device envelope v2 (vault ID bound, AAD over bytes 0..147).
  - [ ] `inbox`: sealed inbox v2 (vault ID in HPKE info and AAD), padding and bucket checks, bounded reads, symlink rejection, exclusive temp + rename + parent sync, quarantine with reason codes, unsupported versions left pending.
  - [ ] `store`: `create` vs `open` (never create on unlock), `cipher_memory_security`, quiet SQLCipher logging, reject newer schema, release the connection even if checkpoint fails.
  - [ ] `vault`: create (temp sibling dir, atomic publish), unlock with password / device envelope, recover (requires a new password), lock, change password, regenerate recovery, enable/disable device envelope, delete (fresh authentication). Durable header replacement.
  - [ ] Integration tests: every flow, wrong credentials, every header field mutation, tampered envelopes, vault-ID substitution, KDF bounds, missing database, crash leftovers.
  - [ ] Docs: RFC cross-references, `03-ios-app.md` API, work breakdown, this file. Open PR.
- **1.3b (next):** FFI `Vault` object replacing `SpikeVault`, `IngestSealer` writing inbox v2 (takes the vault ID), Swift `InboxLocation` and CryptoKit interop test on v2, delete `atlys_vault::m0`. Header and inbox v2 interop fixtures in `spec/vault/v1/vectors.json`.

## Next steps after 1.3

1. **1.4**: zxcvbn check and BIP-39 24-word phrase (1.3 takes the 32-byte recovery entropy directly and enforces only the 12-character minimum).
2. **1.5 – 1.7**: iOS onboarding, lock, settings.

## Blockers and open items

- Apple Developer account: blocks M0 device phase, final Argon2id parameters, Face ID invalidation and Data Protection checks, and HealthKit even on the Simulator.
- `security@` and `conduct@` contact placeholders still need a project domain.
- Remove `apps/ios/Spike/`, the `AtlysSpike` targets, and the `spike` feature before M1 features land in the app.

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
