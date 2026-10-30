use super::super::{NativeFont, projection::pose_transform};
use super::{ChartTextBaseline as Baseline, DirectionMaterial, color};
use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    ecs::system::SystemState,
    image::ImageSampler,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor,
        SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages, encase,
    },
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_phase::ViewSortedRenderPhases,
        sync_world::MainEntity,
        view::ExtractedView,
    },
    shader::ShaderRef,
    text::{
        ComputedTextBlock, FontAtlasSet, FontCx, FontHinting, LayoutCx, LetterSpacing, LineBreak,
        LineHeight, ScaleCx, TextBounds, TextLayoutInfo, TextPipeline,
    },
    ui::ColorStop,
};
use birdman_game_presentation::{
    camera::Pose,
    xr::{
        MenuFont, MenuFontStyle, MenuLayoutFailure, MenuPoint, MenuRect, MenuSize,
        SurfacePaintItem, XrSurfaceStyle,
    },
};
use birdman_game_runtime::{
    ApplicationGlyphMetrics, ApplicationHeadHudProjection, ApplicationMenuProjection,
    ApplicationSpatialProjection, ApplicationSurfaceObservation, ApplicationTextProbe,
};

#[path = "xr_surface/font.rs"]
mod font;
#[path = "xr_surface/material.rs"]
mod material;
#[path = "xr_surface/paint.rs"]
mod paint;
use font::text_font;
pub(crate) use font::{surface, with_metrics};
use material::*;

#[derive(Default, Resource)]
struct Surfaces {
    menu: Option<PaintTarget>,
    head: Option<PaintTarget>,
    cursors: Vec<Cursor>,
}

struct PaintTarget {
    camera: Entity,
    root: Entity,
    plane: Entity,
    image: Handle<Image>,
    mesh: Handle<Mesh>,
    material: Handle<SurfaceMaterial>,
    nodes: Vec<Entity>,
    shapes: Vec<Option<Handle<DirectionMaterial>>>,
    outlines: Vec<Option<Handle<OutlineMaterial>>>,
}

struct Cursor {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<SurfaceMaterial>,
}

pub(crate) fn install(app: &mut App) {
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SURFACE_SHADER.id(),
            Shader::from_wgsl(SURFACE_SOURCE, "native_xr_surface.wgsl"),
        )
        .expect("fixed shader UUID");
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            OUTLINE_SHADER.id(),
            Shader::from_wgsl(OUTLINE_SOURCE, "native_xr_outline.wgsl"),
        )
        .expect("fixed shader UUID");
    app.init_resource::<Surfaces>()
        .add_plugins(MaterialPlugin::<SurfaceMaterial>::default())
        .add_plugins(UiMaterialPlugin::<OutlineMaterial>::default())
        .add_plugins(ExtractComponentPlugin::<SurfaceOrder>::default());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            Render,
            order_surfaces
                .after(RenderSystems::QueueMeshes)
                .before(RenderSystems::PhaseSort),
        );
    }
}

fn target(world: &mut World, order: isize) -> PaintTarget {
    let image = world.resource_mut::<Assets<Image>>().add(image([1, 1]));
    let camera = world
        .spawn((
            Camera2d,
            Camera {
                order,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
        ))
        .id();
    let root = world.spawn((Node::default(), UiTargetCamera(camera))).id();
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));
    let material = world
        .resource_mut::<Assets<SurfaceMaterial>>()
        .add(material(
            Some(image.clone()),
            Color::WHITE,
            XrSurfaceStyle {
                depth_test: false,
                depth_write: false,
                transparent: true,
                render_order: 0,
            },
        ));
    let plane = world
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Visibility::Hidden,
        ))
        .id();
    PaintTarget {
        camera,
        root,
        plane,
        image,
        mesh,
        material,
        nodes: Vec::new(),
        shapes: Vec::new(),
        outlines: Vec::new(),
    }
}

fn image(size: [u32; 2]) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    image
}

