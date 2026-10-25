# Language choice: Rust core, TypeScript connectors

Status: Recommendation (2026-10-23). Feeds open question #2 in [../09-open-questions.md](../09-open-questions.md).

## The question

Is Rust the right language for Atlys, given that the project must:

1. run an encrypted vault on iOS, Android, desktop, and servers;
2. call APIs and parse export files from many services;
3. run **browser automation** for services without an API or export button, for example opening Spotify's privacy page in a logged-in browser to request a data download;
4. let a large community add and maintain connectors.

## Short answer

**Not Rust alone. Use two languages, each for what it does best:**

| Layer | Language | Why |
| --- | --- | --- |
| Vault core: crypto, storage, sync, query, grant enforcement | **Rust** | One audited implementation on every platform, memory safety, no runtime needed |
| CLI, local MCP server, desktop shell (Tauri), sync server | **Rust** | Shares the core directly; official MCP Rust SDK (`rmcp`) is mature |
| Mobile apps | **Swift / Kotlin** shells over the Rust core (UniFFI) | OS data APIs (HealthKit, Health Connect) and background work need native code |
| **Connectors**: API clients, export parsers, browser automation | **TypeScript** (primary) | Best browser-automation ecosystem, runs on desktop *and* inside mobile WebViews, largest contributor pool, ~60 Apache-2.0 connectors already exist to build on |
| Other-language connectors (Python, Go, …) | Any | Allowed through the language-neutral JSONL process protocol, for wrapping existing tools such as HPI parsers |

The vault never trusts connectors. Connectors emit records, and the Rust core validates, encrypts, and stores them.

## Why not Rust for connectors

**Browser automation is a weak spot for Rust.**

| Library | Language | Status (Oct 2026) | Notes |
| --- | --- | --- | --- |
| **Playwright** | TS (official), Python, Java, .NET | ~98k★, Microsoft, very active | Industry standard: Chromium, Firefox, and WebKit; auto-waiting; network interception; tracing. **No official Rust binding.** |
| playwright-rust (`playwright-rs`) | Rust | ~160★, pre-1.0 | Unofficial; drives the Node Playwright driver over JSON-RPC, so Node is still required |
| chromiumoxide | Rust | ~1.4k★, last release Feb 2026 | Pure-Rust CDP client; Chromium only; small maintainer base |
| headless_chrome | Rust | ~3k★ | Puppeteer-like; Chromium only |
| Rustwright | Rust engine, Python/Node API | Alpha | Playwright API on a Rust CDP engine; Chromium only |
| Stagehand | TS (also Python, Go) | ~26k★ | AI-assisted browser actions (`act`, `extract`, `observe`) mixed with scripted steps |
| browser-use | Python | ~119k★ | Fully autonomous AI browser agent |

**Other reasons:**

- **Scrapers break constantly.** Sites change their HTML, login flows, and bot defenses. Connector code is short, gets rewritten often, and needs fast iteration. TypeScript is faster to write, review, and hot-fix than Rust.
- **Contributors.** Far more developers can write a TypeScript scraper than a Rust one. The project's growth depends on outside connector maintainers.
- **Mobile.** Playwright and headless Chrome **cannot run on iOS**, and they can't run inside a normal Android app either. On phones, scripted collection has to run as JavaScript injected into an in-app WebView (`WKWebView` / Android `WebView`). The legacy Atlys "Clean Google Activity" feature worked exactly this way. A TypeScript connector can share logic between a Playwright run on desktop and a WebView run on mobile. A Rust connector can't run in a WebView.
- **AI-assisted scraping.** Self-healing scrapers that use an LLM to find the "Request data" button after a redesign (Stagehand, browser-use) exist in TypeScript and Python, not Rust.
- **Reuse.** The PDP-Connect connector library (Apache-2.0, TypeScript, ~60 sources including Spotify, Google Takeout, Apple Health, Oura, Strava, Netflix, and Amazon) uses this exact model. Its Spotify connector shares code between a desktop browser run and a mobile WebView "pageshim". See [pdpp-and-dataconnect.md](pdpp-and-dataconnect.md).

## Why not TypeScript for everything

