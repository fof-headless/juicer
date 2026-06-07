// Juicer Lite renderer shader.
// One pipeline draws textured/solid, lit/unlit meshes with per-object tint + opacity.

struct Globals {
    view_proj : mat4x4<f32>,
    light_dir : vec4<f32>,   // xyz dir, w = intensity
    light_col : vec4<f32>,   // rgb, w = ambient
};

struct ObjectUniform {
    model    : mat4x4<f32>,
    normal   : mat4x4<f32>,  // inverse-transpose of model (upper 3x3 used)
    tint     : vec4<f32>,    // rgb tint, a = opacity
    flags    : vec4<f32>,    // x = unlit (1/0), y = has_texture (1/0)
};

@group(0) @binding(0) var<uniform> globals : Globals;
@group(1) @binding(0) var<uniform> obj : ObjectUniform;
@group(1) @binding(1) var tex : texture_2d<f32>;
@group(1) @binding(2) var samp : sampler;

struct VsIn {
    @location(0) pos    : vec3<f32>,
    @location(1) normal : vec3<f32>,
    @location(2) uv     : vec2<f32>,
};

struct VsOut {
    @builtin(position) clip : vec4<f32>,
    @location(0) world_normal : vec3<f32>,
    @location(1) uv : vec2<f32>,
};

@vertex
fn vs_main(in : VsIn) -> VsOut {
    var out : VsOut;
    let world = obj.model * vec4<f32>(in.pos, 1.0);
    out.clip = globals.view_proj * world;
    out.world_normal = normalize((obj.normal * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
    var base = obj.tint.rgb;
    var alpha = obj.tint.a;

    if (obj.flags.y > 0.5) {
        let texel = textureSample(tex, samp, in.uv);
        base = texel.rgb * obj.tint.rgb;
        alpha = texel.a * obj.tint.a;
    }

    var color = base;
    if (obj.flags.x < 0.5) {
        // Lit: simple Lambert + ambient.
        let n = normalize(in.world_normal);
        let l = normalize(-globals.light_dir.xyz);
        let diff = max(dot(n, l), 0.0) * globals.light_dir.w;
        let ambient = globals.light_col.w;
        color = base * (ambient + diff) * globals.light_col.rgb;
    }

    return vec4<f32>(color, alpha);
}
