@group(0) @binding(0)
var render_target_tex: texture_multisampled_2d<f32>;
@group(0) @binding(1)
var render_target_sampler: sampler;

struct Uniform {
    f: f32,
    skip_reprojection: u32,
};
@group(1) @binding(0)
var<uniform> uniform_params: Uniform;


struct RenderTargetVertexInput {
    @location(0) coords: vec2f,
}

struct RenderTargetVertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) coords: vec2f,
}

@vertex
fn vs_main(in: RenderTargetVertexInput) -> RenderTargetVertexOutput {
    var out: RenderTargetVertexOutput;
    out.clip_position = vec4f(in.coords, 0.0, 1.0);
    out.coords = in.coords;
    return out;
}

@fragment
fn fs_main(in: RenderTargetVertexOutput) -> @location(0) vec4f {
    let f = uniform_params.f * f32(1 - uniform_params.skip_reprojection);

    let mag2 = dot(in.coords, in.coords);
    let base = (0.5 * (1.0 + mag2)) * f + 1.0 - f;
    let new_coords = in.coords / base;
    let adjusted = 0.5 * vec2f(1.0 + new_coords.x, 1.0 - new_coords.y);

    let dims = textureDimensions(render_target_tex);
    let ucoords = vec2u(round(adjusted * vec2f(dims) - vec2f(0.5)));

    var tex_out = vec4f(0);
    // TODO make this a uniform?
    for (var i: i32 = 0; i < 16; i += 1) {
        tex_out = tex_out + textureLoad(render_target_tex, ucoords, i);
    }
    tex_out /= 16.0;

    return select(
        select(
            vec4(0.3, 0.3, 0.3, 1.0),
            tex_out,
            bool(uniform_params.skip_reprojection)),
        tex_out,
        mag2 < 1.0);
}