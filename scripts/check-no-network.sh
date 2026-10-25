#!/usr/bin/env bash
# Fail if app or core code can make network requests.
#
# v0 makes zero network requests (plan/v0/README.md). This is a coarse,
# deliberately strict text check; code review is the second line of defense.
# Read-only path monitoring (NWPathMonitor) is allowed.
#
# Exceptions go in scripts/no-network-allowlist.txt as `path:pattern` lines,
# each with a comment explaining why.
#
# Usage: scripts/check-no-network.sh [root]
set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
ALLOWLIST="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/no-network-allowlist.txt"

# Swift / Objective-C / C in the apps.
APPLE_PATTERNS=(
  'URLSession'
  'URLRequest'
  'NSURLConnection'
  'NWConnection'
  'NWListener'
  'NWBrowser'
  'NWConnectionGroup'
  'NWEndpoint'
  'NWParameters'
  'import WebKit'
  'WKWebView'
  'SFSafariViewController'
  'import SafariServices'
  'ASWebAuthenticationSession'
  'import MultipeerConnectivity'
  'import CFNetwork'
  'CFStreamCreatePairWithSocket'
  'CFSocketCreate'
  'getaddrinfo'
  'URL\(string: *"(https?|wss?|ftp)://'
)

# Info.plist keys that only matter if the app talks to the network.
PLIST_PATTERNS=(
  'NSAppTransportSecurity'
  'NSAllowsArbitraryLoads'
)

# Rust in the core.
RUST_PATTERNS=(
  'std::net'
  'TcpStream'
  'TcpListener'
  'UdpSocket'
)

allowed() {
  local hit="$1" entry path pattern
  [[ -f "$ALLOWLIST" ]] || return 1
  while IFS= read -r entry; do
    [[ -z "$entry" || "$entry" == \#* ]] && continue
    path="${entry%%:*}"
    pattern="${entry#*:}"
    [[ "$hit" == "$path:"* && "$hit" == *"$pattern"* ]] && return 0
  done <"$ALLOWLIST"
  return 1
}

failures=0
scan() {
  local dir="$1" include="$2"
  shift 2
  [[ -d "$ROOT/$dir" ]] || return 0
  local pattern hit
  for pattern in "$@"; do
    while IFS= read -r hit; do
      [[ -z "$hit" ]] && continue
      hit="${hit#"$ROOT"/}"
      if allowed "$hit"; then
        continue
      fi
      echo "::error::network API in app code: $hit"
      failures=$((failures + 1))
    done < <(grep -rnE --include="$include" -e "$pattern" "$ROOT/$dir" \
      --exclude-dir=.build --exclude-dir=DerivedData --exclude-dir=target || true)
  done
}

for ext in '*.swift' '*.m' '*.mm' '*.h' '*.c'; do
  scan apps "$ext" "${APPLE_PATTERNS[@]}"
done
scan apps '*.plist' "${PLIST_PATTERNS[@]}"
scan core '*.rs' "${RUST_PATTERNS[@]}"

if ((failures > 0)); then
  echo "check-no-network: $failures violation(s). v0 must not use the network." >&2
  echo "If a hit is a false positive, add it to scripts/no-network-allowlist.txt with a reason." >&2
  exit 1
fi
echo "check-no-network: ok"
