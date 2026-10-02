#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/core/defs.hpp>
#include <godot_cpp/godot.hpp>

#include "voxel_world_node.hpp"

using namespace godot;

void initialize_voxel_extension(ModuleInitializationLevel p_level) {
    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {
        return;
    }

    ClassDB::register_class<VoxelWorldNode>();
}

void uninitialize_voxel_extension(ModuleInitializationLevel p_level) {
    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {
        return;
    }
}

extern "C" {
    GDExtensionBool GDE_EXPORT voxel_library_init(GDExtensionInterface *p_interface,
                                                  GDExtensionClassLibraryPtr p_library,
                                                  GDExtensionInitialization *r_initialization) {
        GDExtensionBinding::InitObject init_obj(p_interface, p_library, r_initialization);

        init_obj.register_initializer(initialize_voxel_extension);
        init_obj.register_terminator(uninitialize_voxel_extension);

        return init_obj.init();
    }
}
