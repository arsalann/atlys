# Standards & protocols

## Model Context Protocol (MCP): the agent interface

- Open protocol for connecting AI applications to tools and data. Donated by Anthropic to the **Agentic AI Foundation** (a Linux Foundation directed fund co-founded with Block and OpenAI) in **December 2025**. Supported by OpenAI, Google, Microsoft, and AWS.
- The Nov 2025 spec formalized **OAuth 2.1-based authorization** for remote servers. The 2026-07-28 revision moves to a stateless core and adds Tasks and MCP Apps.
- **Implication:** Atlys ships an MCP server as its primary agent interface: local stdio first (no auth needed, the user's own process), then remote with OAuth, where Atlys grants map onto OAuth scopes.

## PDPP (Personal Data Portability Protocol)

- Protocol for user-controlled, purpose-bound access to personal data. Grants are expressed as OAuth Rich Authorization Requests (RFC 9396), and sources are either platform-native or bridged by "polyfill" connectors through the JSONL **Collection Profile**.
- Developed at **PDP-Connect, a lab at LF Decentralized Trust** (Linux Foundation). Principles v1.0 were published Sept 2026. The core spec is a normative draft.
- **Implication:** the most direct candidate for Atlys's access and connector protocol. See [pdpp-and-dataconnect.md](pdpp-and-dataconnect.md).

## Solid (Social Linked Data)

- Decentralized "pods" in which users store data and grant apps access. Built on RDF/Linked Data, WebID, and Solid-OIDC.
- Standardization moved to the W3C **Linked Web Storage Working Group** (chartered Sept 2024), which treats the Solid Protocol as an *input*. The **Open Data Institute** has stewarded the Solid project since Oct 2024.
- **Takeaways:** the access-control and "apps come to the data" ideas match Atlys exactly. The RDF learning curve and lack of an ingestion story slowed adoption. Atlys should stay JSON-first and offer a Solid bridge later rather than build on top of it.

## Data Transfer Project / Data Transfer Initiative (DTI)

- DTP: Apache-2.0 framework with **common data models** and adapters for direct service-to-service transfer.
- DTI (the nonprofit) works on a **trust model** and accreditation (a "badge") so that senders can safely transfer to receivers, plus **AI personal-data portability principles** (2025): users should be able to download and directly transfer personal data used to personalize AI services, excluding model weights and training data.
- **Implication:** reuse DTP data models as ADM inputs. Long term, Atlys could become an *accredited destination* so users can transfer data straight into their vault.

## AT Protocol (Bluesky)

- Each user's data lives in a signed **repository** on a **Personal Data Server (PDS)** that can be self-hosted and migrated without the old host's cooperation.
- **Takeaways:** a good model for *portable, signed, user-controlled repositories* and account migration. AT Proto data is public by design (social), so it is not suited to private data, but its repo and migration design is worth studying for vault sync.

## MyData

- Nonprofit framework (MyData Global) defining **human-centric personal data** principles and "MyData operators" (infrastructure intermediaries with interoperability at technical, informational, and governance levels).
- **Takeaways:** a governance and principles reference. Possible operator certification if a managed service launches in the EU.

## Domain data standards to map to

| Domain | Standard |
| --- | --- |
| Health & fitness | **Open mHealth** schemas; **HL7 FHIR** (Observation, etc.); Apple HealthKit and Android Health Connect type vocabularies |
| Calendar | iCalendar (RFC 5545) / JSCalendar (RFC 8984) |
| Contacts | vCard (RFC 6350) / JSContact (RFC 9553) |
| Social activity | W3C Activity Streams 2.0 |
| General entities | schema.org |
| Location | GeoJSON (RFC 7946) |
| Consent records | Kantara Consent Receipt spec; ISO/IEC TS 27560 (consent record information structure) |
| Policy / permissions | W3C ODRL (for expressing usage policies on shared data) |
| Credentials (future) | W3C Verifiable Credentials 2.0 (for "proof without disclosure" ideas) |
