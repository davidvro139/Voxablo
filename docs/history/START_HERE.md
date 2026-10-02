# 🎮 Voxablo — START HERE

**Status:** ✅ **M0 COMPLETE** — Ready for M1 (Voxel Terrain Rendering)

---

## 📦 What You Have

```
project/
├── native/                    # C++20 voxel core (COMPILED ✅)
│   ├── voxel_core/
│   │   ├── include/           # API headers
│   │   ├── src/               # Implementation (chunk, world, meshing stub)
│   │   └── tests/             # Unit tests (11/11 PASSING ✅)
│   ├── voxel_core.sln         # Visual Studio solution
│   ├── bin/Release/           # Compiled binaries
│   │   ├── voxel_core_lib.lib  # Static library
│   │   └── voxel_core_tests.exe
│   └── build.bat              # One-command build script
│
├── godot/                     # Godot 4.7.2 project (READY ✅)
│   ├── project.godot          # Project config
│   ├── scenes/
│   │   └── main.tscn          # Main scene with isometric camera
│   └── scripts/
│       ├── main.gd            # Scene controller
│       └── diagnostic_overlay.gd
│
├── BUILD_INSTRUCTIONS.md      # How to build (all platforms)
├── SETUP_ENVIRONMENT.md       # Prerequisites
├── QUICK_START.md             # 5-minute setup
├── M0_IMPLEMENTATION_SUMMARY.md
└── M0_COMPLETE.md             # Full details (you are reading adjacent to this)
```

---

## 🚀 Quick Start (5 Minutes)

### 1. Build & Test Voxel Core
```batch
cd project
build.bat
```

**Expected output:**
```
✅ Build succeeded!

=== Running Unit Tests ===
[PASS] Chunk: set/get voxel
[PASS] Chunk: clean on creation
... (11/11 PASSED)
```

### 2. Launch Godot Editor
Navigate to `project/godot/` folder and run:
```batch
..\..\Godot_v4.7.2-stable_win64.exe -e
```

### 3. Open & Play Main Scene
- File → Open Scene → `scenes/main.tscn`
- Press **F5** to play
- You'll see: empty 3D scene, isometric camera, FPS counter (top-left)
- Press **ESC** to quit

---

## ✅ What Works Now (M0)

✅ **Native C++20 Voxel Core**
- 32×32×32 chunk storage
- Voxel editing (set/get)
- Region management
- Dirty chunk tracking for optimization

✅ **Unit Tests**
- 11 tests, all passing
- Chunk operations covered
- Region lifecycle covered
- Bounds safety verified

✅ **Godot Project**
- Isometric camera (45° rotation, orthographic)
- Diagnostic overlay (FPS, frame time)
- Main scene structure
- Ready for M1 integration

✅ **Build System**
- Visual Studio 2022 (MSBuild)
- One-command build & test
- Release optimizations enabled
- Zero compiler warnings

---

## 🎯 Next: M1 — Voxel Terrain Rendering

**What M1 will add:**
1. **Greedy Meshing** — Convert voxels to GPU-friendly triangles
2. **GDExtension Binding** — Connect native voxel core to Godot
3. **Procedural Generation** — Perlin noise terrain
4. **Rendering** — Textured voxel chunks streamed in real-time
5. **Diagnostics** — Overlay shows live chunk loading stats

**Expected time:** 2-3 weeks solo dev

**Acceptance criteria:**
- Terrain visible in Godot editor
- No visible chunk seams
- Baseline FPS recorded

---

## 📖 Documentation Map

| Document | Purpose |
|----------|---------|
| **QUICK_START.md** | 5-minute setup guide |
| **BUILD_INSTRUCTIONS.md** | Platform-specific build steps |
| **M0_COMPLETE.md** | Full M0 technical summary |
| **06_PROTOTYPE_ROADMAP.md** | Overall milestones & acceptance criteria |
| **02_TECHNICAL_ARCHITECTURE.md** | System design (voxel core, GDExtension, threading) |
| **01_GAME_DESIGN.md** | Game vision & mechanics |

---

## 🛠️ Useful Commands

### Build + Test (One Command)
```batch
cd project
build.bat
```

### Build Only
```batch
cd project\native
msbuild voxel_core.sln /p:Configuration=Release /p:Platform=x64
```

### Run Tests Only
```batch
cd project\native
bin\Release\voxel_core_tests.exe
```

### Open Godot (Editor Mode)
```batch
cd project\godot
..\..\Godot_v4.7.2-stable_win64.exe -e
```

### View Test Code
```
project/native/voxel_core/tests/
├── test_chunk_storage.cpp   (6 tests)
└── test_voxel_world.cpp     (5 tests)
```

---

## 📊 Stats

| Metric | Value |
|--------|-------|
| C++ Code | ~550 LOC |
| Unit Tests | 11 (all passing) |
| Test Coverage | Chunk ops, regions, bounds safety |
| Build Time | ~2 seconds |
| Compiler | MSVC (C++20, /O2, /W4 /WX) |
| Godot Version | 4.7.2 stable |
| Native → Godot | GDExtension (ready for M1) |

---

## 💡 Key Design Decisions

1. **Voxel Size:** 10 cm (can test 5 cm if benchmarks justify)
2. **Chunk Size:** 32³ = 32,768 voxels per chunk
3. **No Godot in voxel_core:** Pure C++20, zero Godot dependencies
4. **Sparse storage:** Chunks only created on edit
5. **Dirty tracking:** For meshing prioritization
6. **Visual Studio only:** MSBuild (no CMake needed)

---

## ⚠️ Limitations (By Design)

These features come later:
- ✋ No rendering yet (M1)
- ✋ No physics (M3)
- ✋ No destruction simulation (M2)
- ✋ No save/load (M4)
- ✋ No enemies/combat (M5)
- ✋ No procedural generation (M1)

---

## 🎬 Getting Started

1. **Read this file** ← You are here
2. **Run:** `cd project && build.bat` (2 min)
3. **Launch Godot** and open `scenes/main.tscn` (1 min)
4. **Verify:** Tests pass, editor opens, camera visible (1 min)
5. **Read:** `06_PROTOTYPE_ROADMAP.md` for M1 details
6. **Start M1:** Implement greedy meshing

---

## 🚨 Troubleshooting

**Build fails with "msbuild not found":**
→ Open PowerShell in admin mode, run setup_and_build.bat

**Godot won't launch:**
→ Verify Godot_v4.7.2-stable_win64.exe exists in parent folder

**Tests show 0 passed:**
→ Ensure build.bat ran to completion, check project/native/bin/Release/

**VSCode not recognizing C++ includes:**
→ Optional; not needed for build. Use Visual Studio IDE if desired.

---

## 📞 Questions?

- **How to build?** → See `BUILD_INSTRUCTIONS.md`
- **What's the architecture?** → See `02_TECHNICAL_ARCHITECTURE.md`
- **What's the game?** → See `01_GAME_DESIGN.md`
- **What's next?** → See `06_PROTOTYPE_ROADMAP.md` (M1 section)

---

## ✨ Status Summary

```
M0 — Repository & Instrumentation
├── ✅ Repository structure
├── ✅ Native C++20 voxel core (compiled)
├── ✅ 11 unit tests (all passing)
├── ✅ Godot 4.7.2 project
├── ✅ Isometric camera scene
├── ✅ Diagnostic overlay
├── ✅ Build system (MSBuild)
├── ✅ Documentation (5 files)
└── ✅ READY FOR M1

Next → M1: Voxel Terrain Rendering
```

---

**Ready to make voxels! 🎨**

Go build: `cd project && build.bat`
