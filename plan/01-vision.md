# 01 — Vision

## Mission

Give every person a private, complete, and portable copy of their own data, and let them put it to work, including with AI, without handing it to anyone else.

## The problem

- **Your data is scattered.** Health is in one app, location in another, messages, purchases, browsing, and photos each sit in a different silo. Each silo is optimized for its owner, not for you.
- **Your data is owned by others in practice.** You can request exports (GDPR/CCPA, Google Takeout, the DMA portability APIs), but what you get is a pile of ZIPs in inconsistent formats that nothing can use.
- **AI assistants need context, and today that means surrendering data.** A useful personal assistant needs your calendar, health, location, messages, and history. The current default is to upload all of it to a vendor's cloud. Companies holding "personal memory" data can be acquired or shut down, and their terms can change. For example, Rewind/Limitless was acquired in December 2025 and its desktop product was wound down (see [research/market-lessons.md](research/market-lessons.md)).
- **There is no standard.** Every app that wants to "use your data" builds its own ingestion, its own schema, and its own storage. Nobody can interoperate, and users can't move their data.

## What Atlys is

1. **Connectors**: open source, community-maintained integrations that extract personal data from devices, APIs, and data exports.
2. **Vault**: an encrypted store that only the user can unlock. It runs locally by default, can be self-hosted on the user's own storage or database, or can run as an optional managed (zero-knowledge) service.
3. **Data model**: a standard, versioned schema that turns raw exports into clean, structured, queryable records.
4. **Access layer**: a permissioned interface, with MCP first, that lets the user's chosen AI agent or app query the vault with scoped, revocable, audited access.
5. **Reference apps**: iOS, Android, desktop/web, and CLI clients that make all of this usable by non-developers.

The long-term goal is for items 3 and 4 to become a **protocol/standard**: the expected way to build AI apps on top of personal data, adopted beyond this codebase.

## What Atlys is not (for now)

- Not a data marketplace. Selling or sharing data was part of the original vision and remains an idea (see [10-ideas-backlog.md](10-ideas-backlog.md)), but it is not part of the core.
- Not an AI model or a chatbot. Atlys is the data and access layer. Users bring their own agent (local or hosted).
- Not a cloud service that sees your data. Even the managed option must be zero-knowledge.
- Not a password manager, although the security model borrows from one.

## Principles

1. **User holds the keys.** Data is encrypted with keys derived from a secret only the user knows. No one else, including the project, a host, or the managed service, can read it.
2. **Local-first.** Everything works offline on the user's own device. Sync and cloud are optional add-ons.
3. **Copy, don't move.** Atlys keeps a copy of your data. Deleting from the source is a separate, explicit user action.
4. **Least privilege for agents.** Agents get scoped, time-limited, revocable access. Every access is logged in an audit trail the user can read.
5. **Open by default.** Open source code, an open schema, an open protocol, and open governance. No proprietary lock-in, including in the managed service.
6. **Raw is sacred.** Original exports are preserved (encrypted) next to normalized data, so parsers can be fixed and re-run without re-requesting data.
7. **Community-scaled.** The number of data sources is unbounded, so the connector SDK, testing, and contribution path must make it easy for others to add and maintain sources.

## Who it is for

| Audience | What they get |
| --- | --- |
| Privacy-conscious individuals | One encrypted place for their digital footprint, plus insight into what services hold about them |
| Personal-AI users and builders | A private, structured context source for their own assistant (Claude, local LLMs, etc.) through MCP |
| Developers and companies | A standard way to build AI features on user data without holding the data themselves |
| Researchers | (Later) consented, user-controlled data donation, similar to Open Humans |

## Positioning in one line

> **"Bitwarden for your personal data, with an MCP port for your own AI."**
