# v0: account setup and vault security

In v0 an "account" is a **local vault** on the phone. There is no email, no sign-up, and no server. The design still produces the same key structure that sync and the managed service will use later, so nothing has to be re-encrypted when they arrive.

## Onboarding flow

```
Welcome ─▶ How Atlys works ─▶ Create master password ─▶ Confirm password
   ─▶ Recovery key (show → save → confirm) ─▶ Face ID (optional)
   ─▶ Choose data sources ─▶ Permission prompts ─▶ Today
```

| Step | What the user sees | Notes |
| --- | --- | --- |
| Welcome | One sentence on what Atlys is | No sign-up, no email |
| How it works | Three short cards: *your data stays on this phone* · *encrypted with a password only you know* · *if you lose the password and recovery key, nobody can recover your data* | Honest about the trade-off up front |
| Master password | Password field with a strength meter and guidance (a passphrase is encouraged) | Minimum strength enforced by the core (zxcvbn score ≥ 3, length ≥ 12). Never logged, never stored. |
| Confirm | Re-enter the password | |
| Recovery key | A 24-word recovery phrase, plus options to copy, save to Files, or print | The user must confirm three random words. The app warns against screenshots and photo libraries. |
| Face ID | "Unlock with Face ID" toggle | Optional. Master password is always the fallback. |
| Data sources | Cards for Health, Location, Motion & Activity, Device | Each explains what is collected and why, with an individual toggle (details in [02-data-sources.md](02-data-sources.md)) |
| Permissions | iOS system prompts, one per enabled source | The app pre-explains each prompt before triggering it |

**Target: under 2 minutes.**

## Key hierarchy (v0)

```
master password ─Argon2id(salt, params)─▶ master key ─┐
                                                       ├─ unwraps ─▶ VAULT KEY (random 256-bit)
recovery phrase (256-bit) ─HKDF─▶ recovery key ───────┤
Secure Enclave key (Face ID) ─ECDH+HKDF─▶ ────────────┘
                                                         │ HKDF with distinct labels
              ┌──────────────────────────┬───────────────┼──────────────────────┐
              ▼                          ▼               ▼                      ▼
     SQLCipher raw key          blob encryption key   header MAC key   ingest private key (X25519,
     (vault.db)                 (blobs/)              (vault header)   stored wrapped by vault key)

ingest PUBLIC key (stored in plaintext) ──▶ used by background collectors to seal new data
```

