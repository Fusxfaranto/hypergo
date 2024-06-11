@group(0) @binding(1)
var render_target_sampler: sampler;

struct Uniform {
    f: f32,
    skip_reprojection: u32,
    padding: vec2u,
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

fn world_coords_mag2(v: vec2f, dims: vec2f) -> f32 {
    let world_v = (2 * v + vec2f(1.0)) / dims - vec2f(1.0);
    return dot(world_v, world_v);
}