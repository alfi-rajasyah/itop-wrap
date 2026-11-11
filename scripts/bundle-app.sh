#!/bin/bash
# Builds build/iTop Wrap.app: a release build, Resources/Info.plist and Resources/AppIcon.icns,
# signed (PLAN.md Task 10). Debug-only switches (ITOPWRAP_*) are compiled out of release.
# Usage: scripts/bundle-app.sh            signs with your Apple Development certificate if you
#                                         have one (rebuilds keep Keychain access), else ad hoc
#        SIGN=adhoc scripts/bundle-app.sh always ad hoc (release DMGs: a development signature
#                                         carries your name and Apple ID email)
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

# With an Apple Development certificate, every build has the same signing identity, so a
# Keychain "Always Allow" lasts across rebuilds. Ad hoc, macOS asks once per new build.
# The identity is found at run time by its hash; nothing about it is written to the repo.
IDENTITY="-"
if [ "${SIGN:-auto}" != "adhoc" ]; then
    IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null \
        | awk '/"Apple Development: / { print $2; exit }')
    [ -n "$IDENTITY" ] || IDENTITY="-"
fi
if [ "$IDENTITY" = "-" ]; then
    echo "Signing ad hoc…" >&2
else
    echo "Signing with your Apple Development certificate…" >&2
fi
codesign --force --sign "$IDENTITY" --timestamp=none "$STAGE/iTop Wrap.app" >&2
codesign --verify --strict "$STAGE/iTop Wrap.app" >&2

mkdir -p build
rm -rf "$APP"
mv "$STAGE/iTop Wrap.app" "$APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" >/dev/null 2>&1 || true
VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
SIGNED=$([ "$IDENTITY" = "-" ] && echo adhoc || echo development)
echo "Built $APP (version $VERSION, build $BUILD, signed $SIGNED)" >&2
printf '{"app":"%s","version":"%s","build":"%s","signed":"%s"}\n' "$APP" "$VERSION" "$BUILD" "$SIGNED"
