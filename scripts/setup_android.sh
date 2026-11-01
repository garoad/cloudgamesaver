#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/android-env.sh

if ! command -v java >/dev/null; then
  echo "Install Android Studio or JDK 17+ first." >&2
  exit 1
fi

android_download_dir="$(mktemp -d "${TMPDIR:-/tmp}/cloudgamesaver-android.XXXXXX")"
if ! command -v rustup >/dev/null; then
  case "$(uname -s)/$(uname -m)" in
    Darwin/arm64) rustup_platform=aarch64-apple-darwin ;;
    Darwin/x86_64) rustup_platform=x86_64-apple-darwin ;;
    Linux/x86_64) rustup_platform=x86_64-unknown-linux-gnu ;;
    Linux/aarch64) rustup_platform=aarch64-unknown-linux-gnu ;;
    *) echo "Install rustup manually on this host." >&2; exit 1 ;;
  esac
  curl -fL --retry 3 "https://static.rust-lang.org/rustup/dist/$rustup_platform/rustup-init" -o "$android_download_dir/rustup-init"
  chmod +x "$android_download_dir/rustup-init"
  RUSTUP_INIT_SKIP_PATH_CHECK=yes "$android_download_dir/rustup-init" -y --no-modify-path --profile minimal
  export PATH="$HOME/.cargo/bin:$PATH"
fi
rustup target add aarch64-linux-android x86_64-linux-android

android_sdkmanager="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [ ! -x "$android_sdkmanager" ]; then
  case "$(uname -s)" in
    Darwin) android_tools_platform=mac ;;
    Linux) android_tools_platform=linux ;;
    *) echo "Install Android SDK command-line tools manually on this host." >&2; exit 1 ;;
  esac
  curl -fL --retry 3 "https://dl.google.com/android/repository/commandlinetools-${android_tools_platform}-13114758_latest.zip" -o "$android_download_dir/tools.zip"
  unzip -q "$android_download_dir/tools.zip" -d "$android_download_dir/tools"
  mkdir -p "$ANDROID_HOME/cmdline-tools"
  mv "$android_download_dir/tools/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
fi

# sdkmanager asks for the Android SDK licenses before installing packages.
set +o pipefail
yes | "$android_sdkmanager" --sdk_root="$ANDROID_HOME" --licenses >/dev/null
set -o pipefail
"$android_sdkmanager" --sdk_root="$ANDROID_HOME" "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;27.2.12479018"
echo "Android toolchain ready. Run npm run android:init, then npm run android:build."
