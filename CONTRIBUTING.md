# Contributing to Atlys

Thanks for your interest in Atlys, an open source, local-first personal data vault. This guide covers how to set up a development environment, the conventions we follow, and what we check before merging.

Please read [AGENTS.md](AGENTS.md) first. Its rules apply to every contributor, human or AI. In short: **this repository is public**, so never commit secrets, signing material, personal information, or real personal data.

By participating you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through [SECURITY.md](SECURITY.md), never public issues.

## Project layout

```
core/                 Rust workspace
  crates/atlys-crypto   Argon2id, HKDF, XChaCha20-Poly1305, HPKE, zeroizing key types
  crates/atlys-vault    SQLCipher store, sealed ingest inbox, unlock flows
  crates/atlys-adm      Atlys Data Model record types and validation
  crates/atlys-ffi      UniFFI surface for Swift (and later Kotlin)
  tools/uniffi-bindgen  pinned bindings generator
apps/ios/             SwiftUI app (iOS 18+), local Swift packages, XcodeGen spec
spec/adm/v0/          JSON Schemas for the v0 record types
scripts/              build and CI helper scripts
plan/                 vision, architecture, roadmap, decisions
```

Start with [plan/README.md](plan/README.md) and [plan/v0/](plan/v0/README.md) for the design.

## Development setup

### Requirements

- macOS with **Xcode 16 or later** (for the iOS app). Linux works for the Rust core alone.
- **Rust** via [rustup](https://rustup.rs). `core/rust-toolchain.toml` pins the version and the iOS targets; rustup installs them on first use.
- **XcodeGen** ([yonaskolb/XcodeGen](https://github.com/yonaskolb/XcodeGen)), only if you change `apps/ios/project.yml`. CI uses version 2.46.0.
- Optional: [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) and [`gitleaks`](https://github.com/gitleaks/gitleaks) to run the CI checks locally.

You do **not** need an Apple Developer account to build and test on the iOS Simulator.

### Rust core

```sh
cd core
cargo build
cargo test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

### iOS app

```sh
scripts/build-ios-core.sh          # Rust -> XCFramework + Swift bindings (run after any core change)
open apps/ios/Atlys.xcodeproj      # or build from the command line:
cd apps/ios
xcodebuild test -project Atlys.xcodeproj -scheme Atlys \
  -destination "$(../../scripts/ios-sim-destination.sh)"
```

`scripts/build-ios-core.sh` writes `AtlysCoreFFI.xcframework` and the generated `AtlysCore.swift` into `apps/ios/Packages/AtlysCore/`. Both are build outputs and are gitignored.

`Atlys.xcodeproj` is generated from `apps/ios/project.yml`. Edit the YAML, run `xcodegen generate` in `apps/ios/`, and commit both. CI fails if they drift. Declare entitlements under the target’s `entitlements.properties` in the YAML; XcodeGen rewrites the entitlement plist. Run `python3 -I scripts/check-ios-health-entitlements.py` after regeneration.

### Running on a device

Device builds need your own Apple Developer team:

```sh
cp apps/ios/Config/Local.xcconfig.example apps/ios/Config/Local.xcconfig
# edit DEVELOPMENT_TEAM and ATLYS_BUNDLE_ID_PREFIX
```

The Atlys target requires HealthKit with Clinical Health Records and Background Delivery. Let Xcode refresh the provisioning profile for these capabilities, then rebuild and reinstall. Verify “Turn on Apple Health” on a signed iPhone build; an unsigned build or Simulator run does not verify the installed entitlements.

`Local.xcconfig` is gitignored. Never commit it, and never commit provisioning profiles, certificates, or `.p12` files.

## Rules that CI enforces

| Check | What it does |
| --- | --- |
| `cargo fmt`, `cargo clippy -D warnings`, `cargo test` | Rust style, lints, and tests (macOS and Linux) |
| `cargo deny check` | License allowlist, security advisories, banned crates (no HTTP/TLS stacks, no OpenSSL) |
| XCFramework build + `xcodebuild test` | The core builds for iOS and the Swift tests pass on a simulator |
| XcodeGen drift | `Atlys.xcodeproj` matches `project.yml` |
| gitleaks | Secret scan of the full history |
| No-network check | Fails on `URLSession`, `Network.framework` connections, WebKit, and similar APIs in app code. Read-only `NWPathMonitor` is allowed. See `scripts/check-no-network.sh`. |
| DCO | Every commit has a `Signed-off-by` line |

Other project rules:

- **No third-party Swift packages.** Apple frameworks only.
- **No custom cryptography.** Use the vetted crates already in `atlys-crypto`. New crypto dependencies need discussion in an issue first.
- **No network access** anywhere in the v0 app.
- **All test data is synthetic.** Never use data exported from a real device or account, including your own.
- **Logs never contain record contents or keys.** Use `os.Logger` privacy annotations on iOS.

## Commits and pull requests

- Use [Conventional Commits](https://www.conventionalcommits.org): `feat(vault): add inbox merge`, `fix(ios): ...`, `docs(plan): ...`, `ci: ...`, `chore(deps): ...`. Scopes are usually a crate name without the `atlys-` prefix (`crypto`, `vault`, `adm`, `ffi`), `ios`, `spec`, `plan`, or `ci`.
- **Sign off every commit** under the [Developer Certificate of Origin](https://developercertificate.org): `git commit -s`. The sign-off certifies that you wrote the change or otherwise have the right to submit it under the Apache-2.0 license. The sign-off includes the email in your git config, which becomes public. A GitHub `noreply` address is fine.
- Keep pull requests small and focused, with a description of what changed and how it was tested. Fill in the PR template checklist.
- Before committing, review your staged diff (`git diff --cached`) for secrets, personal information, logs, database files, and binaries.
- Changes to the vault format, cryptography, or the Atlys Data Model need an RFC or a linked design discussion.
- Changes to `AGENTS.md` need maintainer approval.

## License

Atlys is licensed under the [Apache License 2.0](LICENSE). By contributing, you agree that your contributions are licensed under the same terms.
