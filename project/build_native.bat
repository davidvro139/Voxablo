@echo off
setlocal enabledelayedexpansion

echo.
echo === Voxablo — Native Build ===
echo.

set BUILD_DIR=native\build
set CONFIG=Release

if not exist "%BUILD_DIR%" (
    echo Creating build directory...
    mkdir "%BUILD_DIR%"
)

echo Running CMake configuration...
cd native
if not exist build mkdir build
cd build
cmake .. -G "Visual Studio 17 2022" -A x64
if errorlevel 1 (
    echo CMAKE CONFIGURATION FAILED
    exit /b 1
)

echo.
echo Building voxel_core library...
cmake --build . --config %CONFIG%
if errorlevel 1 (
    echo BUILD FAILED
    exit /b 1
)

echo.
echo === Build Complete ===
echo.
echo Running unit tests...
ctest --output-on-failure --build-config %CONFIG%
if errorlevel 1 (
    echo TESTS FAILED
    exit /b 1
)

echo.
echo === All Tests Passed ===
exit /b 0