impl PaintTarget {
    fn begin(
        &mut self,
        world: &mut World,
        size: MenuSize,
        canvas: [u32; 2],
        pose: Pose,
        style: XrSurfaceStyle,
        layers: &RenderLayers,
    ) -> Result<(), String> {
        if canvas.contains(&0)
            || !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return Err("XR surface dimensions are invalid".to_owned());
        }
        {
            let mut images = world.resource_mut::<Assets<Image>>();
            let mut current = images
                .get_mut(&self.image)
                .ok_or("XR surface image was released")?;
            if current.width() != canvas[0] || current.height() != canvas[1] {
                *current = image(canvas);
            }
        }
        *world
            .resource_mut::<Assets<SurfaceMaterial>>()
            .get_mut(&self.material)
            .ok_or("XR surface material was released")? =
            material(Some(self.image.clone()), Color::WHITE, style);
        let mut transform = pose_transform(pose);
        transform.scale = Vec3::new(size.width as f32, size.height as f32, 1.0);
        world
            .entity_mut(self.plane)
            .insert((transform, layers.clone(), Visibility::Visible));
        if style.depth_test {
            world.entity_mut(self.plane).remove::<SurfaceOrder>();
        } else {
            world
                .entity_mut(self.plane)
                .insert(SurfaceOrder(style.render_order));
        }
        world.entity_mut(self.root).insert(Node {
            width: px(canvas[0]),
            height: px(canvas[1]),
            ..default()
        });
        for entity in &self.nodes {
            world
                .entity_mut(*entity)
                .remove::<(ChildOf, Baseline)>()
                .insert(Visibility::Hidden);
        }
        Ok(())
    }

    fn release(self, world: &mut World) {
        for entity in self.nodes {
            if let Ok(entity) = world.get_entity_mut(entity) {
                entity.despawn();
            }
        }
        world.despawn(self.root);
        world.despawn(self.camera);
        world.despawn(self.plane);
        world
            .resource_mut::<Assets<Image>>()
            .remove(self.image.id());
        world.resource_mut::<Assets<Mesh>>().remove(self.mesh.id());
        world
            .resource_mut::<Assets<SurfaceMaterial>>()
            .remove(self.material.id());
        for shape in self.shapes.into_iter().flatten() {
            world
                .resource_mut::<Assets<DirectionMaterial>>()
                .remove(shape.id());
        }
        for outline in self.outlines.into_iter().flatten() {
            world
                .resource_mut::<Assets<OutlineMaterial>>()
                .remove(outline.id());
        }
    }
}

pub(crate) fn submit(
    world: &mut World,
    projection: Option<&ApplicationSpatialProjection>,
    layers: RenderLayers,
) -> Result<(), String> {
    let mut state = world
        .remove_resource::<Surfaces>()
        .ok_or("XR surfaces were not installed")?;
    let result = submit_inner(world, &mut state, projection, &layers);
    if result.is_err() {
        for target in [&state.menu, &state.head].into_iter().flatten() {
            world.entity_mut(target.plane).insert(Visibility::Hidden);
            if let Some(mut camera) = world.get_mut::<Camera>(target.camera) {
                camera.is_active = false;
            }
        }
        for cursor in &state.cursors {
            world.entity_mut(cursor.entity).insert(Visibility::Hidden);
        }
    }
    world.insert_resource(state);
    result
}

