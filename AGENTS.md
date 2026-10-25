# AGENTS.md

Instructions for AI coding agents (and humans) working in this repository.

## 1. This repository is public

Atlys is an open source project and this repository is public. Everything committed here is visible to the world, may be cached or indexed, and is effectively permanent, even if it is deleted later.

**Never commit:**

- Secrets of any kind: API keys, tokens, passwords, private keys, signing keystores (`*.jks`, `*.keystore`, `*.p12`), service-account files (`google-services.json`, `GoogleService-Info.plist`), `.env` files, connection strings, or hard-coded server addresses.
- Personal information: real people's names, emails, phone numbers, addresses, account IDs, or any real personal data, including sample data exported from someone's actual accounts. Use synthetic fixtures instead.
- Private commentary: opinions, banter, internal discussion, or notes about individuals. Write documentation in a neutral, project voice.
- Logs, crash dumps, database files, data exports, build artifacts, and binaries (`*.apk`, `*.aab`, `*.ipa`).
- Anything else that should not be public.

Before every commit, review the staged diff (`git diff --cached`) for anything that breaks these rules. If something sensitive was committed by mistake, stop and flag it: deleting it in a later commit does not remove it from history, and the secret must be rotated.

Use `.context/` (gitignored) for scratch files, research downloads, and anything that must stay local.

## 2. Feedback and lessons learned go into this file

- When a maintainer gives feedback, a correction, or a preference about how work should be done, record it in the [Feedback & lessons learned](#4-feedback--lessons-learned) section below.
- When an agent makes a mistake or learns something that should change future behavior, record that lesson below as well.
- Write each entry as a short, general rule followed by the reason. Keep entries free of personal details (see section 1).

## 3. Changing this file needs approval

Before changing `AGENTS.md`, show the proposed change and ask a maintainer to review and approve it. If there is no response by the time the task is otherwise complete, go ahead and apply the change, and say clearly in the summary that it was applied without explicit approval.

## 4. Feedback & lessons learned

<!-- Format: - **Rule.** Why it matters. -->

- **Never commit credentials, signing keys, or service config files.** The pre-2021 Atlys codebase committed Android keystores, Firebase config, and hard-coded server IPs. That is unacceptable in a public repository.
- **Atlys (this project) is not Atlys the visa startup.** This project descends from Atlys Networks (founded 2019, personal data management). A separate, unrelated visa-application company founded in 2021 also uses the name "Atlys". Never mix up their history, funding, products, or news in docs or research. Verify which company a source refers to before citing it.
- **Check platform limits against the latest OS release before calling something impossible.** An early v0 draft said iOS screen time could not be exported, which missed the iOS 26.4 entitlement that allows it in the EU. Platform rules change every release, so state the version and region a limit applies to.

## 5. Project orientation

- `plan/`: vision, architecture, roadmap, and open questions. Read [`plan/README.md`](plan/README.md) first.
- `plan/research/`: landscape research on related projects, standards, and regulation.
- No application code exists yet. Follow the proposed monorepo layout in [`plan/03-architecture.md`](plan/03-architecture.md) when adding it.
- Decisions and their status are tracked in [`plan/09-open-questions.md`](plan/09-open-questions.md).
