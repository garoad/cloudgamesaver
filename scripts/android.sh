#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/android-env.sh

if ! command -v rustup >/dev/null || [ -z "${NDK_HOME:-}" ]; then
  echo "Android toolchain is missing. Run npm run android:setup first." >&2
  exit 1
fi
android_java_version="$(java -version 2>&1)"
android_java_pattern='version "([0-9]+)'
if [[ "$android_java_version" =~ $android_java_pattern ]]; then
  android_java_major="${BASH_REMATCH[1]}"
  if [ "$android_java_major" -lt 17 ] || [ "$android_java_major" -gt 24 ]; then
    echo "Set JAVA_HOME to JDK 17 or 21; this Gradle version does not support JDK $android_java_major." >&2
    exit 1
  fi
fi
# Android 15+ devices can use 16 KB memory pages. Cargo's RUSTFLAGS take
# precedence over the CLI's target flags, so retain Tauri's Android libraries.
if [ "${1:-}" != init ]; then
  # Keep generated Android resources in sync with the project's app icons.
  cp -R src-tauri/icons/android/. src-tauri/gen/android/app/src/main/res/
  export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-landroid -C link-arg=-llog -C link-arg=-lOpenSLES"
fi
exec npm run tauri -- android "$@"
