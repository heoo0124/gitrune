#!/bin/sh
# Builds the desktop bundle, signs it, and repackages the macOS disk image.
#
# `tauri build` writes the .app and the .dmg in one pass, and the signature it
# leaves on the .app is the linker's: it covers the executable but seals no
# resources, so Gatekeeper refuses to open the bundle. Signing after that pass
# means the .dmg already holds the unsigned copy, so the image is rebuilt here.
#
# Usage: scripts/bundle-release.sh [--target <triple>]
set -eu

TARGET_ARG=""
if [ "${1:-}" = "--target" ] && [ -n "${2:-}" ]; then
  TARGET_ARG="--target $2"
fi

pnpm exec tauri build $TARGET_ARG

# Only macOS has a bundle signature to repair.
if [ "$(uname -s)" = "Darwin" ]; then
  scripts/sign-macos.sh

  case "$TARGET_ARG" in
    *x86_64-apple-darwin*) TRIPLE="x64" ;;
    *) TRIPLE="aarch64" ;;
  esac
  VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
  APP="target/release/bundle/macos/GitRune.app"
  DMG="target/release/bundle/dmg/GitRune_${VERSION}_${TRIPLE}.dmg"

  echo "repackaging $DMG with the signed app"
  STAGE=$(mktemp -d)
  trap 'rm -rf "$STAGE"' EXIT
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  rm -f "$DMG"
  hdiutil create -volname "GitRune" -srcfolder "$STAGE" -ov -format UDZO "$DMG" > /dev/null
  echo "bundled $(du -h "$DMG" | cut -f1)  $DMG"
fi
