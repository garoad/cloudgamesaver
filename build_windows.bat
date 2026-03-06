@echo off
set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
echo [1/3] Cleaning previous builds...
rmdir /s /q src-tauri\target\release
rmdir /s /q dist

echo [2/3] Building CloudGameSaver for Windows (Release Mode)...
npm run tauri build

echo [3/3] Build Complete!
echo Opening output directory...
start src-tauri\target\release\bundle\msi
pause
