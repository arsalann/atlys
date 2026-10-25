# RFC-0002: Vault format and key hierarchy

Status: **Accepted** (2026-10-26, PR #6). This is the design step for M1.1.
The M0 spike does not implement the product format. Device calibration and
biometric tests remain open.

## Scope

Define the bytes needed to create and unlock a local vault across platforms:
header authentication, unlock envelopes, purpose-specific keys, and sealed inbox
files. Database records belong to RFC-0001; encrypted export packaging and blob
formats are reserved for later RFCs. No network protocol is introduced.

The terms MUST, MUST NOT, and SHOULD describe requirements for the M1
implementation. All randomness MUST come from the operating system CSPRNG.

## Keys and encodings

A vault has an independently random 16-byte vault ID, random 32-byte vault key,
and independently generated X25519 ingest keypair. Never derive the vault key
from a password. The ingest private key is wrapped, not derived from the vault
key. New vaults MUST NOT reuse IDs or keys.

Binary fields in JSON use lowercase hex, without a prefix. Reject odd lengths,
uppercase, invalid digits, and decoded lengths other than those specified.
Integers are unsigned decimal JSON integers; reject floats, negative values,
duplicate fields, unknown fields, and null where a value is required. Versions
are exact integer discriminants, not minimum compatibility levels.

HKDF means HKDF-SHA256 with a 32-byte output. Labels are exact ASCII, without a
terminating NUL. Absent salt means the RFC 5869 default zero salt.

| Input | Salt | HKDF info | Purpose |
| --- | --- | --- | --- |
| Vault key | absent | `atlys/v1/sqlcipher` | SQLCipher raw key |
| Vault key | absent | `atlys/v1/blob` | Reserved blob key; no blob writes in v1 |
| Vault key | absent | `atlys/v1/header-mac` | Header HMAC key |
| Vault key | absent | `atlys/v1/ingest-wrap` | Ingest private-key wrapping key |
| Recovery entropy (32 bytes) | absent | `atlys/v1/recovery` | Recovery KEK |
| P-256 ECDH secret (32 bytes) | ephemeral public key then enclave public key | `atlys/v1/envelope/secure-enclave` | Device KEK |

The recovery phrase encodes 256 random bits with the BIP-39 English wordlist and
checksum (24 words). Decode and validate that checksum, then use the original
32 entropy bytes. Do not use BIP-39's PBKDF2 wallet seed or a mnemonic passphrase.
A phrase replacement generates fresh entropy. Passwords use exact UTF-8 bytes,
without trimming, case folding, or Unicode normalization. Creation/change
requires at least 12 Unicode scalar values and zxcvbn score >= 3; unlocking
existing vaults MUST NOT apply a new password-strength policy.

## Header container

`vault/header.json` contains exactly two strings: `payload_hex` and `mac_hex`.
`payload_hex` encodes the exact UTF-8 bytes of an inner JSON object. This avoids
requiring a JSON canonicalization implementation: authenticate the decoded bytes,
never a reserialized object. Whitespace and field ordering inside the payload
are authenticated; whitespace and order in the outer container are immaterial.
Both JSON layers reject duplicate and unknown fields. Limit the complete file
to 64 KiB and JSON nesting to 8 before expensive processing.

The inner object contains exactly:

| Field | Shape |
| --- | --- |
| `format_version` | `1` |
| `vault_id` | 16-byte hex |
| `kdf` | object: `algorithm`: `"argon2id"`, `version`: `19`, `memory_kib`, `iterations`, `parallelism`, `salt` (16-byte hex) |
| `password_envelope` | 72-byte hex |
| `recovery_envelope` | 72-byte hex |
| `ingest_public_key` | 32-byte hex |
| `ingest_private_envelope` | 72-byte hex |

The MAC is HMAC-SHA256 under the derived header-MAC key over
`ASCII("atlys/v1/header") || 0x00 || exact_payload_bytes`. Store all 32 tag bytes.
Verify with a constant-time MAC verification API. The MAC authenticates every
inner field, including the public ingest key and KDF parameters, after an unlock
method yields a candidate vault key. Do not open the database, return a session,
or trust header metadata until MAC verification succeeds. Check that the
unwrapped ingest private key produces the authenticated public key.

Readers may inspect the unverified public key to seal background data while
locked. A filesystem attacker can replace that key and redirect future writes;
the MAC will detect this only at the next unlock. This design does not claim
pre-unlock integrity against filesystem modification. iOS file protection and
the app sandbox are the first defense. Stop collectors and report an integrity
failure if verification fails; do not silently create a replacement vault.

## Password KDF and resource policy

Use Argon2id version 0x13, 32 output bytes, no secret or associated-data parameter,
and a fresh 16-byte salt on creation and each password change. The creation
proposal is 262144 KiB, 3 iterations, 1 lane, pending oldest-device measurements.
The M0 `V0_DEFAULT` (65536 KiB) remains a benchmark baseline, not an approved
production default. Record the final creation policy before shipping onboarding.

For v1 imports/unlocks accept 19456..262144 KiB, 2..4 iterations, and exactly
1 lane. Reject other costs before allocating memory or running Argon2. Permit
only one KDF operation at a time, off the UI thread. These are deliberately
stricter than the spike's independent caps of 1 GiB / 64 passes / 16 lanes:
those caps in combination allow excessive unauthenticated work. Raising the
accepted policy requires device measurements and an RFC revision. A valid MAC
cannot protect resource use before the password has been derived.

Failed authentication returns one password-or-corruption error. UI retry delays
help accidental repeated attempts but are not a defense against offline attacks.

## Key envelopes

All wraps use XChaCha20-Poly1305 and serialize as
`nonce[24] || ciphertext[32] || tag[16]`. Generate a fresh nonce for every write.
AEAD decryption MUST verify the tag before releasing any plaintext.

Define `C = ASCII("atlys/v1/envelope") || 0x00 || vault_id[16]`.
The following AAD values are exact concatenations, without length prefixes:

| Wrap | KEK | Plaintext | AAD |
| --- | --- | --- | --- |
| Password | Argon2id output | Vault key | `C || "password" || 0x00 || BE32(memory_kib) || BE32(iterations) || BE32(parallelism) || salt[16]` |
| Recovery | Recovery HKDF output | Vault key | `C || "recovery"` |
| Ingest | Ingest-wrap subkey | Ingest private key | `C || "ingest"` |

Literal strings in this table are ASCII. Binding password parameters in AAD
rejects their substitution even before the whole-header MAC check. Replacing
one envelope requires recomputing the MAC; unrelated envelopes and the database
key remain unchanged.

### Device-local biometric envelope

Keep both the encrypted device envelope and the Secure Enclave key's opaque
representation in device-local Keychain items, outside the portable header.
This keeps them out of exported vaults and avoids modifying authenticated
portable metadata after a restore. Use a vault-ID-scoped item identifier and
`WhenUnlockedThisDeviceOnly`; the enclave private key uses
`privateKeyUsage` and `biometryCurrentSet`. There is at most one device envelope
per local vault. No private enclave key leaves the hardware.

The product envelope has version **2**, distinct from the M0 version 1:

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | Version = 2 |
| 1 | 16 | Vault ID |
| 17 | 65 | Ephemeral P-256 public key, X9.63 uncompressed |
| 82 | 65 | Enclave P-256 public key, X9.63 uncompressed |
| 147 | 72 | Wrapped vault key |

The KEK uses the ECDH/HKDF row above. AAD is `C || "device" || bytes[0..147)`.
The platform MUST validate both P-256 public points, match the stored enclave
public key to the local key, and enforce biometric authentication for unwrap.
Rust MUST check the vault ID and header MAC after unwrap. Swift only handles
the ECDH shared secret; it never receives vault, database, or ingest private keys.
The shared secret is sensitive: with the envelope it suffices to recover the
vault key, so keep its lifetime short and never cache or log it.

Missing/invalidated device keys fall back to the password or recovery phrase.
Enabling Face ID requires an unlocked vault. Disabling removes both local items.
Creation failure must leave password and recovery unlock working. Device-only
biometric enforcement and backup behavior require the M0 manual tests.

## Database and durable lifecycle

Use SQLCipher 4 compatibility, 4096-byte pages, raw 32-byte key mode, encrypted
page authentication, WAL, foreign keys, and secure deletion. Enable
`cipher_memory_security`; disable SQLCipher diagnostic logging of expected
wrong-key failures. Apply key and format settings before the first database
read. Check supported schema version before migration and reject newer schemas.
An unlock MUST open an existing database, never implicitly create an empty one
when a file is missing. Database schema version is separate from header version.

Creation writes a new sibling temporary vault directory, initializes and closes
the database, writes and syncs the authenticated header, then publishes the
directory atomically without overwriting an existing vault. Sync parent
directories where supported. Temporary creation directories are never treated
as valid vaults. Report failures rather than silently falling back to plaintext.

Header updates write a same-directory temporary file with exclusive creation,
sync it, atomically replace the header, then sync the directory. Serialize all
vault mutations. Do not report success until durability succeeds. A crash leaves
the old or new complete header. Password/recovery replacement only changes the
corresponding envelope (and KDF fields for a password), plus the MAC. Recovery
requires setting a new password before returning a normal unlocked session.

Lock cancels/drains in-flight core operations, closes SQLCipher, and drops all
Rust key buffers and plaintext caches. A checkpoint failure must not leave an
unlocked session available; release the connection even on error. Swift also
clears password/phrase UI state and obscures app-switcher snapshots. Zeroization
reduces exposure; it is not proof that compiler, OS, UI, or library copies never
existed. A process crash and device lock remain device-test scenarios.

Delete requires fresh password or biometric authentication. Stop collectors,
close the vault, remove device Keychain items, and delete vault, inbox, quarantine,
and interrupted-creation files. Report partial deletion and permit retry. Flash
storage erasure and deletion of old backups cannot be guaranteed. Password
changes/recovery regeneration do not revoke old backups or defeat rollback of a
complete old header: v1 has no external monotonic counter.

## Sealed inbox version 2

Version 2 binds the recipient vault ID. Version 1 is the throwaway M0 format;
product readers MUST reject it without guessing or trying fallback contexts.

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 8 | ASCII `ATLYSINB` |
| 8 | 1 | Version = 2 |
| 9 | 1 | Suite = 1 |
| 10 | 16 | Vault ID |
| 26 | 32 | HPKE encapsulated key |
| 58 | remaining | Ciphertext and 16-byte tag |

Suite 1 is RFC 9180 base mode, DHKEM(X25519, HKDF-SHA256), HKDF-SHA256,
ChaCha20-Poly1305 (KEM 0x0020, KDF 0x0001, AEAD 0x0003). Each file uses a fresh
sender context, one message at sequence number zero. HPKE info is
`ASCII("atlys/v2/inbox") || 0x00 || vault_id`; AAD is bytes `[0..26)`.
Reject a mismatched ID, unknown version, or suite before decrypting.

Plaintext is `BE32(json_length) || batch_json || zero_padding`. Pad the complete
plaintext to the smallest bucket that fits: 1, 4, 16, 64, 256 KiB, 1 MiB, then
whole MiB. Reject invalid lengths, nonzero padding, and non-bucket sizes. Batch
semantics are defined by RFC-0001; M0's draft records are not the final ADM.
Limit each complete file to 64 MiB, enforce that limit on writes before allocating
padding and on reads using a bounded reader (metadata size alone is insufficient).
Process one file at a time. Reject symlinks and nonregular files in managed paths.

Use `<ulid>.sealed`; names reveal approximate write times. Write a uniquely named
exclusive temporary file, sync, rename, and sync the parent. Ignore partial temp
files. HPKE base mode gives recipient confidentiality and ciphertext integrity,
not sender identity: anyone with the public key can submit batches. Treat all
opened records as untrusted input. Validate and deduplicate in one transaction;
delete the sealed file only after commit. Replaying a file must not duplicate
records. RFC-0001/M2 specifies durable record identities and merge cursors.

Move permanently malformed or unauthentic files to a same-container `quarantine/`
directory; retain ciphertext only and expose counts plus generic reason codes.
Do not quarantine transient I/O failures, device-lock failures, or files with a
newer unsupported version: leave them pending and report that an update is
needed. Quarantined files are never automatically retried or included in exports.
Use a combined inbox/quarantine budget of 256 MiB; when full, defer collection
with visible status instead of silently deleting unmerged data. Deleting rejected
files is an explicit user action. No record contents appear in logs.

## iOS storage and backups

Header, inbox, quarantine, and their directories use Complete Until First User
Authentication. Database, WAL/SHM, and reserved blobs use Complete protection.
Set protection before publishing files; verify on devices, not the Simulator.
Keep all data in the app's own container. Background collectors open only public
metadata and sealed inbox files, never SQLCipher or device unlock items.

Portable backups include the authenticated header, a consistent database snapshot,
and any defined blob data. They exclude Keychain items. An export must account
for pending inbox data explicitly; packaging and import are deferred to M3. Do
not copy a live WAL database as a single file. iOS restore needs password or
recovery unlock and a newly enrolled biometric envelope.

## Vectors and implementation gates

[`../vault/v1/vectors.json`](../vault/v1/vectors.json) contains public synthetic
HKDF outputs, header-MAC framing, envelope/inbox context bytes, and fixed-nonce
password, recovery, and ingest envelopes. Regenerate with
`python3 scripts/vault-format-vectors.py`; `--check` verifies the committed
fixture with Python's independent HMAC/HKDF implementation and a pure-Python
XChaCha20-Poly1305 that first checks itself against RFC 8439 section 2.8.2 and
draft-irtf-cfrg-xchacha-03 vectors. The password envelope's KEK stands in for an
Argon2id output. The minimal MAC
payload is deliberately not a valid complete vault header. The synthetic P-256
salt fixture tests HKDF only, not point validation or ECDH.

Rust tests check the HKDF subkeys and the header MAC against the fixture, and
open each envelope with the fixture's KEK and AAD, rejecting every single-bit
change. Existing primitive known-answer tests cover Argon2id against its
reference C implementation and HPKE against RFC 9180 Appendix A.2.1; property
tests cover AEAD tampering. M1.3 must add complete header and sealed inbox v2
interoperability fixtures and parser tests before claiming product-format
conformance. This RFC does not introduce a
production deterministic-nonce API.

Acceptance of this design does not finish M1. The implementation must test wrong
password/phrase/key, every header field mutation, duplicate/unknown JSON fields,
KDF bounds before allocation, malformed lengths, vault-ID substitution, crash
points during creation/updates, missing databases, rollback limitations, and
resource limits. Follow with create/unlock/lock/change/recover/delete integration
tests, then onboarding UI tests. Oldest-device Argon2 memory/timing, biometric
invalidation, backup restore, and Data Protection remain release gates.
