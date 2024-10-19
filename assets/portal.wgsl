#import bevy_pbr::{mesh_view_bindings, mesh_bindings, forward_io::VertexOutput}

@group(2) @binding(0)
var texture: texture_2d<f32>;
@group(2) @binding(1)
var texture_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    let dims = textureDimensions(texture);

    // Convert from screen-space coordinates to uv coordinates
    let uv = vec2(in.position.x / f32(dims.x), in.position.y / f32(dims.y));

    let color = textureSample(texture, texture_sampler, uv).rgb;
    return vec4(color, 1.0);
}
