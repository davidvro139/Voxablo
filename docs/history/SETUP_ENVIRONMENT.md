# Environment Setup for Voxablo

## Status: M0 Repository Structure Complete

Your project structure is ready, but the development environment needs to be configured.

## Required Tools

### 1. Visual Studio 2022 (MSVC C++ Compiler)
- **Download:** https://visualstudio.microsoft.com/downloads/
- **Install** with "Desktop development with C++" workload
- Ensure C++20 language feature is available

### 2. CMake 3.24+
- **Download:** https://cmake.org/download/
- **Windows:** Download MSI installer
- **Install** and add to PATH (installer option: "Add CMake to the system PATH")

### 3. Git (optional but recommended)
- **Download:** https://git-scm.com/download/win

## Verification

Open PowerShell and verify installations:

```powershell
# Check CMake version
cmake --version

# Check MSVC compiler
cl.exe
# If not found, run: "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
```

## Quick Start After Installation

1. **Build native voxel core:**
   ```batch
   cd project
   build_native.bat
   ```

2. **Launch Godot:**
   ```batch
   ..\Godot_v4.7.2-stable_win64.exe -e --path ".\godot"
   ```

## Godot 4.7.2 Information

- **Location:** `../Godot_v4.7.2-stable_win64.exe`
- **Version:** 4.7.2 stable
- **Architecture:** Win64
- **Project path:** `project/godot/`

## Next Steps

1. Install prerequisites above
2. Run `build_native.bat` to compile and test
3. Launch Godot editor to verify scene loads
4. Report any build errors

## M0 Deliverables

✅ Repository structure created  
✅ Native voxel core source code  
✅ CMake build configuration  
✅ Unit tests framework  
✅ Godot project (4.7.2)  
✅ Isometric camera scene  
✅ Diagnostic overlay  
✅ Build instructions  
⏳ Build tools installation (your next step)  
⏳ Native compilation & tests  
⏳ Godot integration

## Support

If CMake or Visual Studio installation fails:
- Ensure you have ~10 GB free disk space
- Disable antivirus temporarily during installation
- Restart PowerShell after PATH changes
