
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vertex(
    @builtin(vertex_index) index: u32
) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );

    var output: VertexOutput;

    output.position = vec4<f32>(
        positions[index],
        0.0, 1.0
    );

    return output;
}

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>
) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);

    return textureLoad(
        input_texture,
        pixel,
        0 
    );
}
