#!/usr/bin/env bash
# Print an xcodebuild -destination for the newest available iPhone simulator,
# for example `id=1A2B...`. Override with ATLYS_SIM_UDID.
set -euo pipefail

if [[ -n "${ATLYS_SIM_UDID:-}" ]]; then
  echo "id=$ATLYS_SIM_UDID"
  exit 0
fi

xcrun simctl list -j devices available | python3 -c '
import json, re, sys

devices = json.load(sys.stdin)["devices"]
best = None
for runtime, entries in devices.items():
    m = re.search(r"iOS-(\d+)-(\d+)", runtime)
    if not m:
        continue
    version = (int(m.group(1)), int(m.group(2)))
    for d in entries:
        if d.get("isAvailable") and d["name"].startswith("iPhone"):
            key = (version, d["name"])
            if best is None or key > best[0]:
                best = (key, d["udid"])
if best is None:
    sys.exit("no available iPhone simulator found")
print(f"id={best[1]}")
'
