struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x = -1.0 + f32(i32((in_vertex_index & 1u) << 2u));
    let y = -1.0 + f32(i32((in_vertex_index & 2u) << 1u));
    let u = (x+1.0)*0.5;
    let v = (y+1.0)*0.5;

    var out: VertexOutput;
    out.tex_coord = vec2<f32>(u, v);
    out.position = vec4<f32>(x, y * -1, 0.0, 1.0);
    return out;
}

@group(0) @binding(0)
var r_color: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(textureDimensions(r_color).xy) * in.tex_coord;
    let tex = textureLoad(r_color, vec2<i32>(uv), 0);
    return vec4<f32>(tex.x, tex.y, tex.z, 1.0);
}

