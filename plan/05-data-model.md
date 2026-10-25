# 05 — Atlys Data Model (ADM), draft

The ADM is the "standard and structured format" layer. It is the most important piece to get right for the protocol ambition, because it is what lets connectors, apps, and agents interoperate without knowing about each other.

## Design goals

- **Source-agnostic:** a `health.sleep_session` looks the same whether it came from Apple Health, Oura, or a Fitbit export.
- **Provenance-preserving:** every record knows where it came from and links to its raw blob.
- **Versioned and extensible:** schemas are semver'd. Connectors can add namespaced extension fields without forking the core schema.
- **Reuse existing standards where they exist**, instead of inventing new ones:
  - Health: map to **Open mHealth** schemas and **HL7 FHIR** resources where practical
  - Events/places/people: lean on **schema.org** vocabulary
  - Calendar: iCalendar (RFC 5545) semantics. Contacts: vCard / JSContact (RFC 9553).
  - Activity streams (posts, likes, follows): **W3C Activity Streams 2.0**
  - Location: GeoJSON geometries, WGS84
- **Agent-friendly:** records are compact, self-describing JSON with human-readable `summary` text, so an LLM can consume query results directly.

## Record envelope (proposal)

```jsonc
{
  "id": "rec_01J…",                 // ULID, stable within the vault
  "type": "health.sleep_session",   // namespaced ADM type
  "schema_version": "0.1.0",
  "time": { "start": "2026-10-21T23:12:00Z", "end": "2026-10-22T06:48:00Z", "tz": "Europe/Berlin" },
  "data": { /* type-specific, validated against JSON Schema */ },
  "summary": "Slept 7h36m, 1h22m deep",
  "provenance": {
    "connector": "apple-health@0.3.1",
    "source_id": "…",               // id in the source system
    "raw_ref": "blob_sha256:…",     // pointer to original raw data
    "ingested_at": "2026-10-22T07:00:03Z",
    "derived_from": []               // set for derived/computed records
  },
  "sensitivity": "high",            // drives consent + access defaults
  "ext": { "oura": { /* namespaced extension fields */ } }
}
```

## Initial namespaces

| Namespace | Example types |
| --- | --- |
| `location.*` | `point`, `visit`, `trip`, `place` |
| `health.*` | `steps`, `heart_rate`, `sleep_session`, `workout`, `body_measurement`, `nutrition` |
| `device.*` | `app_session`, `screen_session`, `battery`, `motion_activity` |
| `calendar.*` | `event` |
| `person` / `org` | contacts, entities resolved across sources |
| `comms.*` | `message`, `email`, `call` |
| `media.*` | `photo`, `video`, `listen`, `watch`, `read` |
| `web.*` | `visit`, `search`, `bookmark` |
| `commerce.*` | `order`, `transaction`, `subscription` |
| `social.*` | `post`, `reaction`, `follow` |
| `account.*` | services the user has accounts with: the "digital footprint" inventory |
| `privacy.*` | data requests, deletion requests, breach exposures, app permissions |
| `custom.*` / `note` | user-defined |

## Sensitivity tiers

`low` (e.g. listening history) · `medium` (calendar, purchases) · `high` (health, precise location, messages) · `special` (GDPR Art. 9 categories: health diagnoses, biometrics, religion, sexuality…). These tiers drive default grant scopes and consent prompts in the access layer.

## Spec deliverables

- `spec/adm/` holds JSON Schema per type, examples, and changelog.
- A generated documentation site.
- Conformance test suite: given input X, a connector must emit valid ADM Y.
- Code generation for the TypeScript, Python, Rust, Swift, and Kotlin type bindings.
