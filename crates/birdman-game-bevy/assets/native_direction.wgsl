#import bevy_ui::ui_vertex_output::UiVertexOutput

struct DirectionMaterial {
    points01: vec4<f32>,
    points23: vec4<f32>,
    canvas: vec4<f32>,
    color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> material: DirectionMaterial;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let points = array<vec2<f32>, 4>(
        material.points01.xy, material.points01.zw,
        material.points23.xy, material.points23.zw,
    );
    let point = in.uv * material.canvas.xy;
    var distance = length(point - points[0]);
    var inside = false;
    for (var index = 0u; index < 4u; index += 1u) {
        let start = points[index];
        let end = points[(index + 1u) % 4u];
        let edge = end - start;
        let along = clamp(dot(point - start, edge) / dot(edge, edge), 0.0, 1.0);
        distance = min(distance, length(point - start - along * edge));
        if ((start.y > point.y) != (end.y > point.y)) {
            let crossing = start.x + (point.y - start.y) * edge.x / edge.y;
            if (point.x < crossing) {
                inside = !inside;
            }
        }
    }
    let signed_distance = select(distance, -distance, inside);
    let pixel_width = fwidth(signed_distance);
    var coverage = select(0.0, 1.0, inside);
    if (pixel_width > 0.0) {
        coverage = clamp(0.5 - signed_distance / pixel_width, 0.0, 1.0);
    }
    return vec4(material.color.rgb, material.color.a * coverage);
}
