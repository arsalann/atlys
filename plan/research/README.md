# Research

Landscape research for the Atlys relaunch. Last updated: 2026-10-23.

| Doc | Contents |
| --- | --- |
| [landscape.md](landscape.md) | Comparable projects and products: open source and commercial |
| [standards-and-protocols.md](standards-and-protocols.md) | Solid, DTP/DTI, MCP, AT Protocol, MyData, health/data standards |
| [regulation.md](regulation.md) | GDPR, CCPA/CPRA, California Delete Act (DROP), EU DMA portability, EU Data Act |
| [market-lessons.md](market-lessons.md) | What happened to earlier personal-data companies, and what it teaches |
| [technology-options.md](technology-options.md) | Storage, crypto, sync, and agent-interface building blocks |
| [language-choice.md](language-choice.md) | Rust vs. TypeScript vs. others, including browser automation and mobile constraints |
| [local-database.md](local-database.md) | On-device database options and the SQLCipher recommendation |
| [managed-cloud-stack.md](managed-cloud-stack.md) | Zero-knowledge managed service design: server, metadata DB, object storage, auth, agent relay |
| [pdpp-and-dataconnect.md](pdpp-and-dataconnect.md) | The Personal Data Portability Protocol (Linux Foundation) and its reference app: closest prior art |

## Key takeaways

1. **No one owns this exact space.** There are strong open source *importers/timelines* (Timelinize, HPI, Dogsheep), strong *memory-for-AI* tools (mem0/OpenMemory, screenpipe, Khoj), and strong *standards efforts* (Solid, DTP/DTI, MCP). No project combines **multi-source ingestion + a zero-knowledge encrypted vault + a standard schema + permissioned agent access**. PDPP/DataConnect comes closest but leaves encryption at rest unspecified (see #8). That combination is the gap Atlys targets.
2. **Encryption is the differentiator.** The leading self-hosted personal-data tools (Timelinize, HPI, Dogsheep) generally store data **unencrypted** and assume a trusted machine. A password-manager-grade vault is a real gap.
3. **MCP is the agent interface to bet on.** It is now under Linux Foundation governance (Agentic AI Foundation, Dec 2025) with broad industry buy-in, and the spec has formal OAuth-based authorization.
4. **Regulation is pushing in the direction Atlys needs.** DMA Art. 6(9) forced continuous, real-time portability APIs (for example, Google's Data Portability API). California's DROP platform went live in 2026. DTI is pushing AI personal-data portability. Ingestion is getting easier by law.
5. **Centralized "personal data stores" mostly struggled as businesses.** Solid/Inrupt, HAT/Dataswyft, and digi.me spent a decade on two-sided "data exchange" models. The lesson: deliver **direct user value first** (a vault plus an AI assistant), and treat the business ecosystem as secondary.
6. **Personal AI memory held by vendors is fragile.** Meta acquired Limitless (formerly Rewind) in Dec 2025 and wound down the Rewind desktop app. This makes the "your memory should live in your own vault" argument easy to explain.
7. **Reuse, don't reinvent.** HPI (MIT), Open Wearables (MIT), DTP (Apache-2.0), and Promnesia (MIT) have parsers and models that are license-compatible with Apache-2.0. Timelinize and Khoj are AGPL, so use them as design references unless Atlys also goes AGPL.
8. **PDPP is the closest prior art, and a likely foundation.** The Personal Data Portability Protocol (PDP-Connect lab, LF Decentralized Trust) already defines grants, MCP access, and a JSONL connector protocol nearly identical to the Atlys plan, with ~60 Apache-2.0 TypeScript connectors. It is deliberately silent on encryption at rest and light on mobile, which are exactly Atlys's strengths. Aligning with it is probably better than competing.
9. **Rust core + TypeScript connectors.** Rust lacks an official Playwright binding and has thin browser tooling. Browser automation can't run on iOS except as JavaScript in a WebView. So connectors should be TypeScript, while the vault stays Rust.
