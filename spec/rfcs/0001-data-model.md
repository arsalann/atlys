# RFC-0001: Atlys Data Model v0

Status: **Accepted** (2026-10-27, PR #10). Revised before v0 shipped (PR #11): raw data, all accessible types, deletions kept as flags. This is the design step for M2.1. It is
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
- Units: values are stored as the source reports them. If the source attaches a unit, the record stores the unit string next to the value (for example `{"value": 0.5, "unit": "mi"}`). Fixed units appear in field names (`distance_m`, `duration_s`). Atlys never converts or localizes values when storing them.

## Principles

These come from the maintainer:

1. **Collect everything that is accessible.** Each collector reads every type and field its framework exposes, not a curated subset.
2. **Keep it raw.** Store source values, units, codes, and identifiers as-is. Names Atlys adds (such as `value_name`) sit next to the raw value and never replace it.
3. **Never lose data because the source did.** When the source deletes an item, the vault keeps the record and flags it as deleted at the source.

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
sensitivity tier. The generated schemas in [`spec/adm/v0/types/`](../adm/v0/types/)
are normative for the `data` fields, their ranges, and their descriptions. They
are generated from the crate's type table.

Apple Health uses **generic families** keyed by the HealthKit identifier, so
every readable type is covered. This includes types added in later iOS
releases, with no change to this RFC.

| Type | Time | Sensitivity | `data` |
| --- | --- | --- | --- |
| `health.quantity` | either | high | `identifier` (for example `HKQuantityTypeIdentifierStepCount`), `value`, `unit` (the unit Apple Health uses for the type, for example `count/min` or `mi`), `metadata` |
| `health.category` | either | high | `identifier`, raw integer `value`, optional `value_name` (for example `asleepCore`), `metadata` |
| `health.workout` | interval | high | raw `activity_type`, `activity_name`, `duration_s`, `statistics` (per quantity type: sum, average, minimum, maximum, unit), `events`, `activities`, `metadata` |
| `health.workout_route` | interval | high | `workout_id` (the workout's source ID), `points` |
| `health.electrocardiogram` | interval | high | raw `classification` and `symptoms_status`, `average_heart_rate_bpm`, `sampling_frequency_hz`, `voltages` |
| `health.heartbeat_series` | interval | high | `beats` |
| `health.audiogram` | either | high | `sensitivity_points` |
| `health.state_of_mind` | either | high | raw `kind`, `valence`, `labels`, `associations` |
| `health.clinical_record` | either | high | `identifier`, `display_name`, and the FHIR resource as-is |
| `health.characteristic` | instant | high | `identifier` (for example date of birth or blood type), raw `value` |
| `health.sample` | either | high | `identifier` and `details` for any other HealthKit sample type |
| `location.point` | instant | high | `latitude`, `longitude`, `horizontal_accuracy_m`, plus every optional Core Location field (altitudes, accuracies, speed, course, floor, simulated, from accessory) and the `trigger` that delivered it |
| `location.visit` | either | high | `latitude`, `longitude`, `horizontal_accuracy_m`. `end_ms` is absent until the departure is known. |
| `device.motion_activity` | instant | medium | Every Core Motion flag (stationary, walking, running, cycling, automotive, unknown; several can be true) and raw `confidence` 0–2. Each update holds until the next. |
| `device.pedometer` | interval | medium | `steps`, `distance_m`, `floors_ascended`, `floors_descended`, `average_active_pace_s_per_m` |
| `device.battery` | instant | low | `level` 0–1 (optional), `state`, `low_power_mode` |
| `device.connectivity` | instant | low | `status`, `interface`, `expensive`, `constrained`, IPv4/IPv6/DNS support |
| `device.info` | instant | low | OS, model, app version, storage, locale, time zone, thermal state, uptime, memory |

Coordinates are WGS84 degrees.

**Metadata.** `metadata` holds HealthKit metadata as-is. Dates become epoch
milliseconds, quantities become `{value, unit}`, and other values keep their
JSON form.

**Subtype.** A record's `data.identifier`, when present, is its subtype. The
app groups records by type and subtype for browsing.

Derived records (sleep sessions, daily totals, places) are computed later from
these records and are not part of v0.

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
   - **Records, in order.** Insert each record with a new ULID `id`. If `(type, source_id)` already exists, replace its time, `tz`, `source`, and `data`, keep its `id`, and clear any deleted-at-source flag, since the source has the item again. The last write in merge order wins. Replacement is how a visit gains its departure time and how the current pedometer hour grows.
   - **Deletions.** Set `deleted_at_source` on the record with that key, if present and not already flagged. **The record is never removed.** A deletion for a key the vault has never seen is ignored.
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
  "type": "health.quantity",
  "schema_version": "0.1",
  "time": { "start": "2025-10-22T06:48:00.000Z", "end": "2025-10-22T06:49:00.000Z" },
  "data": { "identifier": "HKQuantityTypeIdentifierStepCount", "value": 42, "unit": "count" },
  "provenance": {
    "collector": "health",
    "source_id": "00000000-0000-4000-8000-000000000007",
    "source": { "name": "Health", "bundle_id": "com.apple.health", "device": "iPhone" },
    "ingested_at": "2025-10-22T06:53:00.123Z"
  },
  "sensitivity": "high"
}
```

- Times are RFC 3339 UTC with millisecond precision.
- `time.end`, `time.tz`, `provenance.source`, and `provenance.deleted_at_source` appear only when present. `deleted_at_source` is when the vault learned that the source deleted the item.
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
