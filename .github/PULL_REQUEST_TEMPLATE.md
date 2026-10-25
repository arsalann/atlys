## What and why

<!-- What does this change, and why? Link issues, RFCs, or plan docs. -->

## How it was tested

<!-- Commands run, simulator or device, OS versions. Say what was NOT tested. -->

## Checklist

- [ ] **No secrets:** no API keys, tokens, passwords, private keys, signing files, `.env` files, `Local.xcconfig`, or hard-coded server addresses
- [ ] **No real personal data:** all fixtures, screenshots, and logs are synthetic; no real names, emails, locations, health data, or account IDs
- [ ] No logs, database files, data exports, build artifacts, or binaries committed
- [ ] I reviewed the staged diff (`git diff --cached`) before committing
- [ ] No network access added to the app (no `URLSession`, `Network` connections, or WebKit)
- [ ] No third-party Swift packages added; any new Rust crate passes `cargo deny` and is justified above
- [ ] Commits follow Conventional Commits and are signed off (`git commit -s`)
- [ ] Tests added or updated, and CI passes
- [ ] Docs in `plan/` or `spec/` updated if behavior or design changed
