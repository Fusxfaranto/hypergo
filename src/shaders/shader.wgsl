struct Uniform {
    transform: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> vertex_uniform: Uniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
};
struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// TODO consider processing spinors directly in shader, instead of pre-converting to matrix?
// https://tech.metail.com/performance-quaternions-gpu/
@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let instance_transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );
    out.color = instance.color;
    //out.clip_position = vertex_uniform.transform * instance_transform * 
    //    vec4<f32>(model.position, 1.0);
    // var temp_pos = instance_transform * 
    //     vec4<f32>(model.position, 1.0);
    // out.clip_position = vertex_uniform.transform * (temp_pos / temp_pos.w);
    var transform = vertex_uniform.transform * instance_transform;
    out.clip_position = transform * vec4<f32>(model.position.xy, 0.0, model.position.z);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    //return vec4<f32>(in.clip_position.xyz, 1.0);
    return in.color;
}