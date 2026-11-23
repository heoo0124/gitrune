#!/bin/sh
# Signs the macOS bundle the way Gatekeeper needs it.
#
# `tauri build` leaves Rust's linker signature in place: it covers the executable
# only, so the bundle reports `Sealed Resources=none` and Gatekeeper refuses to
# open it ("code has no resources but signature indicates they must be present").
# Re-signing the whole bundle seals Info.plist and Resources.
#
# Usage: scripts/sign-macos.sh [path/to/GitRune.app]   (defaults to the release bundle)
set -eu

# Resolve against the repository root so a caller's cwd does not decide the path.
cd "$(dirname "$0")/.."

APP=${1:-target/release/bundle/macos/GitRune.app}

[ -d "$APP" ] || { echo "no bundle at $APP" >&2; exit 1; }

# An identity from the environment wins (a real certificate), else ad-hoc.
IDENTITY=${APPLE_SIGNING_IDENTITY:-}
case "$IDENTITY" in
  "" | "-") IDENTITY="-" ;;
esac

echo "signing $APP with identity '$IDENTITY'"
codesign --force --deep --sign "$IDENTITY" "$APP"

# Fail loudly rather than ship a bundle that will not launch.
codesign --verify --deep --strict "$APP"
echo "signature verified:"
codesign -dv --verbose=2 "$APP" 2>&1 | grep -E 'Sealed Resources|Info.plist entries' || true
