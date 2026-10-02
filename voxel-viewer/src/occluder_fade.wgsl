// StandardMaterial shading, plus a dithered cut-away of geometry that sits between the camera and
// the player. Only the main pass is affected, so faded walls still cast shadows.

#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#endif

struct OccluderFade {
    // xyz: centre of the player's body, w: height of the player's feet
    player: vec4<f32>,
    // xyz: camera view direction (unit length), w: cut-away radius in metres
    view_dir: vec4<f32>,
    // x: strength of the fade at its centre (0 = off, 1 = fully removed)
    settings: vec4<f32>,
}

@group(2) @binding(100) var<uniform> fade: OccluderFade;

fn bayer4(pixel: vec2<u32>) -> f32 {
    var m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return (m[(pixel.y % 4u) * 4u + (pixel.x % 4u)] + 0.5) / 16.0;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
#ifndef PREPASS_PIPELINE
    let to_fragment = in.world_position.xyz - fade.player.xyz;
    // Negative when the fragment is closer to the camera than the player.
    let depth = dot(to_fragment, fade.view_dir.xyz);
    let off_axis = length(to_fragment - fade.view_dir.xyz * depth);
    let radius = fade.view_dir.w;
    let feet = fade.player.w;

    let amount = (1.0 - smoothstep(radius * 0.6, radius, off_axis))
        * smoothstep(0.4, 1.0, -depth)
        * smoothstep(feet + 0.3, feet + 0.6, in.world_position.y)
        * fade.settings.x;
    if amount > bayer4(vec2<u32>(in.position.xy)) {
        discard;
    }
#endif

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
