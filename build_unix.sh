#!/bin/bash
# 이 스크립트는 macOS 또는 Linux 환경에서 실행하세요.

echo "[1/3] Cleaning previous builds..."
rm -rf src-tauri/target/release
rm -rf dist

echo "[2/3] Installing dependencies..."
npm install

echo "[3/3] Building CloudGameSaver (Release Mode)..."
npm run tauri build

echo "----------------------------------------"
echo "Build Complete!"

if [[ "$OSTYPE" == "darwin"* ]]; then
  echo "macOS Bundle Location: src-tauri/target/release/bundle/dmg/"
  open src-tauri/target/release/bundle/dmg/
else
  echo "Linux Bundle Location: src-tauri/target/release/bundle/appimage/"
  # AppImage가 생성된 폴더 열기 시도
  if command -v xdg-open > /dev/null; then
    xdg-open src-tauri/target/release/bundle/appimage/
  fi
fi
