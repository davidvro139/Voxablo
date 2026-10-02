@echo off
setlocal enabledelayedexpansion

echo.
echo ===================================================
echo Voxablo — Build & Test (Visual Studio)
echo ===================================================
echo.

REM Initialize MSVC environment
set VSINSTALL=C:\Program Files\Microsoft Visual Studio\2022\Community
if not exist "%VSINSTALL%\VC\Auxiliary\Build\vcvarsall.bat" (
    echo [ERROR] Visual Studio 2022 not found at %VSINSTALL%
    exit /b 1
)

echo [SETUP] Initializing MSVC C++ environment...
call "%VSINSTALL%\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to initialize MSVC
    exit /b 1
)

echo [OK] MSVC ready
echo.

REM Change to native directory
cd native
if errorlevel 1 (
    echo [ERROR] native/ directory not found
    exit /b 1
)

echo ===================================================
echo [BUILD] Compiling voxel core library and tests...
echo ===================================================
echo.

msbuild voxel_core.sln /p:Configuration=Release /p:Platform=x64 /m
if errorlevel 1 (
    echo.
    echo [ERROR] Build failed
    exit /b 1
)

echo.
echo ===================================================
echo [TEST] Running unit tests...
echo ===================================================
echo.

bin\Release\voxel_core_tests.exe
if errorlevel 1 (
    echo.
    echo [ERROR] Tests failed
    exit /b 1
)

echo.
echo ===================================================
echo [SUCCESS] M0 Complete!
echo ===================================================
echo.
echo Next steps:
echo   1. Launch Godot: ..\Godot_v4.7.2-stable_win64.exe -e --path ".\godot"
echo   2. Open scenes/main.tscn and press F5
echo   3. Press ESC to quit
echo.
pause
