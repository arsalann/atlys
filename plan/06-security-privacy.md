# 06 — Security & privacy (draft)

Security is the product. The bar is "would a password-manager security team sign off on this?"

## Threat model (initial)

| Adversary | Goal | Primary mitigation |
| --- | --- | --- |
| Device thief / lost phone | Read vault | Encryption at rest; key derived from master password; OS secure enclave for unlock; auto-lock |
| Storage/hosting provider (BYO or managed) | Read or tamper with data | Client-side E2EE; authenticated encryption; server only stores ciphertext |
| Malicious or over-reaching AI agent / app | Exfiltrate more data than needed | Scoped, time-boxed grants; aggregation/redaction transforms; audit log; rate limits |
| Prompt injection via ingested content (e.g. a malicious email in the vault) | Make the agent leak data or take actions | Treat vault content as untrusted data; read-only by default; no write or outbound tools in the default MCP profile; tool output tagging |
| Malicious connector (supply chain) | Steal tokens or data | Connector review and certification; declared network allow-list enforced by host; signed releases; least-privilege OAuth scopes |
| Project maintainers / managed service operator | Access user data | Zero-knowledge design; reproducible builds; open source server |
| Network attacker | Intercept sync | TLS plus E2EE payloads |

## Key hierarchy (Bitwarden-style, to be specified)

```
master password ──Argon2id(salt)──▶ master key ──▶ unwraps ──▶ vault key (random, 256-bit)
                                                                │
                                   per-collection / per-record data keys ◀─┘
recovery key (printed / stored offline) ──▶ also unwraps vault key
device keys (Secure Enclave / StrongBox / TPM) ──▶ biometric quick-unlock
```

- The master password never leaves the device. The managed service authenticates with **OPAQUE** (RFC 9807), so it never receives the password or a password-equivalent hash. Passkeys with the WebAuthn PRF extension can unwrap the vault key for quick unlock.
- The random vault key means changing the password re-wraps one key, not the whole vault.
- Per-scope data keys make **selective sharing** possible later: share a sub-key for one collection with another vault or app.
- A **recovery key** is mandatory at setup. There is no "forgot password" backdoor, by design.
- Primitives: Argon2id (memory-hard KDF), XChaCha20-Poly1305 or AES-256-GCM (AEAD), X25519/Ed25519 for sharing and signing. Use well-reviewed libraries only. **No custom crypto.**

## Storage modes and what each party can see

| Mode | Data at rest | Who can decrypt | Metadata leaked to host |
| --- | --- | --- | --- |
| Local | Encrypted file on device | User only | None |
| BYO storage | Encrypted blobs in user's bucket/DB | User only | Object count/sizes/timestamps |
| Managed | Encrypted blobs on project infra | User only | Account email, sizes, timestamps |

Minimize metadata: pad blob sizes, batch writes, and use opaque object names.

## Access-layer safeguards for agents

- The default MCP profile is **read-only**, scoped to selected namespaces and time ranges.
- Sensitive namespaces (`health`, `comms`, precise `location`) are off by default and need explicit opt-in per grant.
- Location is coarsened by default (city level) unless precise access is granted.
- Every tool call is logged: client, grant, query, and record count returned.
- A "local-only agent" mode: when the agent's model runs locally, nothing leaves the device at all. When the model is hosted (for example, a cloud LLM), the UI states plainly that query results are sent to that provider.

## Project-level security practices

- `SECURITY.md` with a private vulnerability reporting channel (GitHub private advisories).
- Secret scanning plus push protection on the repo, a gitleaks pre-commit hook, and CI checks.
- Dependabot/Renovate; signed commits for maintainers; SLSA provenance on release artifacts; reproducible builds for apps where feasible.
- An independent cryptography audit before any "1.0" or managed-service launch.
- No analytics or third-party tracking SDKs in any client. Crash reporting is opt-in only and scrubbed.
- Test data is always synthetic. Real personal data never goes in the repo, issues, or logs.
