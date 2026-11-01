#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/android-env.sh

if [ -z "${ANDROID_KEYSTORE_PATH:-}" ] && [ -f .android-signing/release.env ]; then
  source .android-signing/release.env
fi
if [ "${CI:-false}" = true ] && [ -z "${ANDROID_KEYSTORE_PATH:-}" ]; then
  echo "CI packaging requires ANDROID_KEYSTORE_PATH for the release signing key." >&2
  exit 1
fi

android_keystore="${ANDROID_KEYSTORE_PATH:-$HOME/.android/debug.keystore}"
if [ ! -f "$android_keystore" ]; then
  echo "Signing key not found. Set ANDROID_KEYSTORE_PATH, or build a debug APK to create the development key." >&2
  exit 1
fi
npm run android:build -- --ci
bash scripts/sign_android.sh
