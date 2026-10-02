# Persistent Voxel ARPG — Build Instructions (M0)

## Prerequisites

### Windows (Tested: Windows 11)
- **CMake** 3.24 or later
- **Visual Studio 2022** with C++20 support (MSVC v143)
- **Godot 4.7.2** (included in parent directory)
- **godot-cpp** (to be vendored for M1)

### macOS / Linux
- **CMake** 3.24 or later
- **Clang 14+** or **GCC 11+** with C++20 support

## Building Native Voxel Core

### Windows (Visual Studio)

```batch
cd project\native
mkdir build
cd build
cmake .. -G "Visual Studio 17 2022" -A x64
cmake --build . --config Release
```

### Windows (Command Line with Run Instructions)

Run the provided build script:

```batch
cd project
.\build_native.bat
```

### macOS / Linux

```bash
cd project/native
mkdir build
cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build .
```

## Running Native Unit Tests

### Windows

```batch
cd project\native\build
ctest --output-on-failure
# Or directly:
Release\voxel_core_tests.exe
```

### macOS / Linux

```bash
cd project/native/build
ctest --output-on-failure
# Or directly:
./voxel_core_tests
```

## Running Godot

### Launch the Godot editor:

```batch
# Windows
..\Godot_v4.7.2-stable_win64.exe -e --path ".\project\godot"
```

```bash
# macOS / Linux
godot -e --path "./project/godot"
```

### Play the scene:
- From the Godot editor, select **File → Open Scene** and open `scenes/main.tscn`
- Press **Play** (F5) to run the scene

### Headless smoke test (command line):
```batch
..\Godot_v4.7.2-stable_win64.exe --headless --script ".\project\godot\scripts\smoke_test.gd" --path ".\project\godot"
```

## M0 Acceptance Criteria

- [x] Native voxel_core builds as standalone library
- [x] Unit tests build and run independently
- [x] Godot project opens without errors
- [x] Main scene displays with isometric camera
- [x] Diagnostic overlay shows FPS and frame time
- [x] Build/test commands documented
- [ ] Headless smoke test passes (GDExtension integration)

## Known Limitations (M0)

- No voxel rendering yet (M1)
- No damage/destruction yet (M2)
- No GDExtension bindings yet (integrated in M1)
- Diagnostic overlay shows placeholder values (awaiting M1 integration)

## Troubleshooting

### CMake not found
Ensure CMake is installed and in your PATH.

### Visual Studio version mismatch
Update to Visual Studio 2022 with C++20 workload.

### Godot won't launch
Verify the Godot executable path matches your installation location.

## Next Steps (M1)
- Implement greedy meshing algorithm
- Integrate godot-cpp bindings
- Create GDExtension native class
- Stream and render procedural terrain
