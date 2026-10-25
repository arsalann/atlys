# Plan

Working notes for the relaunch of Atlys as an open source personal data vault and protocol. Everything here is a draft and open for discussion. Nothing is final until it shows up in code or a versioned spec.

| Doc | What it covers |
| --- | --- |
| [01-vision.md](01-vision.md) | Mission, positioning, principles, who it is for |
| [02-legacy-review.md](02-legacy-review.md) | What the original Atlys (2019–2020) built and what to carry forward |
| [03-architecture.md](03-architecture.md) | System layers: connectors → vault → normalization → access → agents/apps |
| [04-data-sources.md](04-data-sources.md) | Connector catalog, ingestion methods, phased rollout |
| [05-data-model.md](05-data-model.md) | The standard schema ("Atlys Data Model") and how records are structured |
| [06-security-privacy.md](06-security-privacy.md) | Threat model, encryption, key management, storage modes |
| [07-open-source-governance.md](07-open-source-governance.md) | License, repo layout, community files, contribution model |
| [08-roadmap.md](08-roadmap.md) | Phased milestones |
| [09-open-questions.md](09-open-questions.md) | Decisions still to be made |
| [10-ideas-backlog.md](10-ideas-backlog.md) | Brainstorm parking lot: ideas not yet scheduled |
| [v0/](v0/README.md) | **v0 plan:** iOS app, account setup, on-device data sources |
| [research/](research/README.md) | Landscape: related projects, standards, regulation, technology options |

## One-paragraph summary

Atlys is a local-first, open source **personal data vault**. It pulls a copy of your data from your phone, your accounts, and the services you use, through APIs, on-device sensors, and data-export ("takeout") requests. It stores that copy encrypted under a key only you hold, then normalizes it into a standard, queryable schema. On top of the vault sits a permissioned **access layer**, starting with an MCP server, so you can point your own AI agent at your own data and get a private personal assistant. The project aims to be a framework and protocol that other apps can build on, so that "build an AI app on top of user data" no longer means "collect the user's data on your servers."
