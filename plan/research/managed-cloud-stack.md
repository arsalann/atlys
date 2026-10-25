# Managed cloud stack

Status: Recommendation (2026-10-25). Not built in v1, but decided now so the clients and the vault format are designed for it. Feeds open question #16 in [../09-open-questions.md](../09-open-questions.md).

## The key insight: the server is not a database of user data

In a zero-knowledge design the managed service **never sees plaintext**. It cannot query, index, search, or summarize anyone's records. So the "cloud database" is really three things:

1. **Encrypted object storage**: opaque ciphertext segments (the change log and blobs).
2. **A small metadata database**: accounts, devices, wrapped (encrypted) keys, sync cursors, object pointers, billing status.
3. **A thin API server**: authentication, sync ordering, notifications, quotas, and a relay for agent access.

Everything clever happens on the user's devices. This is the same architecture used by Bitwarden, Ente (Go server + Postgres + S3-compatible storage), and Proton.

## Recommended stack

| Concern | Choice | Why | Self-host equivalent |
| --- | --- | --- | --- |
| **API server** | **Rust** (`axum` + `tokio`), one stateless binary, OCI image | Shares types and crypto with the vault core; same language decision | Same binary |
| **Metadata DB** | **PostgreSQL** (through `sqlx`) | Boring, reliable, available as a managed service everywhere (RDS, Cloud SQL, Neon, Supabase, Scaleway, OVH…), strong transactional ordering for the sync log | **SQLite** in the same binary for single-user or family self-hosting, as Vaultwarden does |
| **Ciphertext storage** | **S3-compatible object storage** (through the `object_store` crate) | Cheap, durable, provider-neutral | Local filesystem, or SeaweedFS (Apache-2.0) / Garage (AGPL) for S3 on your own hardware. Avoid MinIO: its community edition is no longer maintained. |
| **Login** | **OPAQUE** (RFC 9807, `opaque-ke` crate, audited by NCC Group for WhatsApp backups) | The server never receives the password *or a password-equivalent hash*, which is stronger than Bitwarden's double-hash login | Same |
| **Second factor / quick unlock** | **Passkeys (WebAuthn)**, with the **PRF extension** to unwrap the vault key | Phishing-resistant login; passkey unlock without typing the master password (Bitwarden's approach) | Same |
| **Real-time sync** | WebSocket or SSE "new data" pings; APNs/FCM pushes with **no content** | Devices fetch the actual ciphertext themselves | Same (push optional) |
| **Agent relay** | End-to-end encrypted relay (Noise protocol) between an agent and the user's own always-on device | See "Agents in managed mode" below | Same |
| **Billing** | Stripe (managed service only) | Holds only billing metadata, never vault data | n/a |
| **Observability** | OpenTelemetry metrics and traces; **no request bodies or content in logs** | Operational visibility without data exposure | Same |
| **Deployment** | Containers on any provider; no proprietary cloud services in the critical path | Anyone can run the same stack, including the project itself | Docker Compose: server + Postgres/SQLite + storage |
| **Hosting region** | EU first (GDPR), US second; data residency chosen per account | Matches the privacy positioning | User's choice |

**Rejected or deferred:**

- **A hosted SQL or document DB holding user records** (Firestore, DynamoDB, Supabase rows with plaintext): breaks zero knowledge.
- **CRDT sync services** (Ditto, Realm Sync, ElectricSQL): plaintext on the server, or vendor lock-in. Realm Device Sync was shut down in Sept 2025, which is a reminder of the lock-in risk.
- **Proprietary-only services** (DynamoDB, Firebase): would make the managed service impossible to self-host.
- **Blockchain or decentralized storage** (IPFS, Filecoin): unnecessary complexity for v1. Could be offered later as just another storage backend.

## What the server stores

```
accounts        id, email (or none), opaque_registration, plan, region, created_at
devices         id, account_id, public_key, name, last_seen
key_envelopes   account_id, kind (master|recovery|passkey_prf|device), wrapped_vault_key  ← ciphertext
log_segments    account_id, seq (server-assigned), device_id, object_key, size, sha256, created_at
blobs           account_id, object_key, size, sha256, refcount
shares          id, from_account, to_account|link, wrapped_share_key, expires_at        ← later
grants_meta     id, account_id, client_id, expires_at, revoked                          ← no scope details in plaintext
audit           account-level security events only (logins, device adds), never data access
```

Object storage holds `log/<account>/<seq>` and `blob/<account>/<sha256>`, all ciphertext. Object names are opaque, and sizes are padded to reduce metadata leakage.

## Agents in managed mode: the hard part

A zero-knowledge server **cannot answer an AI agent's query**, because it can't decrypt anything. Options:

| Option | How it works | Trade-off |
| --- | --- | --- |
| **A. Device serves queries** (recommended for v1 of managed) | The agent connects to the managed relay. The relay forwards end-to-end encrypted requests to the user's own always-on device (desktop app, home server, or phone when awake). That device decrypts, enforces the grant, and answers. | True zero knowledge. Requires one of the user's devices to be online. |
| B. Self-hosted "home node" | The same as A, but the user runs an Atlys node on a home server or NAS | For technical users |
| C. Confidential computing node (later, opt-in) | Decryption happens inside an attested enclave (AWS Nitro Enclaves, confidential VMs), similar to Apple's Private Cloud Compute. The client verifies attestation before releasing a key. | Always available, but trust shifts to hardware vendors and attestation. Needs careful design and an audit. |

**Decision:** design for A now, keep C as a future opt-in. **Grant enforcement therefore always runs in the Rust core on the user's device**, never on the server.

## What this means for v1 (design now, build later)

1. **One portable format for everything that leaves the device.** Backups, BYO storage, device-to-device sync, and the managed service all use the same **encrypted change-log segments + encrypted content-addressed blobs**. The managed service is just one more storage target.
2. **Key hierarchy includes server-related envelopes from day one.** Master-password, recovery-key, device, and passkey-PRF envelopes all wrap the same random vault key, so adding a server later doesn't require re-encrypting anything.
3. **Ordering.** Every change carries a device ID, a ULID record ID, and a hybrid logical clock, so logs from multiple devices can be merged without a server. The server only adds a sequence number for convenience.
4. **A storage-target trait** in the core: `LocalFolder`, `S3`, `WebDav`, `AtlysServer`. v1 ships `LocalFolder` (which also covers iCloud Drive, Dropbox, and Syncthing folders).
5. **Grants evaluated on the device.** The access layer lives in the core and is exposed locally (stdio MCP) first. A relay later reuses exactly the same code path.
6. **No feature may require server-side plaintext.** Any feature proposal that needs the server to read data must go through an RFC and default to option C (attested, opt-in).
7. **Self-hostable by design.** `docker compose up` must run the full managed stack. The project's own hosted service is one deployment of the open source server, not a fork.

## Open items

- Pricing and quotas (storage-based, like Ente and Bitwarden) are a business decision, kept out of this public repo.
- Account recovery UX without a backdoor: recovery key, plus optional trusted-contact recovery using shared key envelopes.
- Abuse handling without seeing content: rate limits and quotas only.
- An independent audit before launch (crypto design + server).
