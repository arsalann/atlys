# Atlys

**An open source personal data vault and protocol. Own a private copy of your digital life, and connect it to your own AI.**

> **Status: early planning.** There is no usable software yet. The project is being re-founded as open source. Read the [plan](plan/README.md) and join the discussion.

---

## Why

Your data is spread across dozens of apps and services: health in one, location in another, messages, purchases, listening, and browsing history each in their own silo. You can request copies, but they arrive as inconsistent ZIP files that nothing can use. Meanwhile, AI assistants are only useful when they know your context, and today that means uploading your life to someone else's cloud.

Atlys takes the opposite approach:

- **Collect.** Pull a copy of your data from your phone's sensors, from service APIs, and from data-export and data-access requests.
- **Secure.** Store it in an encrypted vault that only you can unlock, like a password manager for your entire digital footprint.
- **Organize.** Clean and normalize everything into a standard, structured, queryable format.
- **Use.** Connect your own AI agent through [MCP](https://modelcontextprotocol.io) with scoped, revocable, audited access, and get a personal assistant that runs on your terms.

## Core components (planned)

| Component | Description |
| --- | --- |
| **Connectors** | Community-maintained integrations that extract personal data from devices, APIs, and exports (Apple Health, Health Connect, Google Takeout, Spotify, …) |
| **Vault** | Encrypted storage protected by your master password. Runs **locally by default**, on **your own storage/database**, or on an optional **zero-knowledge managed service** |
| **Atlys Data Model** | An open, versioned schema that makes data from any source look the same |
| **Access layer** | MCP server and API with fine-grained grants, so agents and apps see only what you allow |
| **Apps** | iOS, Android, desktop/web, and CLI clients |

See [plan/03-architecture.md](plan/03-architecture.md) for the full design.

## Principles

1. **You hold the keys.** Nobody else can read your vault: not the project, not your host, not the managed service.
2. **Local-first.** Works offline on your own device. Cloud is optional.
3. **Least privilege for AI.** Agents get scoped, time-limited, revocable access, and every access is logged.
4. **Open everything.** Open source code, open schema, open protocol.
5. **Community-scaled.** The world has endless data sources. Connectors are designed for contributors to add and maintain.

## Roadmap at a glance

| Phase | Focus |
| --- | --- |
| 0 | Foundations: plan, specs (RFCs), community and security files, CI |
| 1 | Local encrypted vault, CLI, first export importers, local MCP server |
| 2 | Mobile apps with on-device collectors (health, location, usage) |
| 3 | Bring-your-own storage, E2EE multi-device sync, desktop app, security audit |
| 4 | Connector SDKs and registry, Access API, data and deletion requests, managed vault beta |
| 5 | Spec v1.0 and third-party adoption as a standard |

Details: [plan/08-roadmap.md](plan/08-roadmap.md).

## Technology

| Area | Decision |
| --- | --- |
| Vault core, CLI, MCP server, desktop | Rust |
| Connectors | TypeScript (any language through a JSONL protocol) |
| Mobile apps | Native shells over the Rust core |
| On-device database | SQLite + SQLCipher |
| Managed service (later) | Rust + PostgreSQL + S3-compatible storage; zero-knowledge, self-hostable |

Rationale and open items: [plan/09-open-questions.md](plan/09-open-questions.md).

## Repository guide

| Path | Contents |
| --- | --- |
| [`plan/`](plan/README.md) | Vision, architecture, data model, security, governance, roadmap, open questions |
| [`plan/research/`](plan/research/README.md) | Landscape of related projects, standards, regulation, and technology options |
| [`AGENTS.md`](AGENTS.md) | Rules for AI coding agents and contributors working in this repo |

## History

Atlys started in 2019 as a privacy-focused mobile app built by **Atlys Networks** that collected on-device data (location, activity, screen time) into an encrypted local database, with insights, privacy tools, and peer-to-peer sharing. This relaunch rebuilds it from scratch as an open source framework. See [plan/02-legacy-review.md](plan/02-legacy-review.md).

*This project is not affiliated with Atlys, the visa-application company.*

## Contributing

The project is in its planning phase, and feedback on the [plan](plan/README.md) and [open questions](plan/09-open-questions.md) is the most valuable contribution right now. Contribution guidelines, a code of conduct, and a security policy will be added shortly.

**Never include real personal data, credentials, or secrets in issues, pull requests, or commits.**

## License

[Apache License 2.0](LICENSE).
