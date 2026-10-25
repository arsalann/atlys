# Regulation relevant to Atlys

*Not legal advice. A summary to inform product design.*

## Rights that power ingestion

| Law | Right | How Atlys uses it |
| --- | --- | --- |
| **GDPR** (EU/EEA, UK GDPR) | Art. 15 access; Art. 20 portability (structured, machine-readable); 1-month response (extendable) | DSAR letter generator + tracking; import responses |
| **GDPR** | Art. 17 erasure | "Delete at source" requests after copying |
| **CCPA/CPRA** (California) | Right to know, delete, correct, opt out of sale/sharing; 45-day response; **authorized agents** allowed | Request generation; possibly act as an authorized agent later (Permission Slip precedent) |
| **California Delete Act / DROP** | One request deletes data held by all registered data brokers. Platform live **Jan 1, 2026**; brokers must check at least every 45 days and process within 90 days from **Aug 1, 2026** | Link out to DROP rather than duplicate it; record the request in `privacy.*` |
| **EU Digital Markets Act Art. 6(9)** | Gatekeepers must provide **free, continuous, real-time** portability to end users and authorized third parties (for example, the Google Data Portability API, up to 180-day authorizations) | API connectors for gatekeeper platforms (Google, Meta, Apple, Microsoft, Amazon, ByteDance) for EEA users |
| **EU Data Act** (applies from Sept 2025) | Users of connected products (IoT, wearables, cars) can access the data they generate and share it with third parties | Future connectors for connected devices and vehicles |
| Other US state privacy laws (VA, CO, CT, TX, OR…) | Access/delete rights similar to CCPA | Same DSAR flow with per-state templates |

## Obligations if Atlys runs services

- **Local and BYO modes:** Atlys software processes data on the user's own device. The project is not a controller or processor of that data, which is a strong position and one more reason to keep these modes the default.
- **Managed service:** stores only ciphertext. Zero-knowledge design reduces, but does not eliminate, obligations: account data, billing, DPA/processor terms, breach notification, and data residency choices (EU region).
- **Health data:** in the US, HIPAA generally does not apply to consumer-directed apps the user controls. The FTC Health Breach Notification Rule *may* apply to personal health record vendors. In the EU, health data is a GDPR Art. 9 special category, which is another reason for zero-knowledge design.
- **Authorized-agent role** (sending requests on users' behalf) creates obligations: identity verification and records. Start with *user-sent* requests that Atlys drafts and tracks.
- **App store rules:** Apple and Google restrict use of HealthKit/Health Connect data (no advertising, no sale, purpose limits). The vault must respect the store policies attached to data that came through those APIs, especially for the sharing and selling ideas.

## Design implications

1. Keep the default architecture **controller-free**: everything stays local and user-held.
2. Tag records with **legal-origin metadata** (for example, "came from HealthKit") so policy restrictions can be enforced on sharing.
3. Maintain a community **directory of company privacy contacts and DSAR procedures** as an open dataset, similar to datarequests.org.
4. "Sell your data" features need separate legal review (consumer protection, data-broker registration laws, app store policy).
