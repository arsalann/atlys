# 07 — Open source governance & project standards

## License

**Decided (2026-10-23): Apache-2.0 for the whole project.** The options that were considered are kept below for reference.

| Component | Option A (simple) | Option B (protect the hosted core) |
| --- | --- | --- |
| Spec (ADM, Access protocol) | Apache-2.0 / CC-BY-4.0 for docs | Same |
| SDKs, connectors, core library | Apache-2.0 | Apache-2.0 |
| Apps + sync/managed server | Apache-2.0 | AGPL-3.0 (like Bitwarden server, Vaultwarden, Timelinize) |

Apache-2.0 everywhere maximizes adoption by companies, which serves the "standard" goal. AGPL on the server discourages closed hosted forks of the managed service. See [09-open-questions.md](09-open-questions.md). Whatever is chosen, the **spec and SDKs should stay permissive** so companies can adopt the standard.

Contributions: use the **DCO** (`Signed-off-by`) rather than a CLA, unless dual licensing becomes necessary.

## Repository community files (to add)

| File | Purpose |
| --- | --- |
| `README.md` | What, why, status, quick start |
| `LICENSE` | Present (Apache-2.0) |
| `AGENTS.md` | Rules for AI agents and contributors (present) |
| `CONTRIBUTING.md` | Dev setup, branching, commit conventions, DCO, how to add a connector |
| `CODE_OF_CONDUCT.md` | Contributor Covenant 2.1, with a project (not personal) contact address |
| `SECURITY.md` | Private vulnerability reporting via GitHub Security Advisories; supported versions |
| `GOVERNANCE.md` | Roles (maintainers, connector owners), decision making, RFC process |
| `CODEOWNERS` | Per-directory and per-connector ownership |
| `.github/ISSUE_TEMPLATE/` | Bug, feature, **new connector request**, connector breakage |
| `.github/PULL_REQUEST_TEMPLATE.md` | Checklist including "no secrets / no real personal data" |
| `.github/workflows/` | Lint, test, secret scan, schema validation, CodeQL |
| `CHANGELOG.md` | Keep a Changelog format; per-package changelogs in a monorepo |
| `docs/rfcs/` | Design proposals for spec changes |

## Conventions

- **Conventional Commits** (`feat(connector-spotify): …`) enable automated changelogs and releases.
- **SemVer** for packages. The spec has its own version. Connectors are versioned independently.
- **Trunk-based development:** short-lived branches, PRs to `main`, required review and CI.
- An **RFC process** for changes to the ADM or Access protocol, because they affect every adopter.
- OpenSSF Scorecard and Best Practices badge as a quality target.

## Governance path

1. **Now:** a single maintainer (BDFL-style), with documented decisions in `plan/` and RFCs.
2. **Growing:** a maintainer group plus connector owners; lazy consensus; RFCs for spec changes.
3. **Standard ambitions:** consider moving the spec to a neutral home (a foundation such as the Linux Foundation or OpenSSF-adjacent, or a W3C Community Group) once there are outside adopters. MCP's move to the Agentic AI Foundation is a relevant precedent.

## Sustainability

- An optional **managed vault** subscription (zero-knowledge), the Bitwarden model.
- Sponsorships (GitHub Sponsors / Open Collective) and grants (NLnet/NGI, Mozilla, Sovereign Tech Fund, privacy foundations).
- Paid support or hosting for companies adopting the protocol.