fn submit_inner(
    world: &mut World,
    state: &mut Surfaces,
    projection: Option<&ApplicationSpatialProjection>,
    layers: &RenderLayers,
) -> Result<(), String> {
    for target in [&state.menu, &state.head].into_iter().flatten() {
        world.entity_mut(target.plane).insert(Visibility::Hidden);
        world
            .entity_mut(target.camera)
            .get_mut::<Camera>()
            .ok_or("XR UI camera missing")?
            .is_active = false;
    }
    for cursor in &state.cursors {
        world.entity_mut(cursor.entity).insert(Visibility::Hidden);
    }
    let Some(projection) = projection else {
        return Ok(());
    };
    if let ApplicationMenuProjection::Ready {
        world_from_panel,
        surface_style,
        canvas_size,
        paint,
        cursors,
    } = &projection.menu
    {
        let target = state.menu.get_or_insert_with(|| target(world, -8));
        target.begin(
            world,
            paint.size,
            *canvas_size,
            *world_from_panel,
            *surface_style,
            layers,
        )?;
        world
            .entity_mut(target.camera)
            .get_mut::<Camera>()
            .ok_or("XR UI camera missing")?
            .is_active = true;
        let scale = Vec2::new(
            canvas_size[0] as f32 / paint.size.width as f32,
            canvas_size[1] as f32 / paint.size.height as f32,
        );
        let mut used = 0;
        let background = target.node(
            world,
            &mut used,
            target.root,
            Node {
                position_type: PositionType::Absolute,
                width: px(canvas_size[0]),
                height: px(canvas_size[1]),
                ..default()
            },
        );
        world
            .entity_mut(background)
            .insert(BackgroundColor(color(&paint.background, paint.opacity)?));
        let clip = target.clip_node(
            world,
            paint.content.clip,
            target.root,
            Vec2::ZERO,
            scale,
            &mut used,
        );
        target.paint(
            world,
            &paint.content.items,
            clip,
            Vec2::new(paint.content.clip.x as f32, paint.content.clip.y as f32) * scale,
            scale,
            &mut used,
        )?;
        target.paint(
            world,
            &paint.footer,
            target.root,
            Vec2::ZERO,
            scale,
            &mut used,
        )?;
        for (index, cursor) in cursors.iter().enumerate() {
            while state.cursors.len() <= index {
                let mesh = world.resource_mut::<Assets<Mesh>>().add(Circle::new(0.5));
                let material = world
                    .resource_mut::<Assets<SurfaceMaterial>>()
                    .add(material(None, Color::WHITE, *surface_style));
                let entity = world
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Visibility::Hidden,
                    ))
                    .id();
                state.cursors.push(Cursor {
                    entity,
                    mesh,
                    material,
                });
            }
            let retained = &state.cursors[index];
            let style = XrSurfaceStyle {
                depth_test: cursor.depth_test,
                depth_write: false,
                transparent: true,
                render_order: cursor.render_order,
            };
            let rgb = cursor.color_srgb;
            *world
                .resource_mut::<Assets<SurfaceMaterial>>()
                .get_mut(&retained.material)
                .ok_or("XR cursor material missing")? = material(
                None,
                Color::srgba(
                    (rgb >> 16 & 255) as f32 / 255.0,
                    (rgb >> 8 & 255) as f32 / 255.0,
                    (rgb & 255) as f32 / 255.0,
                    cursor.opacity as f32,
                ),
                style,
            );
            let mut transform = pose_transform(cursor.world_from_cursor);
            transform.scale = Vec3::splat(cursor.diameter_m as f32);
            world.entity_mut(retained.entity).insert((
                transform,
                layers.clone(),
                Visibility::Visible,
            ));
            if style.depth_test {
                world.entity_mut(retained.entity).remove::<SurfaceOrder>();
            } else {
                world
                    .entity_mut(retained.entity)
                    .insert(SurfaceOrder(style.render_order));
            }
        }
    }
    if let ApplicationHeadHudProjection::Ready {
        world_from_surface,
        surface_style,
        paint,
    } = &projection.head_hud
    {
        let target = state.head.get_or_insert_with(|| target(world, -7));
        target.begin(
            world,
            paint.size,
            paint.canvas_size,
            *world_from_surface,
            *surface_style,
            layers,
        )?;
        world
            .entity_mut(target.camera)
            .get_mut::<Camera>()
            .ok_or("XR UI camera missing")?
            .is_active = true;
        let scale = Vec2::new(
            paint.canvas_size[0] as f32 / paint.size.width as f32,
            paint.canvas_size[1] as f32 / paint.size.height as f32,
        );
        target.paint(world, &paint.items, target.root, Vec2::ZERO, scale, &mut 0)?;
    }
    Ok(())
}

