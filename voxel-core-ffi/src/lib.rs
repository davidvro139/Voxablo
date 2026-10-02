use std::os::raw::c_void;

#[repr(C)]
struct MeshData {
    positions: *mut f32,
    normals: *mut f32,
    materials: *mut u16,
    indices: *mut u32,
    vertex_count: u32,
    index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawPieceVoxel {
    x: i32,
    y: i32,
    z: i32,
    material: u16,
}

#[repr(C)]
struct DetachedPieces {
    voxels: *mut RawPieceVoxel,
    piece_starts: *mut u32,
    piece_count: u32,
    voxel_count: u32,
}

#[link(name = "voxel_wrapper", kind = "static")]
extern "C" {
    fn voxel_create_world() -> *mut c_void;
    fn voxel_destroy_world(world: *mut c_void);
    fn voxel_load_region(world: *mut c_void, region_id: u64);
    fn voxel_unload_region(world: *mut c_void, region_id: u64);
    fn voxel_generate_terrain(world: *mut c_void, region_id: u64, chunk_x: i32, chunk_y: i32, chunk_z: i32);
    fn voxel_get_voxel(world: *mut c_void, region_id: u64, x: i32, y: i32, z: i32) -> u16;
    fn voxel_build_house(world: *mut c_void, region_id: u64, x: i32, z: i32, width: i32, depth: i32, storeys: i32) -> i32;
    fn voxel_get_chunk_mesh(world: *mut c_void, region_id: u64, chunk_x: i32, chunk_y: i32, chunk_z: i32) -> MeshData;
    fn voxel_free_mesh_data(mesh: MeshData);
    fn voxel_extract_detached(
        world: *mut c_void,
        region_id: u64,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        max_piece_voxels: u32,
    ) -> DetachedPieces;
    fn voxel_free_detached(pieces: DetachedPieces);
    fn voxel_set_voxels(world: *mut c_void, region_id: u64, voxels: *const RawPieceVoxel, count: u32);
    fn voxel_apply_damage(world: *mut c_void, x: f32, y: f32, z: f32, radius: f32, energy: f32);
    fn voxel_apply_collapse(world: *mut c_void, region_id: u64);
    fn voxel_update_physics(world: *mut c_void, region_id: u64, delta_time: f32);
    fn voxel_loaded_chunk_count(world: *mut c_void) -> u64;
    fn voxel_dirty_chunk_count(world: *mut c_void) -> u64;
}

pub const CHUNK_SIZE: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Material {
    Air = 0,
    Dirt = 1,
    Wood = 2,
    Brick = 3,
    Stone = 4,
    Grass = 5,
    Path = 6,
    Water = 7,
    Tile = 8,
    Thatch = 9,
    Plaster = 10,
    Cloth = 11,
    Glass = 12,
    Ember = 13,
    Gold = 14,
    Violet = 15,
    Unknown = u16::MAX,
}

impl From<u16> for Material {
    fn from(id: u16) -> Self {
        match id {
            0 => Material::Air,
            1 => Material::Dirt,
            2 => Material::Wood,
            3 => Material::Brick,
            4 => Material::Stone,
            5 => Material::Grass,
            6 => Material::Path,
            7 => Material::Water,
            8 => Material::Tile,
            9 => Material::Thatch,
            10 => Material::Plaster,
            11 => Material::Cloth,
            12 => Material::Glass,
            13 => Material::Ember,
            14 => Material::Gold,
            15 => Material::Violet,
            _ => Material::Unknown,
        }
    }
}

/// A voxel in world voxel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceVoxel {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub material: Material,
}

/// Chunk-local mesh; `positions`, `normals` and `materials` are parallel arrays.
#[derive(Default)]
pub struct ChunkMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub materials: Vec<Material>,
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

pub struct VoxelWorld {
    handle: *mut c_void,
}

impl VoxelWorld {
    pub fn new() -> Self {
        let handle = unsafe { voxel_create_world() };
        VoxelWorld { handle }
    }

    pub fn load_region(&self, region_id: u64) {
        unsafe { voxel_load_region(self.handle, region_id) }
    }

    pub fn unload_region(&self, region_id: u64) {
        unsafe { voxel_unload_region(self.handle, region_id) }
    }

