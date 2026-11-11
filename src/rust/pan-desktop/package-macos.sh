#!/bin/zsh
set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../../.." && pwd)"
output_dir="${1:-$repo_dir/dist}"
app="$output_dir/PAN Preview.app"
iconset="$repo_dir/target/pan-icon.iconset"

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$iconset"
cp "$repo_dir/target/release/pan-desktop" "$app/Contents/MacOS/PAN Preview"

for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$repo_dir/src/rust/pan-desktop/assets/pan-logo.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$repo_dir/src/rust/pan-desktop/assets/pan-logo.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
python3 "$repo_dir/src/rust/pan-desktop/tools/create_icns.py" "$iconset" "$app/Contents/Resources/PAN.icns"

cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>PAN Preview</string>
  <key>CFBundleDisplayName</key><string>PAN Preview</string>
  <key>CFBundleIdentifier</key><string>com.pan.preview</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleExecutable</key><string>PAN Preview</string>
  <key>CFBundleIconFile</key><string>PAN</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

plutil -lint "$app/Contents/Info.plist"
xattr -cr "$app"
codesign --force --deep --sign - "$app"
ditto -c -k --sequesterRsrc --keepParent "$app" "$output_dir/PAN-Preview-macOS.zip"
