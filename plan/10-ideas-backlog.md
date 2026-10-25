# 10 — Ideas backlog

Brainstorm parking lot. Nothing here is committed. Ideas move into the roadmap through discussion.

## From the original vision

- **Insights:** dashboards and periodic reports over the vault ("your year in data", digital-wellness reports, commute patterns, sleep vs. screen time). Defined declaratively over the ADM, like the legacy JSON widget config.
- **Delete at source:** one-click deletion requests after the data is safely copied. Track compliance deadlines and escalate to the data protection authority if they are ignored.
- **Share securely:** share a scoped, time-boxed slice of your vault with a person, doctor, or app, P2P where possible. The legacy app did this over WebRTC for group fitness and family dashboards.
- **Sell / donate data:** a consented data marketplace or data donation for research (compare Vana data DAOs and Open Humans). High ethical and regulatory complexity, so it is explicitly out of the core and would need its own RFC.
- **AI agent:** a personal assistant grounded in the vault. Delivered as "bring your own agent" through MCP rather than a bundled chatbot.

## New ideas

- **Digital footprint inventory:** discover which services hold your data, from email receipts, password-manager exports, and OAuth grants, and build an `account.*` map with links to export/delete pages.
- **Privacy score / exposure report:** breach exposure (HIBP), data-broker presence, and app permissions, combined.
- **"Memory" for agents:** a writable `note.*` namespace where an agent stores what it learned about you, kept in *your* vault instead of the vendor's memory feature.
- **Portable AI context:** export your vault-derived profile as context for any assistant, matching DTI's push for AI personal-data portability.
- **Local LLM bundle:** an optional one-click local model (llama.cpp/Ollama) for a fully offline assistant.
- **Household / family vaults:** shared collections with per-member keys.
- **Legacy / digital inheritance:** a dead-man's-switch handover of selected collections to a trusted person.
- **Proof without disclosure:** zero-knowledge or attestation-style proofs ("over 18", "lives in region X", "ran 100 km this month") without sharing raw data.
- **Researcher mode:** IRB-friendly data donation flows with aggregate-only queries and differential privacy.
- **Company adoption kit:** "Sign in with Atlys"-style grant flow so a third-party app can request scoped access to a user's vault instead of collecting the data itself.
- **Browser extension:** capture browsing and reading history directly into the vault, similar to Promnesia or Memex.
- **Screen/audio memory connector:** import from screenpipe-style local recorders.
