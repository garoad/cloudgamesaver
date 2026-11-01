#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/android-env.sh

if [ -z "${ANDROID_KEYSTORE_PATH:-}" ] && [ -f .android-signing/release.env ]; then
  source .android-signing/release.env
fi

if [ -n "${ANDROID_KEYSTORE_PATH:-}" ]; then
  android_keystore="$ANDROID_KEYSTORE_PATH"
  : "${ANDROID_KEY_ALIAS:?Set ANDROID_KEY_ALIAS for the release signing key}"
  : "${ANDROID_KEYSTORE_PASSWORD:?Set ANDROID_KEYSTORE_PASSWORD for the release signing key}"
  export ANDROID_KEY_PASSWORD="${ANDROID_KEY_PASSWORD:-$ANDROID_KEYSTORE_PASSWORD}"
  android_signing_kind="Release"
else
  if [ "${CI:-false}" = true ]; then
    echo "CI packaging requires a persistent release signing key: set ANDROID_KEYSTORE_PATH." >&2
    exit 1
  fi
  android_keystore="$HOME/.android/debug.keystore"
  export ANDROID_KEY_ALIAS=androiddebugkey
  export ANDROID_KEYSTORE_PASSWORD=android
  export ANDROID_KEY_PASSWORD=android
  android_signing_kind="Development"
fi
export ANDROID_KEYSTORE_PASSWORD

android_input="${1:-src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk}"
android_output="${2:-artifacts/android/CloudGameSaver-android-arm64.apk}"
if [ ! -f "$android_keystore" ] || [ ! -f "$android_input" ]; then
  echo "Signing key or unsigned APK not found." >&2
  exit 1
fi
android_build_tools="$ANDROID_HOME/build-tools/36.0.0"
mkdir -p "$(dirname "$android_output")"
android_signed_tmp="$(mktemp "${android_output}.XXXXXX")"
trap 'rm -f -- "$android_signed_tmp"' EXIT
"$android_build_tools/apksigner" sign \
  --ks "$android_keystore" --ks-key-alias "$ANDROID_KEY_ALIAS" \
  --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEY_PASSWORD \
  --v4-signing-enabled false \
  --out "$android_signed_tmp" "$android_input"
"$android_build_tools/apksigner" verify "$android_signed_tmp"
"$android_build_tools/zipalign" -c -P 16 4 "$android_signed_tmp"
mv "$android_signed_tmp" "$android_output"
rm -f -- "${android_output}.idsig"
echo "$android_signing_kind-signed APK: $android_output"