    pub fn generate_terrain(&self, region_id: u64, chunk_x: i32, chunk_y: i32, chunk_z: i32) {
        unsafe { voxel_generate_terrain(self.handle, region_id, chunk_x, chunk_y, chunk_z) }
    }

    /// Builds a house whose footprint starts at voxel (x, z); returns the ground-floor Y in voxels.
    pub fn build_house(&self, region_id: u64, x: i32, z: i32, width: i32, depth: i32, storeys: i32) -> i32 {
        unsafe { voxel_build_house(self.handle, region_id, x, z, width, depth, storeys) }
    }

    pub fn get_voxel(&self, region_id: u64, x: i32, y: i32, z: i32) -> Material {
        unsafe { voxel_get_voxel(self.handle, region_id, x, y, z) }.into()
    }

    pub fn get_chunk_mesh(&self, region_id: u64, chunk_x: i32, chunk_y: i32, chunk_z: i32) -> ChunkMesh {
        unsafe {
            let data = voxel_get_chunk_mesh(self.handle, region_id, chunk_x, chunk_y, chunk_z);
            if data.vertex_count == 0 || data.index_count == 0 {
                voxel_free_mesh_data(data);
                return ChunkMesh::default();
            }

            let n = data.vertex_count as usize;
            let as_vec3 = |ptr: *const f32| {
                std::slice::from_raw_parts(ptr, n * 3)
                    .chunks_exact(3)
                    .map(|v| [v[0], v[1], v[2]])
                    .collect()
            };

            let mesh = ChunkMesh {
                positions: as_vec3(data.positions),
                normals: as_vec3(data.normals),
                materials: std::slice::from_raw_parts(data.materials, n)
                    .iter()
                    .map(|&m| m.into())
                    .collect(),
                indices: std::slice::from_raw_parts(data.indices, data.index_count as usize).to_vec(),
            };
            voxel_free_mesh_data(data);
            mesh
        }
    }

    /// Removes voxel groups near the sphere (voxel units) that no longer connect to the ground
    /// and returns them, one `Vec` per piece. Larger groups than `max_piece_voxels` stay put.
    pub fn extract_detached(
        &self,
        region_id: u64,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        max_piece_voxels: u32,
    ) -> Vec<Vec<PieceVoxel>> {
        unsafe {
            let raw = voxel_extract_detached(self.handle, region_id, x, y, z, radius, max_piece_voxels);
            if raw.piece_count == 0 {
                voxel_free_detached(raw);
                return Vec::new();
            }
            let voxels = std::slice::from_raw_parts(raw.voxels, raw.voxel_count as usize);
            let starts = std::slice::from_raw_parts(raw.piece_starts, raw.piece_count as usize + 1);
            let pieces = starts
                .windows(2)
                .map(|w| {
                    voxels[w[0] as usize..w[1] as usize]
                        .iter()
                        .map(|v| PieceVoxel { x: v.x, y: v.y, z: v.z, material: v.material.into() })
                        .collect()
                })
                .collect();
            voxel_free_detached(raw);
            pieces
        }
    }

    pub fn set_voxels(&self, region_id: u64, voxels: &[PieceVoxel]) {
        let raw: Vec<RawPieceVoxel> = voxels
            .iter()
            .map(|v| RawPieceVoxel { x: v.x, y: v.y, z: v.z, material: v.material as u16 })
            .collect();
        unsafe { voxel_set_voxels(self.handle, region_id, raw.as_ptr(), raw.len() as u32) }
    }

    pub fn apply_damage(&self, x: f32, y: f32, z: f32, radius: f32, energy: f32) {
        unsafe { voxel_apply_damage(self.handle, x, y, z, radius, energy) }
    }

    pub fn apply_collapse(&self, region_id: u64) {
        unsafe { voxel_apply_collapse(self.handle, region_id) }
    }

    pub fn update_physics(&self, region_id: u64, delta_time: f32) {
        unsafe { voxel_update_physics(self.handle, region_id, delta_time) }
    }

    pub fn loaded_chunk_count(&self) -> u64 {
        unsafe { voxel_loaded_chunk_count(self.handle) }
    }

    pub fn dirty_chunk_count(&self) -> u64 {
        unsafe { voxel_dirty_chunk_count(self.handle) }
    }
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for VoxelWorld {
    fn drop(&mut self) {
        unsafe { voxel_destroy_world(self.handle) }
    }
}
