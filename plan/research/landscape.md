# Landscape: comparable projects

Snapshot as of 2026-10-22 (PDPP added 2026-10-23). Star counts and activity come from GitHub at that date and will drift.

## A. Open source personal data aggregation / archiving

| Project | What it is | License | Activity | Encrypted? | Relevance |
| --- | --- | --- | --- | --- | --- |
| **[Timelinize](https://github.com/timelinize/timelinize)** | Self-hosted app that imports photos, messages, location, and social exports into one SQLite timeline; Go; web UI + CLI + HTTP API | AGPL-3.0 | ~3.7k★, pre-1.0 (v0.0.x) | No (third-party reviews note no encryption at rest) | **Closest open source analogue** for ingestion + timeline. Atlys differentiates on encryption, schema-as-standard, and agent access. |
| **[HPI (Human Programming Interface)](https://github.com/karlicoss/HPI)** | Python package (`my`) that unifies access to personal data exports as Python objects; many modules | MIT | ~1.6k★, active | No | Huge library of export parsers; a model for a community module ecosystem. Parsers are license-compatible to reuse. |
| **[Promnesia](https://github.com/karlicoss/promnesia)** | Browser extension that surfaces your own history and notes on any URL | MIT | ~1.9k★, active | No | Web-history ingestion and contextual recall ideas |
| **[Dogsheep](https://dogsheep.github.io/) / Datasette** | "*-to-sqlite" tools (Twitter, HealthKit, GitHub…) plus Datasette for exploring | Apache-2.0 | Low activity | No | Established the "export → SQLite → query" pattern; good UX reference |
| **[Perkeep](https://github.com/perkeep/perkeep)** (formerly Camlistore) | "Personal storage system for life": content-addressed blob store with indexing | Apache-2.0 | ~7.2k★, v0.12 (Nov 2025) | Optional encrypted blob storage | Content-addressed raw-blob storage design |
| **[Open Humans](https://github.com/OpenHumans/open-humans)** | Platform for collecting personal data streams and donating them to research projects with granular consent | MIT | Low activity (last push Feb 2025) | Server-side | Data donation and consent model |
| **[personal-vault](https://github.com/Tubaleviao/personal-vault)** | Small Node.js library: user-owned data store with revocable consent and a tamper-evident audit log | Open source | Early stage | Yes | Validates the concept; tiny scale |

## B. Personal AI memory / context for agents

| Project | What it is | License | Activity | Relevance |
| --- | --- | --- | --- | --- |
| **[mem0 / OpenMemory MCP](https://github.com/mem0ai/mem0)** | Memory layer for AI apps; OpenMemory runs locally as an MCP server | Apache-2.0 | ~68k★, very active | Shows strong demand for an agent memory layer. It stores *agent-derived memories*, not ingested source data. Atlys can be the data source *underneath* tools like this. |
| **[screenpipe](https://github.com/screenpipe/screenpipe)** | 24/7 local screen + audio capture with OCR/transcription; REST + MCP | Originally MIT; GitHub now shows a non-standard license, so check | ~22k★, very active | A future **connector** source. Strong local-first plus MCP precedent. |
| **[Khoj](https://github.com/khoj-ai/khoj)** | Self-hostable "AI second brain": chat over docs/notes, agents, automations | AGPL-3.0 | ~38k★, active | A consumer of personal data rather than a vault. Possible integration target. |
| Various MCP memory servers (personal-memory-mcp, mcp-memory-service, memorious-mcp…) | Local SQLite/vector memory for agents over MCP | Mostly MIT | Small | Confirms MCP as the integration pattern |

## C. Data portability infrastructure

| Project | What it is | License | Relevance |
| --- | --- | --- | --- |
| **[PDPP + DataConnect](https://github.com/PDP-Connect)** (LF Decentralized Trust lab, originated at Vana) | Protocol for grant-scoped access to personal data, plus a local-first Tauri desktop app, local personal server with MCP, and ~60 TypeScript connectors (Playwright/CDP) | Apache-2.0 (spec text: Community Specification License) | **Closest prior art.** See [pdpp-and-dataconnect.md](pdpp-and-dataconnect.md). |
| **[Data Transfer Project](https://github.com/dtinit/data-transfer-project)** (DTI) | Framework of adapters and common data models for service-to-service transfer (founded by Google, Meta, Microsoft, X, Apple) | Apache-2.0, ~3.6k★, active | Common data models (photos, calendar, contacts, playlists…) are a starting point for the ADM. DTI's trust model and accreditation are relevant if Atlys becomes a *destination* for direct transfers. |
| **[Open Wearables](https://github.com/the-momentum/open-wearables)** | Self-hosted unified API across wearables (HealthKit, Health Connect, Garmin, Oura, Whoop…) with an MCP server | MIT, ~2.7k★, active | Candidate to wrap or partner with for wearable connectors instead of building ~8 integrations |
| **Google Data Portability API** | DMA-driven OAuth API for recurring exports of Google product data | Proprietary API | A Phase 2 connector; EEA users |

## D. Personal data stores / "data wallets" (mostly commercial)

| Project | Model | Status / notes |
| --- | --- | --- |
| **Solid / Inrupt** | Decentralized "pods" (RDF/Linked Data) with app-level access control; Inrupt sells enterprise servers | Standardization continues in the W3C *Linked Web Storage* WG (formed Sept 2024); ODI stewards the project since Oct 2024. Consumer adoption is limited. |
| **HAT / Dataswyft** | Per-user "microserver" personal data accounts; apps request permissioned access | UK; FCA-approved for finance data in 2019; low visibility since |
| **digi.me** | Consent-based personal data exchange; app fetches data to the device and shares it with businesses | Merged with Personal (2017); business model was per-share fees |
| **Vana** | Blockchain "data DAOs" where users pool data to train user-owned AI models | Mainnet Dec 2024; token-based; focused on *collective* data monetization |
| **MyData operators** | Nonprofit framework and certification for human-centric data operators | Governance and principles reference, not software |

## E. Privacy / deletion services

| Service | What it does | Notes |
| --- | --- | --- |
| **Permission Slip (Consumer Reports)** | Authorized-agent app sending opt-out and deletion requests | ~5M requests; moved to DeleteMe in May 2026 |
| **PrivacyBot** (UC Berkeley, 2021) | Open source email-based deletion requests to data brokers | Student project; limited maintenance |
| **DeleteMe, Incogni, Optery** | Commercial data-broker removal | Closed source subscriptions |
| **California DROP** | State-run one-shot deletion request to all registered brokers | Live Jan 2026; brokers must process from Aug 2026 |
| **Have I Been Pwned** | Breach lookup API | Used by legacy Atlys; reuse for privacy audit |

## F. Adjacent consumer "memory" products (closed)

| Product | Note |
| --- | --- |
| **Rewind / Limitless** | Acquired by Meta (Dec 2025); Rewind desktop app wound down; pendant no longer sold |
| **Microsoft Recall** | OS-level screen memory on Copilot+ PCs; faced heavy privacy criticism at launch |
| **Apple Intelligence** | On-device + Private Cloud Compute personal context; closed ecosystem |

## Positioning map

```
                       ENCRYPTED / ZERO-KNOWLEDGE
                                 ▲
                                 │        ★ Atlys (target)
              personal-vault ●   │
                                 │
  SINGLE-SOURCE ◀────────────────┼────────────────▶ MULTI-SOURCE INGESTION
   (memory tools: mem0,          │      ● Timelinize   ● HPI
    screenpipe, Khoj)            │      ● Dogsheep     ● Solid (pods, no ingestion)
                                 │
                                 ▼
                        PLAINTEXT / TRUSTED HOST
```
