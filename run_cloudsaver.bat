@echo off
set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
echo Starting CloudSaver with Rust environment...
npm run tauri dev
pause
