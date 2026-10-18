#!/bin/sh
# Builds the macOS disk image of Prism (Prism-<version>.dmg), ready to hand
# out: the application and a link to /Applications (ADR 0039).
#
#   scripts/make-dmg.sh [--ndi] [--universal]
#
# --universal builds for Apple Silicon and Intel in one binary (needs the
# x86_64-apple-darwin target: rustup target add x86_64-apple-darwin).
#
# Signing and notarization, when the variables are set:
#   PRISM_SIGN_IDENTITY   "Developer ID Application: Name (TEAMID)", a
#                         certificate of the keychain
#   PRISM_NOTARY_PROFILE  keychain profile of notarytool
#                         (xcrun notarytool store-credentials)
# Without an identity, the application is signed ad hoc: it opens on other
# Macs only after a right click > Open (Gatekeeper).
set -eu

cd "$(dirname "$0")/.."
app_args=""
for argument in "$@"; do
    case "$argument" in
        --ndi) app_args="--ndi" ;;
        --universal) PRISM_TARGETS="aarch64-apple-darwin x86_64-apple-darwin"; export PRISM_TARGETS ;;
        *) echo "option inconnue : $argument" >&2; exit 2 ;;
    esac
done

# shellcheck disable=SC2086
bundle=$(PRISM_NO_INSTALL=1 scripts/make-app.sh $app_args | tail -n 1)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
dist="$(dirname "$bundle")/dist"
mkdir -p "$dist"
dmg="$dist/Prism-$version.dmg"

identity="${PRISM_SIGN_IDENTITY:-}"
if [ -n "$identity" ]; then
    codesign --force --options runtime --timestamp \
        --entitlements installer/macos/entitlements.plist \
        --sign "$identity" "$bundle"
else
    codesign --force --sign - "$bundle"
    echo "Signature ad hoc : définir PRISM_SIGN_IDENTITY pour un Developer ID." >&2
fi
codesign --verify --strict "$bundle"

staging="$dist/staging"
rm -rf "$staging" "$dmg"
mkdir -p "$staging"
cp -R "$bundle" "$staging/Prism.app"
ln -s /Applications "$staging/Applications"
cp LICENSE-MIT LICENSE-APACHE NOTICE "$staging/"
hdiutil create -volname "Prism $version" -srcfolder "$staging" -ov -format UDZO "$dmg" >/dev/null
rm -rf "$staging"

if [ -n "$identity" ]; then
    codesign --force --timestamp --sign "$identity" "$dmg"
    if [ -n "${PRISM_NOTARY_PROFILE:-}" ]; then
        xcrun notarytool submit "$dmg" --keychain-profile "$PRISM_NOTARY_PROFILE" --wait
        xcrun stapler staple "$dmg"
    else
        echo "Pas de notarisation : définir PRISM_NOTARY_PROFILE." >&2
    fi
fi
echo "$dmg"
