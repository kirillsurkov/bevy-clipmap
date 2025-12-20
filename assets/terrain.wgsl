#import bevy_pbr::mesh_functions
#import bevy_pbr::pbr_types
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::mesh_view_bindings::lights
#import bevy_pbr::mesh_view_bindings::view

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var heightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var heightmap_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var horizon_texture: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var horizon_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> grid_lod: u32;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var<uniform> minmax: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var<uniform> translation: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var<uniform> wireframe: u32;

fn height_bilinear(uv: vec2<f32>, lod: i32) -> f32 {
    let tex_size = vec2<f32>(textureDimensions(heightmap_texture, lod));
    let pos = uv * tex_size;
    let p0 = vec2<i32>(floor(pos));
    let f = pos - floor(pos);

    let h00 = textureLoad(heightmap_texture, p0, lod).r;
    let h10 = textureLoad(heightmap_texture, p0 + vec2(1, 0), lod).r;
    let h01 = textureLoad(heightmap_texture, p0 + vec2(0, 1), lod).r;
    let h11 = textureLoad(heightmap_texture, p0 + vec2(1, 1), lod).r;

    let hx0 = mix(h00, h10, f.x);
    let hx1 = mix(h01, h11, f.x);

    return mix(hx0, hx1, f.y);
}

@vertex
fn vertex(vertex: Vertex, @builtin(vertex_index) idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = model * vec4<f32>(vertex.position, 1.0);

    let texel_size = 1.0;
    let texture_size = vec2<f32>(textureDimensions(heightmap_texture));
    let world_size = texel_size * texture_size;

    let height_uv = out.world_position.xz / world_size + 0.5;
    // let height = height_bilinear(height_uv, 0);
    let height = textureLoad(heightmap_texture, vec2<i32>(height_uv * world_size), 0).r;

    out.world_position.y = height * (minmax.y - minmax.x) + minmax.x;
    out.position = position_world_to_clip(out.world_position.xyz);

    return out;
}

fn reconstruct_horizon(uv: vec2<f32>, theta: f32) -> f32 {
    const N = 360.0;
    const K = 64 / 2;

    var horizon = textureSample(horizon_texture, horizon_sampler, uv, 0).r / N;
    for (var i = 0; i < K; i++) {
        let angle = f32(i + 1) * theta;
        let a = textureSample(horizon_texture, horizon_sampler, uv, i + 1).r;
        let b = textureSample(horizon_texture, horizon_sampler, uv, i + 1 + K).r;
        horizon += (2.0 / N) * (a * cos(angle) - b * sin(angle));
    }
    return clamp(horizon, 0.0, 0.5 * 3.1415926535);
}

fn horizon_ao(uv: vec2<f32>, cnt: u32) -> f32 {
    const TWO_PI = 6.2831853;

    var ao = 0.0;

    for (var j = 0u; j < cnt; j++) {
        let theta = TWO_PI * f32(j) / f32(cnt);
        let h = reconstruct_horizon(uv, theta);
        let ao_dir = cos(h);
        ao += ao_dir * ao_dir;
    }

    return ao / f32(cnt);
}

fn apply_horizon_shadow(in: pbr_types::PbrInput, color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let ao = horizon_ao(uv, 4);
    var shadow = 0.0;
    for (var i = 0u; i < lights.n_directional_lights; i++) {
        let light = &lights.directional_lights[i];
        let sun = light.direction_to_light;
        let theta = atan2(sun.z, sun.x);
        let sun_elev = asin(sun.y);
        let horizon_elev = reconstruct_horizon(uv, theta);
        shadow += smoothstep(horizon_elev - 0.3, horizon_elev + 0.3, sun_elev);
    }
    shadow /= f32(lights.n_directional_lights);

    var emissive_light = in.material.emissive.rgb * in.material.base_color.a;
#ifdef STANDARD_MATERIAL_CLEARCOAT
    let clearcoat_N = in.clearcoat_N;
    let clearcoat_NdotV = max(dot(clearcoat_N, in.V), 0.0001);
    emissive_light = emissive_light * (0.04 + (1.0 - 0.04) * pow(1.0 - clearcoat_NdotV, 5.0));
#endif
    emissive_light = emissive_light * mix(1.0, view.exposure, in.material.emissive.a);
    return vec4((color.rgb - emissive_light) * ao * shadow + emissive_light, in.material.base_color.a);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    if wireframe != 0 {
        var out: FragmentOutput;
        out.color = vec4(1.0);
        return out;
    }

    var in_modified = in;

    let texel_size = 1.0;
    let texture_size = vec2<f32>(textureDimensions(heightmap_texture));
    let world_size = texture_size * texel_size;

    let uv = in.world_position.xz / world_size + 0.5;
    let step = 1.0 / texture_size;
    let h_r = textureSample(heightmap_texture, heightmap_sampler, uv + vec2(step.x, 0.0)).r;
    let h_l = textureSample(heightmap_texture, heightmap_sampler, uv - vec2(step.x, 0.0)).r;
    let h_t = textureSample(heightmap_texture, heightmap_sampler, uv + vec2(0.0, step.y)).r;
    let h_b = textureSample(heightmap_texture, heightmap_sampler, uv - vec2(0.0, step.y)).r;

    let scale = (minmax.y - minmax.x) / (2.0 * texel_size);
    let dh_dx = (h_r - h_l) * scale;
    let dh_dy = (h_t - h_b) * scale;
    in_modified.world_normal = normalize(vec3(-dh_dx, 1.0, -dh_dy));

    var pbr_input = pbr_input_from_standard_material(in_modified, is_front);
    pbr_input.material.perceptual_roughness = 1.0;
    pbr_input.material.base_color = textureSample(color_texture, color_sampler, uv);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in_modified, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = apply_horizon_shadow(pbr_input, out.color, uv);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
