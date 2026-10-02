#pragma once

#include <godot_cpp/classes/node3d.hpp>
#include "../voxel_core/include/voxel_core.hpp"

namespace godot {

class VoxelWorldNode : public Node3D {
    GDCLASS(VoxelWorldNode, Node3D)

public:
    VoxelWorldNode();
    ~VoxelWorldNode();

    void _ready() override;
    void _process(double delta) override;

    void load_region(uint64_t region_id);
    void apply_damage(float x, float y, float z, float radius, float energy);
    void apply_collapse();

protected:
    static void _bind_methods();

private:
    voxel::VoxelWorld voxel_world;
    voxel::RegionId active_region;
    bool has_active_region;
};

}  // namespace godot
