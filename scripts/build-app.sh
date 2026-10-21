#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_dir"
cargo build --release

app_dir="$project_dir/dist/Naranja.app"
mkdir -p "$app_dir/Contents/MacOS"
cp "$project_dir/target/release/naranja" "$app_dir/Contents/MacOS/naranja"
cat > "$app_dir/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Naranja</string>
  <key>CFBundleDisplayName</key><string>Naranja</string>
  <key>CFBundleExecutable</key><string>naranja</string>
  <key>CFBundleIdentifier</key><string>org.naranja.converter</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

echo "Created $app_dir"
