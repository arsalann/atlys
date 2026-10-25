# 04 — Data sources

The set of possible sources is unbounded. The strategy is to start with sources that are **local, high-signal, and need no third-party approval**, prove the full pipeline end to end, then grow through community-contributed connectors.

## Prioritization criteria

1. **Access friction:** on-device or a user-provided export is lowest. Partner approval or app review is highest.
2. **Signal for a personal assistant:** does it answer real questions ("when did I last…", "how have I been sleeping", "where was I")?
3. **Format stability:** documented APIs and export formats beat scraped HTML.
4. **Legal/ToS risk:** prefer official export or portability mechanisms over scraping.
5. **Contributor appeal:** popular services attract maintainers.

## Phase 1: device and self-generated data

| Source | Platform | Method | ADM types |
| --- | --- | --- | --- |
| Location (GPS, visits) | iOS, Android | On-device | `location.point`, `location.visit`, `location.trip` |
| Health & fitness | iOS HealthKit, Android Health Connect | On-device | `health.*` (steps, heart rate, sleep, workouts, weight…) |
| Screen time / app usage | Android UsageStats; iOS (limited, see note) | On-device | `device.app_session`, `device.screen_session` |
| Device telemetry | Both | On-device | `device.battery`, `device.connectivity`, `device.motion_activity` |
| Calendar | Both (EventKit, CalendarProvider) | On-device | `calendar.event` |
| Contacts | Both | On-device | `person` |
| Photos metadata (not pixels, by default) | Both | On-device | `media.photo` (time, place, people tags) |
| Manual counters / journal | All | Manual | `custom.counter`, `note` |

> **iOS note:** exportable per-app screen time on iOS is limited to EU users on iOS 26.4+ with Apple's app-and-website-usage entitlement. Elsewhere, Screen Time data can only be displayed, not stored. A Mac connector can read synced iPhone usage, and Android exposes it fully. Health and location are well supported on both. The v0 iOS collectors are detailed in [v0/02-data-sources.md](v0/02-data-sources.md).

## Phase 2: exports ("takeout") and portability APIs

These cover the most data with the least partner friction.

| Source | Method | Notes |
| --- | --- | --- |
| Google Takeout | Export import | Location History/Timeline (now on-device only for many users), Chrome history, YouTube, Gmail (mbox), Fit, Maps |
| Google Data Portability API | API | DMA-driven, OAuth-based, recurring access to many Google products for users in the EEA |
| Apple privacy export (privacy.apple.com) | Export import | Account, App Store, iCloud metadata |
| Meta (Facebook / Instagram / WhatsApp) | Export import | JSON "Download Your Information" |
| X / Twitter archive | Export import | Long-standing format |
| Spotify | API + extended streaming history export | Rich listening history |
| Amazon | Export import | Orders, Alexa, Kindle |
| Netflix / YouTube watch history | Export import | Viewing history |
| Reddit | Export import | GDPR export |
| LinkedIn | Export import | Profile, connections, messages |

## Phase 3: APIs with OAuth

| Category | Sources |
| --- | --- |
| Wearables | Oura, Whoop, Garmin, Fitbit/Google, Withings, Strava (consider wrapping or partnering with Open Wearables) |
| Productivity | GitHub, Google Calendar, Notion, Todoist, Linear |
| Communication | Gmail / IMAP, Slack export, Telegram export, Signal (local backup) |
| Finance | Open Banking (EU PSD2, UK OB), Plaid/SimpleFIN (US), bank CSV/OFX import |
| Media | Goodreads, Letterboxd, Last.fm, Pocket, Kindle highlights |
| Browser | Chrome/Firefox/Safari local history DB (desktop) |

## Phase 4: data requests and "delete at source"

For services with no API or export button:

- Generate **GDPR Art. 15 / CCPA right-to-know** request letters from a community-maintained directory of company privacy contacts (similar to the datarequests.org approach).
- Track request status and deadlines (30 days under GDPR, 45 days under CCPA), and import the response once it arrives.
- **Deletion requests (Art. 17 / CCPA delete):** the successor to the legacy *Clean Google Activity* feature. Deletion only ever happens as an explicit, separate user action **after** a verified copy exists in the vault.
- Point Californians to the state **DROP** platform for data-broker deletion instead of duplicating it.

## Community connector model

- A connector **template** with generator, fixtures, and CI checks.
- A **registry** (`connectors/registry.json`) with metadata: maintainer handle, status (`alpha`/`beta`/`stable`/`deprecated`), last verified date, supported ADM types, and auth mode.
- **Certification levels:** `community` (merged, tests pass) → `verified` (reviewed for security and least privilege) → `core` (maintained by the core team).
- **Format-drift alerts:** exports change silently. Golden-file tests built from *synthetic* fixtures, plus opt-in, privacy-preserving parser error reports (error class only, never data), help catch breakage.
- **Codeowners per connector** so maintainers own review of their source.
