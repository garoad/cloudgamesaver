#!/usr/bin/env bash
# Source this file so the CLI, Gradle and Cargo use the same toolchain.
if [ -d "$HOME/.cargo/bin" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

if [ -z "${ANDROID_HOME:-}" ]; then
  if [ "$(uname -s)" = Darwin ]; then
    export ANDROID_HOME="$HOME/Library/Android/sdk"
  else
    export ANDROID_HOME="$HOME/Android/Sdk"
  fi
fi
export ANDROID_SDK_ROOT="$ANDROID_HOME"

if [ -z "${JAVA_HOME:-}" ]; then
  if [ -d "/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" ]; then
    export JAVA_HOME="/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home"
  elif [ -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]; then
    export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  fi
fi
if [ -n "${JAVA_HOME:-}" ]; then
  export PATH="$JAVA_HOME/bin:$PATH"
fi

if [ -z "${NDK_HOME:-}" ]; then
  if [ -d "$ANDROID_HOME/ndk/27.2.12479018" ]; then
    export NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
  else
    for android_ndk in "$ANDROID_HOME"/ndk/*; do
      if [ -d "$android_ndk" ]; then export NDK_HOME="$android_ndk"; fi
    done
  fi
fi
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
