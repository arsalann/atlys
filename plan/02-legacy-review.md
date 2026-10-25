# 02 — Legacy review (Atlys 2019–2020)

A review of the original, closed-source Atlys codebase, built by **Atlys Networks** (founded 2019). Atlys Networks is unrelated to Atlys, the visa-application startup founded in 2021. That code is **not** being imported into this repository. It is summarized here so its ideas and lessons carry forward.

## What existed

### Android app (Java, ~1,700 commits, 2019–2020)

- **Architecture:** three Gradle modules: `app` (UI), `data` (Realm DAOs, domain models, utilities), and `domain` (shared models).
- **Storage:** a local **Realm** database with an encryption key, plus encrypted backup/restore and import/export of the database as a ZIP.
- **On-device collectors (background workers):**
  - Location (Fused Location Provider) → stays, routes, commute and travel-mode detection
  - Google Fit history → steps, calories, heart rate, sleep, weight, workouts
  - Usage stats → screen time and per-app usage
  - Screen on/off broadcast receiver
- **Dashboard of widgets** defined in a JSON config: bar, pie, gauge, map, line, and indicator widgets. Examples include *Steps Summary*, *Screen Time Summary*, *Daily App Usage*, *Routes Map*, *Commute Summary*, *Sleep Summary*, *Work-Life Digital Balance*, *Digital Wellness Report*, and a custom *Thing Counter*.
- **Raw data browser** and dataset management: import, export, and per-table permissions.
- **Privacy tools:**
  - *Email breach check* against the Have I Been Pwned API
  - *Clean Google Activity*: a WebView flow that submitted a deletion request for Google Web & App Activity (an early "delete at source" feature)
  - *App Permission Report*: scanned installed apps and their granted permissions
  - A curated list of privacy links (opt-out pages, EFF guides, takeout pages)
- **Private Groups / Programs:** peer-to-peer data sharing over **WebRTC data channels**. Examples include group fitness challenges, family screen-time dashboards, health-vitals monitoring for loved ones, group digital detox, and green-commute challenges. The data went phone-to-phone and was not stored on a server.

### Admin backend (Node.js / Express / MongoDB, 2020)

- Organizations, departments, employees, and pseudonymous "contributions" keyed by a random key rather than identity. It was an aggregate-only reporting dashboard for organizations running a program (for example, workplace wellness or symptom check-ins).
- Standard Express hardening: helmet, rate limiting, input sanitization, JWT auth.

### Small utilities

- A standalone Android "Google Activity Cleaner" prototype, later folded into the main app.
- A JSON-to-CSV notebook for flattening exported data.

## What to carry forward

| Legacy idea | Relaunch form |
| --- | --- |
| On-device sensor collection (location, activity, screen time) | Phase 1 connectors: the easiest, highest-signal sources and fully local |
| Encrypted local database | Vault core, upgraded to modern, audited crypto (see [06](06-security-privacy.md)) |
| Raw data browser | Vault explorer in the desktop/web app |
| Widgets/insights from JSON config | Declarative "insight" definitions over the standard data model |
| Clean Google Activity | "Delete at source" action framework (DSAR/deletion requests) |
| HIBP breach check, app permission report | Privacy audit tools (Phase 2+) |
| WebRTC P2P Private Groups | Future: P2P selective sharing between vaults (see [10](10-ideas-backlog.md)) |
| Pseudonymous aggregate contributions | Future: consented data donation / aggregate programs |

## Lessons learned

1. **Secrets in the repo.** The legacy repos committed signing keystores, Firebase config files, and a hard-coded server IP. In the relaunch, secrets never go in the repo: enforce `.gitignore`, secret scanning in CI, and pre-commit hooks (now also a rule in `AGENTS.md`).
2. **Binaries in the repo.** APK/AAB builds and vendored JARs were committed. Release artifacts belong in CI-built GitHub Releases.
3. **Platform lock-in.** The app was Android-only, Java, and tied to Google Fit, which Google has since deprecated in favor of Health Connect. Build the core in a portable language and keep platform collectors thin.
4. **No schema contract.** Every data type was its own Realm class and summary table, so adding a source meant touching UI, DAO, and migrations. The relaunch needs a **source-agnostic data model** so connectors and consumers are decoupled.
5. **Insights before infrastructure.** Much effort went into charts and widgets. The relaunch should get ingestion, the vault, and the schema solid first, then let insights, and AI agents, sit on top.
6. **Telemetry in a privacy app.** Firebase Analytics was present, although disabled in places. In the relaunch: no third-party analytics SDKs in clients, ever.