- **Mobile background work.** Health and location collectors run in the background on phones. Shipping a JS engine for background work is fragile and battery-hungry, and iOS restricts it.
- **Crypto on every platform.** The vault's encryption must be identical on iOS, Android, desktop, web (WASM), and servers. A Rust core compiled to each target gives one implementation to audit. Bitwarden's SDK is a Rust core with UniFFI mobile bindings, and 1Password reportedly uses a shared Rust core in the same way.
- **Memory safety and supply chain.** The core handles keys and decrypted personal data. Rust has a smaller attack surface than a large npm dependency tree.
- **Distribution.** A single static binary for the CLI and MCP server, with no Node install required for users who only import files.

## Alternatives considered

| Option | Verdict |
| --- | --- |
| **Rust everywhere** | Strong core, weak connector story (above) |
| **TypeScript everywhere** (Node/Electron + React Native) | Fastest start, weaker security and mobile background story. Vana's DataConnect is mostly TS but still uses a Rust (Tauri) shell. |
| **Go core + Go connectors** (like Timelinize) | Good CLI/server language with an OK browser library (chromedp), but gomobile is weaker than UniFFI for iOS/Android, and there's no WebView reuse story |
| **Python connectors** | Great for export parsing (HPI) and AI agents (browser-use), poor for desktop/mobile distribution. Support it through the JSONL protocol, but don't make it primary. |
| **Kotlin Multiplatform core** | Good for mobile sharing, weaker for CLI/server/WASM and crypto library choices |

## How browser connectors should run

### Desktop (Tauri app / CLI)

- The Rust host launches a **visible** browser (the user's installed Chrome/Edge, or bundled Chromium) with a dedicated Atlys profile.
- It hands the connector process a CDP WebSocket endpoint, the same approach PDPP takes. The connector drives it with Playwright, `playwright-core` `connectOverCDP`, or any CDP client.
- **The user logs in themselves.** Atlys never asks for or stores service passwords. 2FA and CAPTCHAs are handled by the user in the visible window. The connector waits, then continues.
- Connectors declare allowed hosts. The host blocks other navigation and network calls where possible.

### Mobile (iOS / Android)

- An in-app WebView opens the service's login page. The user signs in, then the connector's TypeScript bundle is injected to click through the export flow or read the authenticated pages.
- Records go to the native layer, then into the Rust vault.

## Many "scraping" connectors are really two-phase export requests

Spotify is a good example. The richest data (full streaming history) is not available through the API. It comes from the privacy page's **"Download your data"** request, which arrives by email days later.

1. **Request:** a browser-automation step opens `spotify.com/account/privacy` in the user's logged-in session and submits the request, or guides the user to do it.
2. **Wait:** Atlys records the pending request with its expected timeline in `privacy.data_request`.
3. **Import:** when the email arrives, the user drops in the ZIP (or Atlys fetches the download link if the user has connected their mailbox), and an export parser turns it into records.
4. **Live top-up:** a separate API or web-session connector fills in recent data between exports.

The same pattern covers Google Takeout, Meta, Apple, Amazon, and most GDPR/CCPA exports. Browser automation is mostly the **request** step. The heavy lifting is the **parser**, which is plain TypeScript (or Python) with no browser at all.

## Scraping guardrails (to put in the connector guidelines)

- Prefer, in order: official API → official export → user's own authenticated web session → anything else.
- Only collect the user's **own** data from their **own** account, at human-like rates.
- Never store passwords. Store session cookies only in the encrypted vault, and only if the user opts in to unattended refresh.
- Bot-detection evasion beyond normal browser behavior is out of scope.
- Every connector records its terms-of-service and legal notes in its manifest.

## Decision proposal

- **Adopt:** Rust core + TypeScript connector SDK + language-neutral JSONL connector protocol.
- **First spike (one to two weeks):**
  1. A Rust crate that creates and unlocks an encrypted vault and writes and reads records.
  2. A TypeScript connector that parses a synthetic Spotify export ZIP and emits records over JSONL.
  3. A TypeScript browser connector that connects to a Rust-launched Chromium over CDP and completes a login handoff on a test site.
  4. A Rust MCP server (`rmcp`) that answers queries over the vault.

If the spike shows that Rust-to-TypeScript plumbing is too heavy for an early project, the fallback is a TypeScript-first Phase 1 with the vault core ported to Rust before any mobile work. The connector protocol stays the same either way, so connectors would not need rewriting.
