@group(0) @binding(0)
var render_target_tex: texture_2d<f32>;
@group(0) @binding(1)
var render_target_sampler: sampler;

struct Uniform {
    f: f32,
    skip_reprojection: u32,
};
@group(1) @binding(0)
var<uniform> uniform_params: Uniform;


struct RenderTargetVertexInput {
    @location(0) coords: vec2<f32>,
}

struct RenderTargetVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) coords: vec2<f32>,
}

@vertex
fn vs_main(in: RenderTargetVertexInput) -> RenderTargetVertexOutput {
    var out: RenderTargetVertexOutput;
    out.clip_position = vec4<f32>(in.coords, 0.0, 1.0);
    out.coords = in.coords;
    return out;
}

@fragment
fn fs_main(in: RenderTargetVertexOutput) -> @location(0) vec4<f32> {
    if bool(uniform_params.skip_reprojection) {
        let adjusted = 0.5 * vec2f(1.0 + in.coords.x, 1.0 - in.coords.y);
        return textureSample(render_target_tex, render_target_sampler, adjusted);
    }

    let f = uniform_params.f;

    let mag2 = dot(in.coords, in.coords);
    let base = (0.5 * (1.0 + mag2)) * f + 1.0 - f;
    let new_coords = in.coords / base;
    let adjusted = 0.5 * vec2f(1.0 + new_coords.x, 1.0 - new_coords.y);
 
    //return vec4<f32>(in.coords, 0.0, 1.0);
    return select(
        vec4(0.3, 0.3, 0.3, 1.0),
        textureSample(render_target_tex, render_target_sampler, adjusted),
        mag2 < 1.0);
}