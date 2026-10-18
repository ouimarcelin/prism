#!/bin/sh
# Builds Prism as a macOS application (Prism.app) and installs it in
# ~/Applications, to open it from the Finder, the Dock or Spotlight.
#
#   scripts/make-app.sh          # without NDI
#   scripts/make-app.sh --ndi    # with NDI (ADR 0026)
#
# The application is not signed: it opens on the Mac that built it. With
# PRISM_NO_INSTALL=1 the bundle stays in the target folder (used by
# scripts/make-dmg.sh). PRISM_TARGETS lists the Rust targets to build, for
# a universal binary: "aarch64-apple-darwin x86_64-apple-darwin".
set -eu

cd "$(dirname "$0")/.."
features=""
if [ "${1:-}" = "--ndi" ]; then
    features="--features ndi"
fi

target_dir=$(cargo metadata --format-version 1 --no-deps \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
bundle="$target_dir/Prism.app"

rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
if [ -n "${PRISM_TARGETS:-}" ]; then
    binaries=""
    for target in $PRISM_TARGETS; do
        # shellcheck disable=SC2086
        cargo build --release -p prism-app --target "$target" $features
        binaries="$binaries $target_dir/$target/release/prism"
    done
    # shellcheck disable=SC2086
    lipo -create $binaries -output "$bundle/Contents/MacOS/prism"
else
    # shellcheck disable=SC2086
    cargo build --release -p prism-app $features
    cp "$target_dir/release/prism" "$bundle/Contents/MacOS/prism"
fi

# Icon: every size macOS asks for, from assets/icon-1024.png (the logo).
iconset="$target_dir/Prism.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size assets/icon-1024.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z $double $double assets/icon-1024.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/Prism.icns"
rm -rf "$iconset"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Prism</string>
    <key>CFBundleDisplayName</key><string>Prism</string>
    <key>CFBundleIdentifier</key><string>org.prism-vj.prism</string>
    <key>CFBundleExecutable</key><string>prism</string>
    <key>CFBundleIconFile</key><string>Prism</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSCameraUsageDescription</key><string>Prism montre les caméras comme des clips.</string>
    <key>NSMicrophoneUsageDescription</key><string>Prism écoute l'entrée audio pour animer les réglages au son.</string>
</dict>
</plist>
PLIST

if [ -n "${PRISM_NO_INSTALL:-}" ]; then
    echo "$bundle"
    exit 0
fi

mkdir -p "$HOME/Applications"
rm -rf "$HOME/Applications/Prism.app"
cp -R "$bundle" "$HOME/Applications/Prism.app"
echo "Prism installé : $HOME/Applications/Prism.app"
