#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip

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
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> grid_lod: u32;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> minmax: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> translation: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var<uniform> wireframe: u32;

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
    let height = height_bilinear(height_uv, 0);

    out.world_position.y = height * (minmax.y - minmax.x) + minmax.x;
    out.position = position_world_to_clip(out.world_position.xyz);

    return out;
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

    let uv = in.world_position.xz / world_size + 0.5 / texture_size + 0.5;
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
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
