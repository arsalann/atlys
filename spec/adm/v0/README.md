# Atlys Data Model (ADM) v0

JSON Schemas and synthetic fixtures for
[RFC-0001: Atlys Data Model v0](../../rfcs/0001-data-model.md).

| File | Contents |
| --- | --- |
| [`record-draft.schema.json`](record-draft.schema.json) | The record envelope a collector seals |
| [`batch.schema.json`](batch.schema.json) | The plaintext of one sealed inbox file |
| [`record.schema.json`](record.schema.json) | Record JSON as the app shows it and exports write it |
| [`types/`](types/) | One schema per record type, **generated** from the type table in `core/crates/atlys-adm/src/types.rs` |
| [`fixtures.json`](fixtures.json) | Synthetic valid and invalid records, batches, and stored records |

Checks (both run in CI):

```sh
cd core && cargo test -p atlys-adm                # crate vs. fixtures; fails if types/ is stale
ATLYS_UPDATE_SCHEMAS=1 cargo test -p atlys-adm --test spec   # regenerate types/
python3 -I scripts/adm-schemas.py --check         # schemas vs. fixtures, stdlib only
```

Rules for this directory:

- Change a record type in the crate's type table, regenerate `types/`, and update the RFC and fixtures in the same PR.
- Every example and test fixture is synthetic. Never commit data exported
  from a real person's device or account (see [AGENTS.md](../../../AGENTS.md)).
- Changes after v0 is released go through the RFC process.
