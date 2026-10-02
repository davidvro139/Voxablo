# Quick Start — M0 Build

## Step 1: Install CMake (5 minutes)

### Option A: Automatic (PowerShell as Admin)
```powershell
winget install CMake
```

### Option B: Manual Download
1. Go to: **https://cmake.org/download/**
2. Download: **"Windows x86-64 Installer"** (latest version)
3. Run installer
4. ✅ **CHECK** "Add CMake to system PATH" during installation
5. Click Finish

### Option C: Verify Installation
Open a NEW PowerShell window and run:
```powershell
cmake --version
```

You should see: `cmake version X.XX.X`

---

## Step 2: Build Voxel Core (2 minutes)

Once CMake is installed, run from the project directory:

```powershell
cd <repo-root>/project
.\setup_and_build.bat
```

This will:
- ✅ Configure with CMake
- ✅ Compile with MSVC
- ✅ Run 11 unit tests
- ✅ Report results

---

## Step 3: Launch Godot

Once build succeeds:

```powershell
cd <repo-root>
.\Godot_v4.7.2-stable_win64.exe -e --path ".\project\godot"
```

Then in the editor:
1. Open `scenes/main.tscn`
2. Press **F5** to play
3. You should see an empty 3D scene with a diagonal camera
4. Top-left shows FPS, chunks, and frame time
5. Press **ESC** to quit

---

## Troubleshooting

### "cmake: command not found"
- CMake wasn't added to PATH
- Close PowerShell completely
- Open a NEW PowerShell window
- Try `cmake --version` again

### Build fails with "cl.exe not found"
- Visual Studio C++ tools not installed
- Run Visual Studio Installer
- Add "Desktop development with C++"
- Restart PowerShell

### Godot won't launch
- Path to executable may be wrong
- Try: `Get-ChildItem -Recurse -Filter "Godot*.exe"`
- Update the command with correct path

---

## Expected Test Output

```
=== Voxel Core Unit Tests ===

--- Chunk Storage Tests ---
[PASS] Chunk: set/get voxel
[PASS] Chunk: clean on creation
[PASS] Chunk: marks dirty on edit
[PASS] Chunk: can mark clean
[PASS] Chunk: out-of-bounds writes are safe
[PASS] Chunk size constant

--- VoxelWorld Tests ---
[PASS] VoxelWorld: load region
[PASS] VoxelWorld: unload region
[PASS] VoxelWorld: queue edits
[PASS] VoxelWorld: multiple chunks
[PASS] VoxelWorld: dirty chunks tracked

=== Test Summary ===
Passed: 11
Failed: 0
```

---

## What You've Built

✅ **Native C++20 Voxel Core**
- Chunk storage (32³ voxels)
- Region management
- Dirty chunk tracking
- No dependencies on Godot

✅ **Unit Tests**
- Chunk set/get operations
- VoxelWorld region lifecycle
- Edit queueing
- Dirty flag tracking

✅ **Godot 4.7.2 Project**
- Isometric camera
- Diagnostic overlay (FPS, frame time)
- Main controller script

✅ **Build System**
- CMake (cross-platform)
- Windows MSVC support
- Automated test runner

---

## Next: M1 — Voxel Terrain Rendering

Once M0 passes all tests, M1 will add:
- Greedy meshing algorithm
- Procedural terrain generation
- GDExtension bindings to Godot
- Rendering with texture atlasing
- Chunk streaming and culling

---

## Questions?

See the detailed docs:
- `project/BUILD_INSTRUCTIONS.md` — Full platform guide
- `project/SETUP_ENVIRONMENT.md` — Prerequisites
- `project/M0_IMPLEMENTATION_SUMMARY.md` — Technical details
