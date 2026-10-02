use voxel_core_ffi::{Material, VoxelWorld};

const REGION: u64 = 1;

fn main() {
    let world = VoxelWorld::new();
    world.load_region(REGION);

    let empty = world.get_chunk_mesh(REGION, 0, 0, 0);
    assert!(empty.is_empty(), "chunk should be empty before generation");

    world.generate_terrain(REGION, 0, 0, 0);
    let before = world.get_chunk_mesh(REGION, 0, 0, 0);
    println!(
        "Generated: {} vertices, {} triangles",
        before.positions.len(),
        before.indices.len() / 3
    );
    assert!(!before.is_empty());

    // Winding must agree with the face normal (counter-clockwise from outside)
    let mut mismatched = 0;
    for tri in before.indices.chunks_exact(3) {
        let p = |i: u32| v3(before.positions[i as usize]);
        let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
        let cross = sub(b, a).cross(sub(c, a));
        if cross.dot(v3(before.normals[tri[0] as usize])) <= 0.0 {
            mismatched += 1;
        }
    }
    println!("Triangles with wrong winding: {mismatched}");
    assert_eq!(mismatched, 0);

    let surface_y = (0..32)
        .rev()
        .find(|&y| world.get_voxel(REGION, 16, y, 16) != Material::Air)
        .expect("column should contain terrain");
    println!("Surface at (16, 16): y = {surface_y}");

    world.apply_damage(16.0, surface_y as f32, 16.0, 4.0, 3.0);
    let after = world.get_chunk_mesh(REGION, 0, 0, 0);
    println!(
        "After damage: {} vertices, {} triangles, voxel at surface = {:?}",
        after.positions.len(),
        after.indices.len() / 3,
        world.get_voxel(REGION, 16, surface_y, 16)
    );
    assert_eq!(world.get_voxel(REGION, 16, surface_y, 16), Material::Air);
    assert_ne!(before.indices.len(), after.indices.len());

    println!("OK");
}

#[derive(Clone, Copy)]
struct V(f32, f32, f32);

impl V {
    fn cross(self, o: V) -> V {
        V(self.1 * o.2 - self.2 * o.1, self.2 * o.0 - self.0 * o.2, self.0 * o.1 - self.1 * o.0)
    }
    fn dot(self, o: V) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
}

fn v3(p: [f32; 3]) -> V {
    V(p[0], p[1], p[2])
}

fn sub(a: V, b: V) -> V {
    V(a.0 - b.0, a.1 - b.1, a.2 - b.2)
}
