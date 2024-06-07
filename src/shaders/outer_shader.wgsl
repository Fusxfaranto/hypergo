@group(0) @binding(0)
var render_target_tex: texture_2d<f32>;
@group(0) @binding(1)
var render_target_sampler: sampler;

struct RenderTargetVertexInput {
    @location(0) coords: vec2<f32>,
}

struct RenderTargetVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: RenderTargetVertexInput) -> RenderTargetVertexOutput {
    var out: RenderTargetVertexOutput;
    out.clip_position = vec4<f32>(in.coords, 0.0, 1.0);
    out.tex_coords = 0.5 * vec2<f32>(1.0 + in.coords.x, 1.0 - in.coords.y);
    return out;
}

@fragment
fn fs_main(in: RenderTargetVertexOutput) -> @location(0) vec4<f32> {
 
    //return vec4<f32>(in.tex_coords, 0.0, 1.0);
    return textureSample(render_target_tex, render_target_sampler, in.tex_coords);
}