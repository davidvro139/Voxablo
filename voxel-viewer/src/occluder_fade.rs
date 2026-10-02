//! StandardMaterial extension that dithers away geometry between the camera and the player.

use bevy::asset::load_internal_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

const SHADER: Handle<Shader> = Handle::weak_from_u128(0x6f0c_c1d0_fade_4a1b_9e3f_27b4_d5c8_a901);

pub type FadingMaterial = ExtendedMaterial<StandardMaterial, OccluderFade>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct OccluderFade {
    #[uniform(100)]
    pub params: OccluderFadeParams,
}

pub use uniform::OccluderFadeParams;

// encase's ShaderType derive emits `check` helpers that are never called in a binary crate.
#[allow(dead_code)]
mod uniform {
    use super::*;

    /// Layout must match `OccluderFade` in occluder_fade.wgsl.
    #[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
    pub struct OccluderFadeParams {
        /// xyz: centre of the player's body, w: height of the feet
        pub player: Vec4,
        /// xyz: camera view direction, w: cut-away radius in metres
        pub view_dir: Vec4,
        /// x: fade strength at the centre of the cut-away (0..1)
        pub settings: Vec4,
    }

    impl OccluderFadeParams {
        pub fn new(feet: Vec3, body_center_height: f32, view_dir: Vec3, radius: f32, strength: f32) -> Self {
            Self {
                player: (feet + Vec3::Y * body_center_height).extend(feet.y),
                view_dir: view_dir.normalize().extend(radius),
                settings: Vec4::new(strength, 0.0, 0.0, 0.0),
            }
        }
    }
}

impl MaterialExtension for OccluderFade {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(SHADER)
    }
}

pub struct OccluderFadePlugin;

impl Plugin for OccluderFadePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, SHADER, "occluder_fade.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<FadingMaterial>::default());
    }
}
