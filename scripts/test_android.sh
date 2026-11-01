#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/android-env.sh

# Build/install the application first with android:build -- --debug --ci.
# Select Documents/CloudGameSaverTest in the app before running this command.
src-tauri/gen/android/gradlew -p src-tauri/gen/android \
  :app:assembleUniversalDebugAndroidTest \
  -PabiList=arm64-v8a -ParchList=arm64 -PtargetList=aarch64 \
  -x rustBuildArm64Debug
adb install -r src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug/app-universal-debug-androidTest.apk
# The runner closes existing activities before starting. Tauri exits when its
# final activity is destroyed, so start instrumentation in a fresh process.
adb shell am force-stop com.garoad.cloudgamesaver
android_test_result="$(adb shell am instrument -w -r \
  -e waitForActivitiesToComplete false \
  -e class com.garoad.cloudgamesaver.SaveStoragePluginTest \
  com.garoad.cloudgamesaver.test/androidx.test.runner.AndroidJUnitRunner)"
printf '%s\n' "$android_test_result"
android_success_pattern='OK \([1-9][0-9]* tests?\)'
if ! [[ "$android_test_result" =~ $android_success_pattern ]]; then exit 1; fi
