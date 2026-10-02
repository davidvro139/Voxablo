#include "voxel_world_node.hpp"
#include <godot_cpp/core/class_db.hpp>

namespace godot {

VoxelWorldNode::VoxelWorldNode() : active_region(0), has_active_region(false) {}

VoxelWorldNode::~VoxelWorldNode() {}

void VoxelWorldNode::_bind_methods() {
    ClassDB::bind_method(D_METHOD("load_region", "region_id"), &VoxelWorldNode::load_region);
    ClassDB::bind_method(D_METHOD("apply_damage", "x", "y", "z", "radius", "energy"),
                         &VoxelWorldNode::apply_damage);
    ClassDB::bind_method(D_METHOD("apply_collapse"), &VoxelWorldNode::apply_collapse);
}

void VoxelWorldNode::_ready() {
    load_region(1);
}

void VoxelWorldNode::_process(double delta) {
    if (!has_active_region) {
        return;
    }
    voxel_world.update_physics(active_region, static_cast<float>(delta));
}

void VoxelWorldNode::load_region(uint64_t region_id) {
    voxel_world.load_region(region_id);
    active_region = region_id;
    has_active_region = true;
}

void VoxelWorldNode::apply_damage(float x, float y, float z, float radius, float energy) {
    if (!has_active_region) {
        return;
    }

    voxel::DamageSphere damage;
    damage.x = x;
    damage.y = y;
    damage.z = z;
    damage.radius = radius;
    damage.energy = energy;

    voxel_world.queue_damage(active_region, damage);
}

void VoxelWorldNode::apply_collapse() {
    if (!has_active_region) {
        return;
    }
    voxel_world.queue_collapse(active_region);
}

}  // namespace godot
