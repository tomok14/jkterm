struct Uniform {
    transform: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> u_transform: Uniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VSOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VSOutput {
    var out: VSOutput;
    out.clip_pos = u_transform.transform * vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VSOutput) -> @location(0) vec4<f32> {
    return input.color;
}
