cmake_minimum_required(VERSION 3.24)
project(voxel_arpg_native
    VERSION 0.1.0
    LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Compiler-specific flags
if(MSVC)
    add_compile_options(/W4 /WX)
else()
    add_compile_options(-Wall -Wextra -Werror)
endif()

# Voxel Core Library (independent of Godot)
add_library(voxel_core STATIC)

target_sources(voxel_core PRIVATE
    voxel_core/src/voxel_world.cpp
    voxel_core/src/chunk_storage.cpp
    voxel_core/src/meshing.cpp
)

target_include_directories(voxel_core PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/voxel_core/include)

# Native Unit Tests
enable_testing()

add_executable(voxel_core_tests)

target_sources(voxel_core_tests PRIVATE
    voxel_core/tests/test_chunk_storage.cpp
    voxel_core/tests/test_voxel_world.cpp
    voxel_core/tests/test_main.cpp
)

target_include_directories(voxel_core_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/voxel_core/include)

target_link_libraries(voxel_core_tests PRIVATE voxel_core)

add_test(NAME voxel_core_tests COMMAND voxel_core_tests)

# GDExtension (if building with godot-cpp)
option(BUILD_GODOT_EXTENSION "Build Godot GDExtension" OFF)

if(BUILD_GODOT_EXTENSION)
    # Placeholder for GDExtension build
    message(STATUS "GDExtension build not yet configured")
endif()

# Diagnostic output
message(STATUS "Voxel ARPG Native Build Configuration:")
message(STATUS "  C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "  Compiler: ${CMAKE_CXX_COMPILER_ID}")
message(STATUS "  Build Type: ${CMAKE_BUILD_TYPE}")
message(STATUS "  GDExtension: ${BUILD_GODOT_EXTENSION}")
