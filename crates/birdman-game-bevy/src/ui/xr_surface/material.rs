use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct DepthKey {
    test: bool,
    write: bool,
    textured: bool,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(DepthKey)]
pub(super) struct SurfaceDepth {
    key: DepthKey,
}

impl From<&SurfaceDepth> for DepthKey {
    fn from(value: &SurfaceDepth) -> Self {
        value.key
    }
}

impl MaterialExtension for SurfaceDepth {
    fn fragment_shader() -> ShaderRef {
        SURFACE_SHADER.into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.textured
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment
                .shader_defs
                .push("SURFACE_TEXTURE_PREMULTIPLIED".into());
        }
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_compare = Some(if key.bind_group_data.test {
                CompareFunction::GreaterEqual
            } else {
                CompareFunction::Always
            });
            depth.depth_write_enabled = Some(key.bind_group_data.write);
            depth.bias = default();
        }
        Ok(())
    }
}

pub(super) type SurfaceMaterial = ExtendedMaterial<StandardMaterial, SurfaceDepth>;

pub(super) const SURFACE_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("173d605d-f23b-418c-ab0c-6551cc8def59");

pub(super) const SURFACE_SOURCE: &str = r#"
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
}
@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(input, is_front);
#ifdef SURFACE_TEXTURE_PREMULTIPLIED
    if pbr.material.base_color.a > 0.0 {
        pbr.material.base_color = vec4(pbr.material.base_color.rgb / pbr.material.base_color.a, pbr.material.base_color.a);
    }
#endif
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    var output: FragmentOutput;
    output.color = main_pass_post_lighting_processing(pbr, pbr.material.base_color);
    return output;
}
"#;

pub(super) const OUTLINE_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("909d0142-68eb-4c21-b67b-1dcd4acc838e");

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct OutlineMaterial {
    #[uniform(0)]
    pub(super) extent_radius: Vec4,
    #[uniform(0)]
    pub(super) stroke: Vec4,
    #[uniform(0)]
    pub(super) ink: Vec4,
}

impl UiMaterial for OutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        OUTLINE_SHADER.into()
    }
}

pub(super) const OUTLINE_SOURCE: &str = r#"
#import bevy_ui::ui_vertex_output::UiVertexOutput
struct Outline { extent_radius: vec4<f32>, stroke: vec4<f32>, ink: vec4<f32> }
@group(1) @binding(0) var<uniform> shape: Outline;
fn ellipse_distance(point: vec2<f32>, radius: vec2<f32>) -> f32 {
    var p = abs(point);
    var r = radius;
    if r.x < r.y { p = p.yx; r = r.yx; }
    if r.x == r.y { return length(p) - r.x; }
    let squared = r * r;
    var closest: vec2<f32>;
    if p.y == 0.0 {
        let difference = squared.x - squared.y;
        if r.x * p.x < difference {
            let x = squared.x * p.x / difference;
            closest = vec2(x, r.y * sqrt(max(0.0, 1.0 - x * x / squared.x)));
        } else { closest = vec2(r.x, 0.0); }
    } else {
        var lower = -squared.y;
        var upper = max(0.0, dot(r, p));
        for (var i = 0u; i < 32u; i += 1u) {
            let t = (lower + upper) * 0.5;
            let ratio = r * p / (vec2(t) + squared);
            if dot(ratio, ratio) > 1.0 { lower = t; } else { upper = t; }
        }
        let denominator = vec2(upper) + squared;
        if denominator.y > 0.0 { closest = squared * p / denominator; }
        else {
            let x = min(r.x, squared.x * p.x / (squared.x - squared.y));
            closest = vec2(x, r.y * sqrt(max(0.0, 1.0 - x * x / squared.x)));
        }
    }
    let sign = select(-1.0, 1.0, dot(p / r, p / r) >= 1.0);
    return sign * length(p - closest);
}
@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let half_size = shape.extent_radius.xy * 0.5;
    let radius = shape.extent_radius.zw;
    let point = abs(input.uv * shape.stroke.xy - shape.stroke.xy * 0.5);
    var distance: f32;
    if min(radius.x, radius.y) <= 0.0 {
        let q = point - half_size;
        distance = length(max(q, vec2(0.0))) + min(max(q.x,q.y),0.0);
    } else {
        let q = point - half_size + radius;
        if q.x > 0.0 && q.y > 0.0 { distance = ellipse_distance(q, radius); }
        else { distance = max(q.x - radius.x, q.y - radius.y); }
    }
    let edge = abs(distance) - shape.stroke.z * 0.5;
    let pixel_width = fwidth(edge);
    var coverage = select(0.0,1.0,edge <= 0.0);
    if pixel_width > 0.0 { coverage = clamp(0.5 - edge / pixel_width, 0.0, 1.0); }
    return vec4(shape.ink.rgb,shape.ink.a * coverage);
}
"#;

#[derive(Component, Clone)]
pub(super) struct SurfaceOrder(pub(super) i32);

impl bevy::render::sync_component::SyncComponent for SurfaceOrder {
    type Target = Self;
}
impl ExtractComponent for SurfaceOrder {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;
    fn extract_component(
        item: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(item.clone())
    }
}

pub(super) fn order_surfaces(
    views: Query<&ExtractedView>,
    orders: Query<(&MainEntity, &SurfaceOrder)>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
) {
    for view in &views {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };
        for item in phase.items.values_mut() {
            if let Some((_, order)) = orders.iter().find(|(entity, _)| **entity == item.entity.1) {
                item.sorting_info = TransparentSortingInfo3d::Sorted {
                    mesh_center: view.world_from_view.translation(),
                    depth_bias: order.0 as f32,
                };
            }
        }
    }
}

pub(super) fn material(
    texture: Option<Handle<Image>>,
    ink: Color,
    style: XrSurfaceStyle,
) -> SurfaceMaterial {
    let textured = texture.is_some();
    ExtendedMaterial {
        base: StandardMaterial {
            base_color_texture: texture,
            base_color: ink,
            unlit: true,
            alpha_mode: if style.transparent {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            cull_mode: None,
            fog_enabled: false,
            ..default()
        },
        extension: SurfaceDepth {
            key: DepthKey {
                test: style.depth_test,
                write: style.depth_write,
                textured,
            },
        },
    }
}
