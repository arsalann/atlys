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

## Known portability gaps

Cases where users can *see* their own data but cannot get a usable copy. They are tracked here because they shape the roadmap and are useful evidence for regulators and portability advocates.

### iOS Screen Time (as of 2026-10)

- **What users can do:** view per-app and per-website usage in Settings → Screen Time.
- **What they can't do (outside the EU):** export it, or authorize another app (such as Atlys) to read it. The standard Screen Time API only displays data inside a sandboxed extension; Apple has described the export block as intended behavior.
- **EU exception:** developers report an iOS 26.4+ entitlement (`com.apple.developer.family-controls.app-and-website-usage`) that lets an approved app read usage data, for customers with an EU device and an EU Apple Account. Only one app at a time can hold this access. *Not yet confirmed against Apple's documentation; verify before relying on it.*
- **Why it's likely lawful elsewhere:**
  - GDPR and CCPA access rights apply to data a company holds as controller. Apple states Screen Time is processed on device and synced end-to-end encrypted, so it arguably holds nothing to return. Whether a device maker is a controller for on-device processing is unsettled.
  - The EU Data Act's connected-product rights exclude devices primarily designed to display, record, or transmit content. The legislative drafts list smartphones, tablets, and PCs as examples (recital 15; confirm the final text).
  - No US law requires an operating system to provide export tools or developer APIs.
- **Where the law pushes back:** iOS is a DMA core platform service. DMA Art. 6(9) requires gatekeepers to give users and authorized third parties free, effective, continuous, real-time portability of data generated through the user's activity. Usage logs fit that description, and EU portability guidance treats observed data such as activity logs as "provided by" the user. The EU-only availability of the iOS 26.4 access suggests DMA compliance (inference, not confirmed by Apple or the Commission). Whether a one-app-at-a-time limit is "effective" portability is an open question.
- **Atlys response:**
  1. v0: no screen time on iOS; documented in [../v0/02-data-sources.md](../v0/02-data-sources.md#screen-time-and-app-usage).
  2. v0.1: the EU entitlement path, after confirming it with Apple.
  3. Phase 2: a Mac connector that reads the Mac's Screen Time stores (including synced iPhone usage), plus full support on Android.
  4. Ongoing: send GDPR access and portability requests for Screen Time data and record the responses (see the "portability gaps tracker" idea in [../10-ideas-backlog.md](../10-ideas-backlog.md)).

