#!/bin/bash
# Builds Resources/AppIcon.icns from the Stacked stubs mark (Sources/ITopWrap/MenuBarIcon.swift).
# Usage: scripts/make-app-icon.sh [preview.png]
set -e
cd "$(dirname "$0")/.."
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
echo "Drawing the icon…" >&2
swiftc -o "$TMP/make-icon" Sources/ITopWrap/MenuBarIcon.swift scripts/app-icon/main.swift
"$TMP/make-icon" "$TMP/AppIcon.iconset" ${1:+"$1"}
mkdir -p Resources
iconutil -c icns "$TMP/AppIcon.iconset" -o Resources/AppIcon.icns
echo "Wrote Resources/AppIcon.icns" >&2