pub(crate) fn release(world: &mut World) {
    if let Some(mut state) = world.remove_resource::<Surfaces>() {
        for target in [state.menu.take(), state.head.take()].into_iter().flatten() {
            target.release(world);
        }
        for cursor in state.cursors.drain(..) {
            world.despawn(cursor.entity);
            world
                .resource_mut::<Assets<Mesh>>()
                .remove(cursor.mesh.id());
            world
                .resource_mut::<Assets<SurfaceMaterial>>()
                .remove(cursor.material.id());
        }
        world.insert_resource(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_font_metrics_and_completed_paint_reach_native_resources() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::text::TextPlugin,
        ))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<SurfaceMaterial>()
        .init_asset::<DirectionMaterial>()
        .init_asset::<OutlineMaterial>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(
                super::super::super::native_assets::bundled_font().to_vec(),
            ));
        app.insert_resource(NativeFont(handle));
        assert!(surface(app.world()).is_none());
        app.update();
        let observation = surface(app.world()).expect("registered embedded font");
        assert_eq!(observation.locale, "und");
        let mut probe = ApplicationTextProbe {
            font: observation.font,
            locale: observation.locale,
            font_size_px: 32.0,
            text: "Ag日本語".to_owned(),
        };
        let metrics = with_metrics(app.world_mut(), |measure| measure(&probe))
            .expect("same engine font pipeline");
        assert!(metrics.advance_px > 0.0 && metrics.right_px > 0.0 && metrics.ascent_px > 0.0);
        assert!(
            [
                metrics.advance_px,
                metrics.left_px,
                metrics.right_px,
                metrics.ascent_px,
                metrics.descent_px
            ]
            .into_iter()
            .all(f64::is_finite)
        );
        probe.locale = "ja-JP".to_owned();
        assert_eq!(
            with_metrics(app.world_mut(), |measure| measure(&probe)).unwrap_err(),
            MenuLayoutFailure::MeasurementUnavailable
        );
        let world = app.world_mut();
        let font = text_font(world, &probe.font, "und", 32.0).unwrap();
        let (block, baseline) = {
            let mut state = SystemState::<(
                Res<Assets<Font>>,
                ResMut<TextPipeline>,
                ResMut<FontCx>,
                ResMut<LayoutCx>,
            )>::new(world);
            let (fonts, mut pipeline, mut font_cx, mut layout_cx) = state.get_mut(world).unwrap();
            let mut block = ComputedTextBlock::default();
            pipeline
                .update_buffer(
                    &fonts,
                    std::iter::once((
                        Entity::PLACEHOLDER,
                        0,
                        "Ag日本語",
                        &font,
                        Color::WHITE,
                        LineHeight::default(),
                        LetterSpacing::default(),
                    )),
                    LineBreak::NoWrap,
                    Justify::Left,
                    TextBounds::UNBOUNDED,
                    2.0,
                    &mut block,
                    &mut font_cx,
                    &mut layout_cx,
                    Vec2::ZERO,
                    16.0,
                )
                .unwrap();
            let baseline = block.buffer().lines().next().unwrap().metrics().baseline;
            (block, baseline)
        };
        let label = world
            .spawn((
                block,
                TextLayoutInfo::default(),
                Baseline,
                Node::default(),
                Visibility::Hidden,
            ))
            .id();
        super::super::align_chart_text_baselines(world);
        assert_eq!(world.get::<Node>(label).unwrap().top, px(-baseline / 2.0));
        assert_eq!(
            *world.get::<Visibility>(label).unwrap(),
            Visibility::Inherited
        );
        world.despawn(label);
        let mut target = target(world, -8);
        let root = target.root;
        let image = target.image.id();
        let mut used = 0;
        target
            .paint(
                world,
                &[
                    SurfacePaintItem::Marker {
                        point: MenuPoint { x: 0.25, y: 0.5 },
                        size_px: 8.0,
                        color: "#ffffff".to_owned(),
                    },
                    SurfacePaintItem::RoundedOutline {
                        bounds: MenuRect {
                            x: 0.0,
                            y: 0.0,
                            width: 0.5,
                            height: 0.5,
                        },
                        radius: 0.1,
                        color: "#ffffff".to_owned(),
                        width_px: 2.0,
                        opacity: 0.75,
                    },
                ],
                root,
                Vec2::ZERO,
                Vec2::new(100.0, 200.0),
                &mut used,
            )
            .expect("completed paint");
        assert_eq!(used, 2);
        let marker = world.get::<Node>(target.nodes[0]).unwrap();
        assert_eq!(
            (marker.width, marker.height, marker.left, marker.top),
            (px(8), px(8), px(21), px(96))
        );
        assert_eq!(marker.border_radius, BorderRadius::ZERO);
        let outline = world
            .resource::<Assets<OutlineMaterial>>()
            .get(target.outlines[1].as_ref().unwrap())
            .unwrap();
        assert_eq!(outline.extent_radius, Vec4::new(50.0, 100.0, 10.0, 20.0));
        assert_eq!(outline.stroke, Vec4::new(52.0, 102.0, 2.0, 0.0));
        assert!(target.outlines[0].is_none());
        assert_eq!(world.resource::<Assets<OutlineMaterial>>().len(), 1);
        let entities = target.nodes.clone();
        target.release(world);
        assert!(world.resource::<Assets<Image>>().get(image).is_none());
        assert!(
            entities
                .into_iter()
                .all(|entity| world.get_entity(entity).is_err())
        );
    }
}
