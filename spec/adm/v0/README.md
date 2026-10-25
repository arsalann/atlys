# Atlys Data Model (ADM) v0

JSON Schemas for the v0 record envelope and the `health.*`, `location.*`, and
`device.*` record types live here.

The schemas are written alongside **RFC-0001: Atlys Data Model v0** in
milestone M2 (see [plan/v0/04-work-breakdown.md](../../../plan/v0/04-work-breakdown.md)).
Until then this directory is intentionally empty.

Rules for this directory:

- One schema file per record type, named after the type, for example
  `health.steps.schema.json`.
- Every example and test fixture is synthetic. Never commit data exported
  from a real person's device or account (see [AGENTS.md](../../../AGENTS.md)).
- Changes after v0 is released go through the RFC process.
