@echo off
setlocal enabledelayedexpansion

echo.
echo ===================================================
echo Persistent Voxel ARPG — M0 Setup & Build Script
echo ===================================================
echo.

REM Check if CMake is available
cmake --version >nul 2>&1
if errorlevel 1 (
    echo.
    echo [INSTALL] CMake not found. Installing via winget...
    echo.
    winget install "CMake" --accept-source-agreements --accept-package-agreements
    if errorlevel 1 (
        echo.
        echo [ERROR] winget install failed. Trying manual download...
        echo Please install CMake manually from: https://cmake.org/download/
        echo Then run this script again.
        pause
        exit /b 1
    )
    echo.
    echo [SUCCESS] CMake installed.
    echo Please close and reopen PowerShell/Command Prompt, then run this script again.
    pause
    exit /b 0
)

echo [OK] CMake found:
cmake --version
echo.

REM Check Visual Studio
if not exist "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" (
    echo [ERROR] Visual Studio 2022 not found
    echo Please install Visual Studio 2022 with C++ workload
    exit /b 1
)

echo [OK] Visual Studio 2022 found
echo.

REM Initialize MSVC environment
echo [SETUP] Initializing MSVC C++ environment...
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to initialize MSVC environment
    exit /b 1
)

echo [OK] MSVC environment ready
echo.

REM Navigate to native directory
cd native
if errorlevel 1 (
    echo [ERROR] native/ directory not found
    exit /b 1
)

REM Create build directory
if not exist "build" (
    echo [MKDIR] Creating native/build...
    mkdir build
)

cd build
if errorlevel 1 (
    echo [ERROR] Failed to enter build directory
    exit /b 1
)

echo.
echo ===================================================
echo [BUILD] Configuring with CMake...
echo ===================================================
echo.

cmake .. -G "Visual Studio 17 2022" -A x64
if errorlevel 1 (
    echo.
    echo [ERROR] CMake configuration failed
    exit /b 1
)

echo.
echo ===================================================
echo [BUILD] Compiling voxel core...
echo ===================================================
echo.

cmake --build . --config Release
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

ctest --output-on-failure --build-config Release
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
echo   1. Launch Godot: ..\..\..\Godot_v4.7.2-stable_win64.exe -e --path ".\..\..\godot"
echo   2. Open main.tscn and press F5 to test the scene
echo   3. Read M0_IMPLEMENTATION_SUMMARY.md for details
echo.
pause
