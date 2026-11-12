#!/bin/bash
# Builds the app (scripts/bundle-app.sh), then a compressed disk image with the app and an
# Applications shortcut: build/iTop-Wrap-<version>.dmg, plus its SHA-256.
# Usage: scripts/make-dmg.sh
set -e
cd "$(dirname "$0")/.."
# Always ad hoc: a development signature would publish your name and Apple ID email.
SIGN=adhoc scripts/bundle-app.sh >/dev/null
APP="build/iTop Wrap.app"
VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
# "1.0" is named 1.0.0, like the release tag; "1.0.1" stays as it is.
[[ "$VERSION" == *.*.* ]] || VERSION="$VERSION.0"
DMG="build/iTop-Wrap-$VERSION.dmg"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

echo "Making the disk image…" >&2
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "iTop Wrap $VERSION" -srcfolder "$STAGE" -ov -format UDZO -fs HFS+ "$DMG" >/dev/null
hdiutil verify "$DMG" >/dev/null
SUM=$(shasum -a 256 "$DMG" | cut -d' ' -f1)
echo "$SUM  $(basename "$DMG")" > "$DMG.sha256"
echo "Built $DMG" >&2
printf '{"dmg":"%s","sha256":"%s"}\n' "$DMG" "$SUM"
