# RFC-0001: Atlys Data Model v0

Status: **Proposed** (2026-10-27). This is the design step for M2.1. It is
implemented by `core/crates/atlys-adm`, with JSON Schemas and synthetic
fixtures in [`spec/adm/v0/`](../adm/v0/).

## Scope

This RFC defines the records that the v0 collectors produce and the vault
stores:

- The record draft a collector seals into the inbox.
- Batches: the plaintext of one sealed inbox file (the bytes and encryption are in [RFC-0002](0002-vault-format.md#sealed-inbox-version-2)).
- The v0 record types for `health.*`, `location.*`, and `device.*`.
- Deletions.
- Merge semantics.
- The record JSON that the app shows and that exports write.

Queries, exports, derived records, and connectors outside the phone are out of
scope. The long-term direction is in
[plan/05-data-model.md](../../plan/05-data-model.md).

MUST, MUST NOT, and SHOULD are requirements for v0 writers (collectors) and
readers (the vault). Every example and fixture is synthetic.

## Encoding

- Batches are UTF-8 JSON. Readers reject duplicate object keys at any depth, unknown fields, and `null` wherever a value is required.
- Integers are JSON numbers without a fraction or exponent. `3.0` is not an integer, even though plain JSON Schema would accept it.
- Times are Unix epoch milliseconds (UTC), from `0` to `253402300799999` (9999-12-31T23:59:59.999Z).
- String lengths count Unicode scalar values.
- Units are part of field names (`meters`, `kcal`, `bpm`, `sdnn_ms`, `duration_s`), so a value never needs a separate unit field. Values are not localized.

## Record drafts

A collector produces record drafts:

| Field | Required | Rule |
| --- | --- | --- |
| `type` | yes | A type defined below. Unknown types are rejected. |
| `source_id` | yes | 1–256 characters, no C0 control characters or DEL. Stable for the same underlying item, for example a HealthKit sample UUID. |
| `start_ms` | yes | Start, or the instant for point records. |
| `end_ms` | per type | Required for interval types, forbidden for instant types, optional for the rest. Must be `>= start_ms`. |
| `tz` | no | IANA time zone where the record happened: 1–64 of `A-Z a-z 0-9 _ + - /`. |
| `source` | no | Origin inside the device: `name`, `bundle_id`, `device`, each 1–256 characters. Example: the app and watch that wrote a HealthKit sample. |
| `data` | yes | An object holding exactly the fields of the type, as listed below. |

**Identity.** `(type, source_id)` identifies a record across batches. A
collector MUST derive `source_id` from the source system so that re-reading
the same item produces the same ID. Examples: HealthKit sample UUIDs; for a
visit, its arrival time; for a motion segment, its start time.

Collectors MUST NOT use random IDs for items they might read again. A
snapshot record that is taken once, such as a battery reading, MAY use any
unique ID.

## Record types

Each type has a time shape (instant, interval, or either) and a default
sensitivity tier. The generated schema in [`spec/adm/v0/types/`](../adm/v0/types/)
is normative for the `data` fields, their ranges, and their descriptions. The
crate's type table is the source those schemas are generated from.

| Type | Time | Sensitivity | `data` |
| --- | --- | --- | --- |
| `device.battery` | instant | low | `level` 0–1 (optional), `state` unknown/unplugged/charging/full, `low_power_mode` |
| `device.connectivity` | instant | low | `status` satisfied/unsatisfied/requires_connection, `interface` wifi/cellular/wired/loopback/other/none, `expensive`, `constrained` |
| `device.info` | instant | low | `os_name`, `os_version`, `model`, `app_version`, optional `free_storage_bytes`, `locale`, `time_zone` |
| `device.motion_activity` | interval | medium | `activity` stationary/walking/running/cycling/automotive/unknown, `confidence` low/medium/high |
| `health.body_mass` | instant | high | `kg` |
| `health.distance` | interval | high | `meters`, `activity` walking_running/cycling/swimming/wheelchair/downhill_snow_sports |
| `health.energy` | interval | high | `kcal`, `kind` active/basal |
| `health.exercise_time` | interval | high | `seconds` |
| `health.floors` | interval | high | `count` |
| `health.heart_rate` | either | high | `bpm` |
| `health.height` | instant | high | `m` |
| `health.hrv` | either | high | `sdnn_ms` |
| `health.mindful_session` | interval | high | none |
| `health.resting_heart_rate` | either | high | `bpm` |
| `health.sleep_stage` | interval | high | `stage` in_bed/asleep_unspecified/asleep_core/asleep_deep/asleep_rem/awake |
| `health.stand_hour` | interval | high | `stood` |
| `health.steps` | interval | high | `count` |
| `health.walking_heart_rate` | either | high | `bpm` |
| `health.workout` | interval | high | `activity` (snake_case HealthKit activity name), `duration_s`, optional `kcal`, `meters` |
| `location.point` | instant | high | `latitude`, `longitude`, `horizontal_accuracy_m`, optional `altitude_m`, `vertical_accuracy_m`, `speed_mps`, `course_deg` |
| `location.visit` | either | high | `latitude`, `longitude`, `horizontal_accuracy_m`. `end_ms` is absent until the departure is known. |

Coordinates are WGS84 degrees.

**Mapping to standards.** Health types follow the HealthKit identifiers listed
in [plan/v0/02-data-sources.md](../../plan/v0/02-data-sources.md#1-health-healthkit)
and use Open mHealth's unit choices where one exists: `bpm`, `kcal`, meters,
and seconds. Sleep sessions, daily totals, and places are **derived** records.
They are computed from these types later and are not part of v0.

## Batches

A batch is the records and deletions from one collector run:

| Field | Required | Rule |
| --- | --- | --- |
| `adm_version` | yes | Exactly `"0.1"` for this RFC. |
| `collector` | yes | `[a-z][a-z0-9_]{0,31}`, for example `health`. |
| `created_ms` | yes | When the batch was sealed. |
| `records` | yes | Record drafts. May be empty only if `deletions` is not. |
| `deletions` | no | Objects with exactly `type` (a defined type) and `source_id`. |

A batch is valid only if every record and deletion in it is valid. Writers
MUST validate before sealing, which `IngestSealer.seal` does, so an invalid
batch at merge time indicates a bug or a forged file.

## Merge

On unlock, the vault merges pending inbox files one at a time, in file-name
order. File names are ULIDs, so this is write order. For each file:

1. **Open.** Open and parse the file (RFC-0002), then validate the batch.
   - A batch whose `adm_version` is a well-formed `major.minor` later than the reader's stays pending, and the app reports that an update is needed. The reader checks this before anything else in the batch.
   - Any other invalid batch is quarantined with the reason code `invalid-batch` (RFC-0002). The whole file is quarantined, never only part of it, so nothing is silently dropped.
2. **Apply, in one database transaction:**
   - **Records, in order.** If `(type, source_id)` is tombstoned, skip the record. Otherwise insert it with a new ULID `id`. If the key already exists, replace its time, `tz`, `source`, and `data`, and keep its `id`. The last write in merge order wins. Replacement is how a visit gains its departure time and how a motion segment grows.
   - **Deletions.** Delete the record with that key, if present, and record a tombstone for the key so a re-read sample cannot return.
   - **Merge log.** Record the SHA-256 of the sealed file in the merge log.
3. **Delete** the file only after the transaction commits.

If the file's hash is already in the merge log, delete the file without
applying it again. A crash between commit and delete therefore neither loses
nor duplicates anything. Replaying the same records through a different file
is also harmless, because merging is idempotent per key.

## Record JSON

A stored record is rendered as the following JSON. The app shows it and
exports write it ([`record.schema.json`](../adm/v0/record.schema.json)):

```json
{
  "id": "01K86H3X00ABCDEFGHJKMN0001",
  "type": "health.steps",
  "schema_version": "0.1",
  "time": { "start": "2025-10-22T06:48:00.000Z", "end": "2025-10-22T06:49:00.000Z" },
  "data": { "count": 42 },
  "provenance": {
    "collector": "health",
    "source_id": "00000000-0000-4000-8000-00000000000f",
    "source": { "name": "Health", "bundle_id": "com.apple.health", "device": "iPhone" },
    "ingested_at": "2025-10-22T06:53:00.123Z"
  },
  "sensitivity": "high"
}
```

- Times are RFC 3339 UTC with millisecond precision.
- `time.end`, `time.tz`, and `provenance.source` appear only when present.
- `id` is vault-local and is not a global identifier.

Reserved for later versions:

- A human-readable `summary`.
- `raw_ref`, a pointer to the original raw blob.
- `derived_from`.
- Extension fields in `ext`.

## Versioning

`adm_version` and `schema_version` are both `0.1` for this RFC.

- **Minor version.** Adding a type or an optional field bumps the minor version. Readers accept only versions they implement, so an older app leaves newer batches pending instead of dropping data.
- **Major version.** Changing or removing a field needs a new major version and a migration RFC.

Until v0 ships, this RFC may still change through PRs that update the crate,
schemas, and fixtures together.

## Errors

Validation errors carry a stable code and a path, for example
`records[1].data.count`:

| Code | Meaning |
| --- | --- |
| `malformed` | Not JSON, a duplicate key, or a top-level value of the wrong shape |
| `unsupported_version` | Written by a newer ADM version |
| `unknown_type` | Type not defined in this version |
| `missing_field` | A required field is absent, including `end_ms` on interval types |
| `unknown_field` | A field not defined here, including `end_ms` on instant types |
| `wrong_type` | A field has the wrong JSON type |
| `out_of_range` | A number is outside its range |
| `invalid_value` | A string is empty, too long, not an allowed value, or badly formatted |
| `invalid_time` | `end_ms` is before `start_ms` |

## Privacy

- Record contents never appear in logs, error messages, or quarantine reason codes. Error messages contain only a code and a path.
- `source` names apps and devices on the user's phone. It is personal data, stored only inside the vault.
- Sensitivity tiers are defaults for future sharing and consent features. v0 has no sharing.
- HealthKit-derived records keep `provenance.collector = "health"`, so later sharing features can enforce Apple's HealthKit rules.

## Fixtures and gates

[`spec/adm/v0/fixtures.json`](../adm/v0/fixtures.json) contains synthetic
fixtures:

- Valid records covering every type.
- Invalid records with their expected code and path.
- Valid and invalid batches.
- Stored records with their expected record JSON.

Two independent checks run in CI:

- `cargo test -p atlys-adm` validates every fixture through the crate. It also fails if the schemas in `types/` drift from the type table (regenerate them with `ATLYS_UPDATE_SCHEMAS=1`).
- `python3 -I scripts/adm-schemas.py --check` validates the same fixtures against the JSON Schemas, using only the Python standard library. It also recomputes the RFC 3339 times.

Fixtures marked `"json_schema": false` are rules that plain JSON Schema cannot
express, such as `end_ms >= start_ms` or integers without a fraction.
