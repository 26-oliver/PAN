#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
app="${1:-dist/PAN.app}"
binary="${2:-target/release/pan}"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 755 "$binary" "$app/Contents/MacOS/pan"
install -m 644 assets/pan.icns "$app/Contents/Resources/pan.icns"
install -m 644 packaging/macos/Info.plist "$app/Contents/Info.plist"
codesign --force --sign - "$app"