- **The vault key is random** and never derived from the password, so changing the password only re-wraps one key.
- **Envelopes:** each unlock method (password, recovery, Face ID) is an AEAD-encrypted copy of the vault key. Future methods (passkey PRF, server-side OPAQUE login, trusted-contact recovery) are just more envelopes.
- **Derived keys** come from HKDF-SHA256 over the vault key with fixed, versioned labels (for example `atlys/v1/sqlcipher`), so each purpose has its own key.
- **The ingest keypair** lets background code *encrypt* new data without being able to *read* anything. See [03-ios-app.md](03-ios-app.md#the-write-only-ingest-inbox).

### Parameters

| Item | v0 value | Notes |
| --- | --- | --- |
| KDF | Argon2id, **m = 256 MiB, t = 3, p = 1** (proposed in M0; confirm on the oldest supported iPhone) | M0 found the original starting point (64 MiB, t = 3) takes only ~70 ms on current Apple silicon, far below the ~1 s target, so memory cost goes up first. Parameters are fixed rather than calibrated per device, so a vault restored onto an older phone still unlocks in reasonable time. Headers below the OWASP minimum (19 MiB, t = 2, p = 1) or above sanity caps are rejected. Parameters are stored in the header so they can be raised later. See [spike-report.md](spike-report.md#06-argon2id-calibration). |
| Salt | 16 random bytes per vault | |
| AEAD | XChaCha20-Poly1305 | Envelopes, blobs, and inbox payloads |
| Ingest sealing | HPKE (RFC 9180), X25519 + HKDF-SHA256 + ChaCha20-Poly1305 | Done in the Rust core so it is one implementation on every platform |
| Recovery phrase | 256 bits of entropy shown as 24 words from the BIP-39 English wordlist | High entropy, so HKDF suffices (no Argon2 needed) |
| Face ID envelope | Secure Enclave P-256 key (access control `biometryCurrentSet`, `ThisDeviceOnly`). Swift performs only the ECDH; Rust derives a KEK with HKDF-SHA256 and wraps the vault key with XChaCha20-Poly1305 | Invalidated automatically if biometrics change; never included in backups. The vault key never enters Swift. See [spike-report.md](spike-report.md#07-secure-enclave-wrap-of-the-vault-key). |
| Memory hygiene | Keys held only in Rust, zeroized on lock (`zeroize`) | Swift never sees raw key bytes (for Face ID it sees only an ECDH shared secret that is useless without the envelope) |

## Files on disk

| File | Contents | Encrypted? | iOS file protection |
| --- | --- | --- | --- |
| `vault/header.json` | Format version, vault ID, KDF params and salt, wrapped-key envelopes, ingest public key, header MAC | Envelopes encrypted; the rest is non-secret | Complete Until First User Authentication |
| `vault/vault.db` | SQLCipher database: records, sources, collector state, audit log | Yes (SQLCipher, raw key) | Complete |
| `vault/blobs/` | Large attachments (unused in v0, reserved) | Yes | Complete |
| `inbox/*.sealed` | New data from collectors, waiting to be merged | Yes (HPKE to the ingest public key) | Complete Until First User Authentication, so background collection works while the phone is locked |
| Keychain | Secure Enclave key reference for Face ID | Hardware-protected | `ThisDeviceOnly` |

The header MAC (keyed from the vault key) detects tampering such as a parameter downgrade after unlock. Before unlock, the AEAD envelopes already reject tampered ciphertext.

## Lock and unlock

- **Unlock:** Face ID, or the master password (Argon2id) → unwrap vault key → open SQLCipher → merge the inbox.
- **Auto-lock:** after a configurable time in the background (default 5 minutes) and on device lock. Locking zeroizes keys and closes the database.
- **App switcher:** the screen is blurred when the app goes to the background.
- **Failed attempts:** a growing delay after repeated wrong passwords (the main defense is still Argon2id cost).

## Account maintenance

| Action | How |
| --- | --- |
| Change master password | Requires an unlocked vault. Derive a new master key and re-wrap the vault key. No data re-encryption. |
| Regenerate recovery phrase | Requires an unlocked vault. Replaces the recovery envelope. The old phrase stops working. |
| Forgot password | Enter the recovery phrase → set a new master password |
| Lost both | **Data is unrecoverable.** That is the zero-knowledge promise, stated plainly during onboarding. |
| Device backup / restore | The iOS backup contains the encrypted vault. After a restore, unlock with the master password and re-enable Face ID (the Secure Enclave key doesn't transfer). |
| Encrypted export | Export `.atlysvault` (header + database + blobs) through the share sheet. Opens anywhere with the master password or recovery phrase. |
| Plaintext export | JSON export of records for the user's own use, behind a clear warning |
| Delete vault | Requires the master password or Face ID. Wipes all files and keychain items. |

## Forward compatibility

- **Managed account later:** adds an OPAQUE registration and a server copy of the envelopes, all ciphertext. The local vault key doesn't change.
- **Passkeys later:** add a passkey-PRF envelope.
- **Multiple devices later:** each device gets a device keypair and an envelope. Data moves as the encrypted change log described in the managed-cloud doc.

## Threats considered for v0

| Threat | Mitigation |
| --- | --- |
| Stolen phone, phone unlocked, app locked | Vault requires Face ID or the master password. Keys aren't in memory while locked. |
| Stolen phone, forensic extraction of the file system | SQLCipher + iOS Data Protection; inbox is sealed to a key only the vault holds |
| Weak master password | Strength enforcement + Argon2id; recovery phrase is high entropy |
| Malicious backup file | AEAD envelopes and header MAC reject tampering |
| Shoulder surfing / screenshots of recovery phrase | Warnings; recovery screen marked as sensitive (hidden in screen recordings where iOS allows) |
| Supply chain | No third-party Swift SDKs; Rust dependencies pinned and audited (`cargo deny`, `cargo audit`) |
