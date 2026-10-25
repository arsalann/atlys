#!/usr/bin/env bash
# Build the Rust core for iOS and package it for the AtlysCore Swift package.
#
#   cargo (device + simulator) -> uniffi-bindgen (Swift) -> XCFramework
#
# Outputs (all gitignored):
#   apps/ios/Packages/AtlysCore/AtlysCoreFFI.xcframework
#   apps/ios/Packages/AtlysCore/Sources/AtlysCore/Generated/AtlysCore.swift
#
# Usage: scripts/build-ios-core.sh [--debug]
#
# ATLYS_CORE_FEATURES selects cargo features of atlys-ffi. During M0 it
# defaults to "spike" (the throwaway benchmark/self-test API). Set it to an
# empty string for a build without the spike API.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="$ROOT/core"
PKG="$ROOT/apps/ios/Packages/AtlysCore"
CRATE="atlys-ffi"
LIB="libatlys_ffi.a"
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)

PROFILE="release"
CARGO_PROFILE_FLAG="--release"
if [[ "${1:-}" == "--debug" ]]; then
  PROFILE="debug"
  CARGO_PROFILE_FLAG=""
fi

# Keep in sync with IPHONEOS_DEPLOYMENT_TARGET in apps/ios/Config/Shared.xcconfig.
export IPHONEOS_DEPLOYMENT_TARGET="18.0"

FEATURES="${ATLYS_CORE_FEATURES-spike}"

# SQLCipher must use Apple CommonCrypto. libsqlite3-sys switches to OpenSSL if
# any of these are set, so clear them for this build.
unset OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR

if ! command -v xcrun >/dev/null || ! xcrun --find xcodebuild >/dev/null 2>&1; then
  echo "error: Xcode is required (set DEVELOPER_DIR or run xcode-select)" >&2
  exit 1
fi

cd "$CORE"

for target in "${TARGETS[@]}"; do
  echo "==> cargo build -p $CRATE --target $target ($PROFILE, features: ${FEATURES:-none})"
  # shellcheck disable=SC2086 # empty flag must expand to nothing
  cargo build -p "$CRATE" --lib --locked --target "$target" $CARGO_PROFILE_FLAG \
    --features "$FEATURES"
done

WORK="$CORE/target/ios-xcframework"
rm -rf "$WORK"
mkdir -p "$WORK/gen" "$WORK/headers"

echo "==> uniffi-bindgen generate (Swift)"
cargo run -q -p uniffi-bindgen --locked -- generate \
  --library "$CORE/target/aarch64-apple-ios/$PROFILE/$LIB" \
  --language swift \
  --out-dir "$WORK/gen"

cp "$WORK/gen/AtlysCoreFFI.h" "$WORK/headers/"
# Xcode looks for module.modulemap inside each XCFramework slice's Headers.
cp "$WORK/gen/AtlysCoreFFI.modulemap" "$WORK/headers/module.modulemap"

echo "==> xcodebuild -create-xcframework"
rm -rf "$PKG/AtlysCoreFFI.xcframework"
xcodebuild -create-xcframework \
  -library "$CORE/target/aarch64-apple-ios/$PROFILE/$LIB" -headers "$WORK/headers" \
  -library "$CORE/target/aarch64-apple-ios-sim/$PROFILE/$LIB" -headers "$WORK/headers" \
  -output "$PKG/AtlysCoreFFI.xcframework" >/dev/null

mkdir -p "$PKG/Sources/AtlysCore/Generated"
cp "$WORK/gen/AtlysCore.swift" "$PKG/Sources/AtlysCore/Generated/AtlysCore.swift"

echo "==> done: $PKG/AtlysCoreFFI.xcframework"
