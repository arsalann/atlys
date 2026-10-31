#!/usr/bin/env python3
"""Catch lost HealthKit capabilities after XcodeGen rewrites the app plists."""
import plistlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
with (ROOT / "apps/ios/Atlys/Atlys.entitlements").open("rb") as source:
    entitlements = plistlib.load(source)
with (ROOT / "apps/ios/Atlys/Info.plist").open("rb") as source:
    info = plistlib.load(source)

assert entitlements.get("com.apple.developer.healthkit") is True, "HealthKit entitlement missing"
assert entitlements.get("com.apple.developer.healthkit.background-delivery") is True, "Background delivery missing"
assert "health-records" in entitlements.get("com.apple.developer.healthkit.access", []), "Health Records access missing"
assert info.get("NSHealthShareUsageDescription"), "Health read purpose string missing"
assert info.get("NSHealthClinicalHealthRecordsShareUsageDescription"), "Clinical read purpose string missing"
assert info.get("AtlysHealthRecords") is True, "Clinical collector disabled"
print("HealthKit capabilities and purpose strings present (signed-device verification still required)")
