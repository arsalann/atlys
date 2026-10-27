#!/usr/bin/env python3
"""Check the RFC-0001 JSON Schemas against the synthetic fixtures.

Uses only the Python standard library, so it is independent of the Rust
crate that generates `spec/adm/v0/types/`. Implements the subset of JSON
Schema 2020-12 the ADM schemas use and fails on any other keyword, so a
schema can't silently rely on something this checker ignores.

Integers follow RFC-0001: a JSON number with a fraction or exponent is not
an integer, even if its value is whole.
"""
import datetime
import json
from pathlib import Path
import re
import sys

SPEC = Path(__file__).resolve().parents[1] / 'spec/adm/v0'
ANNOTATIONS = {'$schema', 'title', 'description', 'x-sensitivity', '$defs'}
KEYWORDS = {
    'type', 'properties', 'required', 'additionalProperties', 'enum', 'const',
    'minimum', 'maximum', 'exclusiveMinimum', 'minLength', 'maxLength',
    'pattern', 'items', 'minItems', 'allOf', 'anyOf', 'not', '$ref',
}


class Invalid(Exception):
    pass


def no_duplicates(pairs):
    keys = [k for k, _ in pairs]
    if len(keys) != len(set(keys)):
        raise ValueError('duplicate key')
    return dict(pairs)


def load(path):
    return json.loads(path.read_text(), object_pairs_hook=no_duplicates)


class Validator:
    def __init__(self):
        self.files = {}

    def schema_file(self, path):
        path = path.resolve()
        if path not in self.files:
            self.files[path] = load(path)
        return self.files[path]

    def resolve(self, ref, base):
        file_part, _, pointer = ref.partition('#')
        path = (base.parent / file_part) if file_part else base
        node = self.schema_file(path)
        for part in filter(None, pointer.split('/')):
            node = node[part]
        return node, path.resolve()

    def validate(self, value, schema, base, where='$'):
        unknown = set(schema) - KEYWORDS - ANNOTATIONS
        if unknown:
            raise SystemExit(f'{base.name}: unsupported keywords {sorted(unknown)}')
        if '$ref' in schema:
            target, target_base = self.resolve(schema['$ref'], base)
            self.validate(value, target, target_base, where)
        for sub in schema.get('allOf', []):
            self.validate(value, sub, base, where)
        if 'anyOf' in schema and not any(self.ok(value, s, base) for s in schema['anyOf']):
            raise Invalid(f'{where}: matches no anyOf branch')
        if 'not' in schema and self.ok(value, schema['not'], base):
            raise Invalid(f'{where}: matches a forbidden schema')
        if 'type' in schema and not is_type(value, schema['type']):
            raise Invalid(f'{where}: expected {schema["type"]}')
        if 'const' in schema and not same(value, schema['const']):
            raise Invalid(f'{where}: expected {schema["const"]!r}')
        if 'enum' in schema and not any(same(value, e) for e in schema['enum']):
            raise Invalid(f'{where}: not one of {schema["enum"]}')
        if is_type(value, 'number'):
            if 'minimum' in schema and value < schema['minimum']:
                raise Invalid(f'{where}: below minimum')
            if 'exclusiveMinimum' in schema and value <= schema['exclusiveMinimum']:
                raise Invalid(f'{where}: not above exclusiveMinimum')
            if 'maximum' in schema and value > schema['maximum']:
                raise Invalid(f'{where}: above maximum')
        if isinstance(value, str):
            if len(value) < schema.get('minLength', 0) or len(value) > schema.get('maxLength', len(value)):
                raise Invalid(f'{where}: bad length')
            if 'pattern' in schema and not re.search(schema['pattern'], value):
                raise Invalid(f'{where}: does not match {schema["pattern"]}')
        if isinstance(value, list):
            if len(value) < schema.get('minItems', 0):
                raise Invalid(f'{where}: too few items')
            if 'items' in schema:
                for i, item in enumerate(value):
                    self.validate(item, schema['items'], base, f'{where}[{i}]')
        if isinstance(value, dict):
            for key in schema.get('required', []):
                if key not in value:
                    raise Invalid(f'{where}: missing {key}')
            props = schema.get('properties', {})
            for key, item in value.items():
                if key in props:
                    self.validate(item, props[key], base, f'{where}.{key}')
                elif schema.get('additionalProperties') is False:
                    raise Invalid(f'{where}: unexpected {key}')

    def ok(self, value, schema, base):
        try:
            self.validate(value, schema, base)
            return True
        except Invalid:
            return False


