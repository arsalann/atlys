# Security policy

Atlys stores people's most sensitive data, so we take security reports seriously and appreciate responsible disclosure.

## Reporting a vulnerability

**Please do not open a public issue, discussion, or pull request for security problems.**

Report privately through GitHub's private vulnerability reporting:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Describe the issue, the affected component (for example `core/crates/atlys-crypto` or the iOS app), the impact, and steps to reproduce.

If you cannot use GitHub, email `security@<project-domain>`.

> **Note:** `security@<project-domain>` is a placeholder until the project has its own domain. Until then, GitHub private vulnerability reporting is the only supported channel.

Please do not include real personal data in a report. Synthetic data that reproduces the issue is enough.

## What to expect

- We aim to acknowledge a report within **3 business days** and to share an initial assessment within **10 business days**.
- We will keep you informed while we work on a fix, and agree a disclosure date with you. Our default embargo is 90 days.
- We credit reporters in the advisory unless you ask us not to.

## Supported versions

Atlys is pre-release. No version is supported for production use yet. Security fixes land on `main`. This section will list supported releases once the first TestFlight build ships.

## Scope

In scope:

- The Rust core (`core/`): cryptography, the vault format, key handling, the sealed ingest inbox, and the SQLCipher store.
- The iOS app (`apps/ios/`): key storage, Face ID and Secure Enclave use, background collection, file protection, and any unintended network access.
- The specifications in `spec/` and the design documents in `plan/` (design flaws count).
- CI and release configuration in `.github/`.

Out of scope:

- Attacks that require a jailbroken or already-compromised device, unless Atlys makes them meaningfully worse.
- Brute force of a weak master password (the app enforces a minimum strength; reports that the minimum is too low are welcome).
- Vulnerabilities in third-party dependencies with no demonstrated impact on Atlys. Please report those upstream.

## Design references

- Threat model and key hierarchy: [plan/06-security-privacy.md](plan/06-security-privacy.md) and [plan/v0/01-account-setup.md](plan/v0/01-account-setup.md)
- Write-only sealed inbox: [plan/v0/03-ios-app.md](plan/v0/03-ios-app.md#the-write-only-ingest-inbox)
