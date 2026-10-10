@echo off
set CARGO_TARGET_DIR=%TEMP%\adb_gui_build
echo Building ADB GUI in %CARGO_TARGET_DIR%...
cargo build --release
if %ERRORLEVEL% equ 0 (
    echo Copying adb_gui.exe to project root...
    copy /Y "%TEMP%\adb_gui_build\release\adb_gui.exe" ".\adb_gui.exe"
    echo Build complete! Executable is ready: E:\ADB-GUI\adb_gui.exe
) else (
    echo Build failed with exit code %ERRORLEVEL%
)
