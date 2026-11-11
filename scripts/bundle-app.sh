#!/bin/bash
# Builds build/iTop Wrap.app: a release build, Resources/Info.plist and Resources/AppIcon.icns,
# ad-hoc signed (PLAN.md Task 10). Debug-only switches (ITOPWRAP_*) are compiled out of release.
# Usage: scripts/bundle-app.sh
set -e
cd "$(dirname "$0")/.."
APP="build/iTop Wrap.app"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

echo "Building release…" >&2
swift build -c release --product ITopWrap >&2
BIN="$(swift build -c release --show-bin-path)/ITopWrap"

[ -f Resources/AppIcon.icns ] || scripts/make-app-icon.sh

echo "Assembling the app…" >&2
mkdir -p "$STAGE/iTop Wrap.app/Contents/MacOS" "$STAGE/iTop Wrap.app/Contents/Resources"
cp "$BIN" "$STAGE/iTop Wrap.app/Contents/MacOS/ITopWrap"
cp Resources/AppIcon.icns "$STAGE/iTop Wrap.app/Contents/Resources/AppIcon.icns"
BUILD=$(git rev-list --count HEAD 2>/dev/null || echo 1)
sed "s/BUILD_NUMBER/$BUILD/" Resources/Info.plist > "$STAGE/iTop Wrap.app/Contents/Info.plist"
plutil -lint "$STAGE/iTop Wrap.app/Contents/Info.plist" >&2

# Ad-hoc signature: there's no Developer ID on this machine. macOS asks once per new build
# before the app can read the saved password ("Always Allow").
codesign --force --sign - --timestamp=none "$STAGE/iTop Wrap.app" >&2
codesign --verify --strict "$STAGE/iTop Wrap.app" >&2

mkdir -p build
rm -rf "$APP"
mv "$STAGE/iTop Wrap.app" "$APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" >/dev/null 2>&1 || true
echo "Built $APP (version 0.9, build $BUILD)" >&2
printf '{"app":"%s","version":"0.9","build":"%s"}\n' "$APP" "$BUILD"