def is_type(value, kind):
    if isinstance(kind, list):
        return any(is_type(value, k) for k in kind)
    return {
        'object': isinstance(value, dict),
        'array': isinstance(value, list),
        'string': isinstance(value, str),
        'boolean': isinstance(value, bool),
        'null': value is None,
        'integer': isinstance(value, int) and not isinstance(value, bool),
        'number': isinstance(value, (int, float)) and not isinstance(value, bool),
    }[kind]


def same(a, b):
    return type(a) is type(b) and a == b


def rfc3339(ms):
    t = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc) + datetime.timedelta(milliseconds=ms)
    return t.strftime('%Y-%m-%dT%H:%M:%S.') + f'{ms % 1000:03d}Z'


def main():
    if sys.argv[1:] != ['--check']:
        raise SystemExit('usage: adm-schemas.py --check')
    v = Validator()
    types_dir = SPEC / 'types'
    draft_schema = SPEC / 'record-draft.schema.json'

    def check_record(record, where):
        v.validate(record, v.schema_file(draft_schema), draft_schema, where)
        path = types_dir / f'{record["type"]}.schema.json'
        if not path.exists():
            raise Invalid(f'{where}: no schema for type {record["type"]}')
        v.validate(record, v.schema_file(path), path, where)

    for path in sorted(SPEC.rglob('*.schema.json')):
        v.schema_file(path)
    fixtures = load(SPEC / 'fixtures.json')
    failures = []

    covered = set()
    for i, record in enumerate(fixtures['valid_records']):
        try:
            check_record(record, f'valid_records[{i}]')
            covered.add(record['type'])
        except Invalid as e:
            failures.append(str(e))
    schemas = {p.name.removesuffix('.schema.json') for p in types_dir.glob('*.schema.json')}
    if covered != schemas:
        failures.append(f'valid_records do not cover {sorted(schemas - covered)}')

    for case in fixtures['invalid_records']:
        if case.get('json_schema', True) and v_ok(lambda: check_record(case['record'], case['name'])):
            failures.append(f'invalid record accepted: {case["name"]}')

    batch_schema = SPEC / 'batch.schema.json'

    def check_batch(batch, where):
        v.validate(batch, v.schema_file(batch_schema), batch_schema, where)
        for i, record in enumerate(batch['records']):
            check_record(record, f'{where}.records[{i}]')
        for i, deletion in enumerate(batch.get('deletions', [])):
            if not (types_dir / f'{deletion["type"]}.schema.json').exists():
                raise Invalid(f'{where}.deletions[{i}]: no schema for type {deletion["type"]}')

    for i, batch in enumerate(fixtures['valid_batches']):
        try:
            check_batch(batch, f'valid_batches[{i}]')
        except Invalid as e:
            failures.append(str(e))
    for case in fixtures['invalid_batches']:
        if case.get('json_schema', True) and v_ok(lambda: check_batch(case['batch'], case['name'])):
            failures.append(f'invalid batch accepted: {case["name"]}')
    try:
        json.loads(fixtures['duplicate_key_batch_text'], object_pairs_hook=no_duplicates)
        failures.append('duplicate key batch parsed')
    except ValueError:
        pass

    record_schema = SPEC / 'record.schema.json'
    for i, case in enumerate(fixtures['stored_records']):
        where = f'stored_records[{i}]'
        out = case['json']
        try:
            check_record(case['draft'], f'{where}.draft')
            v.validate(out, v.schema_file(record_schema), record_schema, f'{where}.json')
        except Invalid as e:
            failures.append(str(e))
            continue
        draft = case['draft']
        expected_time = {'start': rfc3339(draft['start_ms'])}
        if 'end_ms' in draft:
            expected_time['end'] = rfc3339(draft['end_ms'])
        if 'tz' in draft:
            expected_time['tz'] = draft['tz']
        if out['time'] != expected_time or out['data'] != draft['data']:
            failures.append(f'{where}: time or data does not match the draft')
        if out['provenance']['ingested_at'] != rfc3339(case['ingested_ms']):
            failures.append(f'{where}: ingested_at does not match ingested_ms')
        sensitivity = v.schema_file(types_dir / f'{draft["type"]}.schema.json')['x-sensitivity']
        if out['sensitivity'] != sensitivity:
            failures.append(f'{where}: sensitivity does not match the type schema')

    for failure in failures:
        print(f'FAIL {failure}', file=sys.stderr)
    if failures:
        sys.exit(1)
    print(f'ok: {len(schemas)} type schemas, {len(fixtures["valid_records"])} valid and '
          f'{len(fixtures["invalid_records"])} invalid records, '
          f'{len(fixtures["valid_batches"]) + len(fixtures["invalid_batches"])} batches')


def v_ok(check):
    try:
        check()
        return True
    except Invalid:
        return False


if __name__ == '__main__':
    main()
