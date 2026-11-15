#!/bin/bash
# Builds build/iTop Wrap.app: the web UI, a release build of the Rust app, bundled by Tauri and
# signed. Debug-only switches (ITOPWRAP_*) are compiled out of release.
# Usage: scripts/bundle-app.sh            signs with your Apple Development certificate if you
#                                         have one (rebuilds keep Keychain access), else ad hoc
#        SIGN=adhoc scripts/bundle-app.sh always ad hoc (release DMGs: a development signature
#                                         carries your name and Apple ID email)
set -e
cd "$(dirname "$0")/.."
APP="build/iTop Wrap.app"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

[ -d ui/node_modules ] || { echo "Installing the UI's packages…" >&2; npm --prefix ui ci >&2; }

echo "Building release…" >&2
# Tauri runs the UI build itself (tauri.conf.json: beforeBuildCommand), then cargo.
ui/node_modules/.bin/tauri build --bundles app >&2
BUILT="target/release/bundle/macos/iTop Wrap.app"
[ -d "$BUILT" ] || { echo "Tauri didn't produce $BUILT" >&2; exit 1; }

echo "Assembling the app…" >&2
cp -R "$BUILT" "$STAGE/iTop Wrap.app"
BUILD=$(git rev-list --count HEAD 2>/dev/null || echo 1)
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD" "$STAGE/iTop Wrap.app/Contents/Info.plist"
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
codesign --force --deep --sign "$IDENTITY" --timestamp=none "$STAGE/iTop Wrap.app" >&2
codesign --verify --strict "$STAGE/iTop Wrap.app" >&2

mkdir -p build
rm -rf "$APP"
mv "$STAGE/iTop Wrap.app" "$APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" >/dev/null 2>&1 || true
VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
SIGNED=$([ "$IDENTITY" = "-" ] && echo adhoc || echo development)
echo "Built $APP (version $VERSION, build $BUILD, signed $SIGNED)" >&2
printf '{"app":"%s","version":"%s","build":"%s","signed":"%s"}\n' "$APP" "$VERSION" "$BUILD" "$SIGNED"
