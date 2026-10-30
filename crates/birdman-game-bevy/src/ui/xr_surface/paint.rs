use super::*;

impl PaintTarget {
    pub(super) fn node(
        &mut self,
        world: &mut World,
        used: &mut usize,
        parent: Entity,
        node: Node,
    ) -> Entity {
        let entity = if let Some(entity) = self.nodes.get(*used) {
            *entity
        } else {
            let entity = world.spawn_empty().id();
            self.nodes.push(entity);
            entity
        };
        *used += 1;
        world
            .entity_mut(entity)
            .remove::<(
                Text,
                TextFont,
                TextColor,
                BackgroundColor,
                BackgroundGradient,
                BorderColor,
                MaterialNode<DirectionMaterial>,
                MaterialNode<OutlineMaterial>,
                Baseline,
            )>()
            .insert((
                ChildOf(parent),
                node,
                UiTransform::default(),
                Visibility::Inherited,
            ));
        entity
    }

    pub(super) fn paint(
        &mut self,
        world: &mut World,
        items: &[SurfacePaintItem],
        parent: Entity,
        origin: Vec2,
        scale: Vec2,
        used: &mut usize,
    ) -> Result<(), String> {
        let point = |p: MenuPoint| Vec2::new(p.x as f32, p.y as f32) * scale - origin;
        let bounds = |r: MenuRect| Node {
            position_type: PositionType::Absolute,
            left: px(r.x as f32 * scale.x - origin.x),
            top: px(r.y as f32 * scale.y - origin.y),
            width: px(r.width as f32 * scale.x),
            height: px(r.height as f32 * scale.y),
            ..default()
        };
        for item in items {
            match item {
                SurfacePaintItem::Rectangle {
                    bounds: rect,
                    color: ink,
                    opacity,
                    gradient,
                } => {
                    let entity = self.node(world, used, parent, bounds(*rect));
                    world
                        .entity_mut(entity)
                        .insert(BackgroundColor(if gradient.is_some() {
                            Color::NONE
                        } else {
                            color(ink, *opacity)?
                        }));
                    if let Some([left, right]) = gradient {
                        world.entity_mut(entity).insert(BackgroundGradient::from(
                            LinearGradient::to_right(vec![
                                ColorStop::new(color(left, *opacity)?, percent(0)),
                                ColorStop::new(color(right, *opacity)?, percent(100)),
                            ]),
                        ));
                    }
                }
                SurfacePaintItem::RoundedOutline {
                    bounds: rect,
                    radius,
                    color: ink,
                    width_px,
                    opacity,
                } => {
                    let mut node = bounds(*rect);
                    let half = *width_px as f32 * 0.5;
                    node.left = px(rect.x as f32 * scale.x - origin.x - half);
                    node.top = px(rect.y as f32 * scale.y - origin.y - half);
                    node.width = px(rect.width as f32 * scale.x + half * 2.0);
                    node.height = px(rect.height as f32 * scale.y + half * 2.0);
                    let slot = *used;
                    let entity = self.node(world, used, parent, node);
                    let extent = Vec2::new(rect.width as f32, rect.height as f32) * scale;
                    let mut radii = Vec2::new(*radius as f32 * scale.x, *radius as f32 * scale.y);
                    let reduce = (extent.x / (2.0 * radii.x))
                        .min(extent.y / (2.0 * radii.y))
                        .min(1.0);
                    if reduce.is_finite() {
                        radii *= reduce;
                    }
                    let ink = color(ink, *opacity)?.to_linear();
                    let outline = OutlineMaterial {
                        extent_radius: extent.extend(radii.x).extend(radii.y),
                        stroke: Vec4::new(
                            extent.x + half * 2.0,
                            extent.y + half * 2.0,
                            *width_px as f32,
                            0.0,
                        ),
                        ink: Vec4::new(ink.red, ink.green, ink.blue, ink.alpha),
                    };
                    while self.outlines.len() <= slot {
                        self.outlines.push(None);
                    }
                    let handle = self.outlines[slot]
                        .get_or_insert_with(|| {
                            world
                                .resource_mut::<Assets<OutlineMaterial>>()
                                .add(outline.clone())
                        })
                        .clone();
                    *world
                        .resource_mut::<Assets<OutlineMaterial>>()
                        .get_mut(&handle)
                        .ok_or("XR outline material was released")? = outline;
                    world.entity_mut(entity).insert(MaterialNode(handle));
                }
                SurfacePaintItem::Line {
                    start,
                    end,
                    color: ink,
                    width_px,
                    opacity,
                } => {
                    let a = point(*start);
                    let b = point(*end);
                    let delta = b - a;
                    let center = (a + b) * 0.5;
                    let entity = self.node(
                        world,
                        used,
                        parent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(center.x - delta.length() * 0.5),
                            top: px(center.y - *width_px as f32 * 0.5),
                            width: px(delta.length()),
                            height: px(*width_px),
                            ..default()
                        },
                    );
                    world.entity_mut(entity).insert((
                        BackgroundColor(color(ink, *opacity)?),
                        UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
                    ));
                }
                SurfacePaintItem::Marker {
                    point: p,
                    size_px,
                    color: ink,
                } => {
                    let p = point(*p);
                    let radius = *size_px as f32 * 0.5;
                    let entity = self.node(
                        world,
                        used,
                        parent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(p.x - radius),
                            top: px(p.y - radius),
                            width: px(*size_px),
                            height: px(*size_px),
                            ..default()
                        },
                    );
                    world
                        .entity_mut(entity)
                        .insert(BackgroundColor(color(ink, 1.0)?));
                }
                SurfacePaintItem::Text {
                    value,
                    font,
                    font_meters,
                    locale,
                    anchor,
                    rotation_degrees,
                    color: ink,
                    opacity,
                } => {
                    let font = text_font(world, font, locale, font_meters * f64::from(scale.y))
                        .map_err(|error| format!("XR font measurement: {error:?}"))?;
                    let p = point(*anchor);
                    let anchor = self.node(
                        world,
                        used,
                        parent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(p.x),
                            top: px(p.y),
                            width: px(0),
                            height: px(0),
                            ..default()
                        },
                    );
                    world
                        .entity_mut(anchor)
                        .insert(UiTransform::from_rotation(Rot2::radians(
                            rotation_degrees.to_radians() as f32,
                        )));
                    let text = self.node(
                        world,
                        used,
                        anchor,
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                    );
                    world.entity_mut(text).insert((
                        Text::new(value),
                        font,
                        TextColor(color(ink, *opacity)?),
                        TextLayout::no_wrap(),
                        Baseline,
                        Visibility::Hidden,
                    ));
                }
                SurfacePaintItem::Plot { clip, items } => {
                    let clip_entity = self.clip_node(world, *clip, parent, origin, scale, used);
                    self.paint(
                        world,
                        items,
                        clip_entity,
                        Vec2::new(clip.x as f32, clip.y as f32) * scale,
                        scale,
                        used,
                    )?;
                }
                SurfacePaintItem::Triangle {
                    points,
                    color: ink,
                    opacity,
                } => {
                    let [a, b, c] = points.map(point);
                    if (b - a).perp_dot(c - a) == 0.0 {
                        continue;
                    }
                    let min = a.min(b).min(c);
                    let max = a.max(b).max(c);
                    let size = max - min;
                    let slot = *used;
                    let entity = self.node(
                        world,
                        used,
                        parent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(min.x),
                            top: px(min.y),
                            width: px(size.x),
                            height: px(size.y),
                            ..default()
                        },
                    );
                    let [a, b, c] = [a - min, b - min, c - min];
                    let mid = (a + b) * 0.5;
                    let ink = color(ink, *opacity)?.to_linear();
                    let shape = DirectionMaterial {
                        points01: Vec4::new(a.x, a.y, mid.x, mid.y),
                        points23: Vec4::new(b.x, b.y, c.x, c.y),
                        canvas: Vec4::new(size.x, size.y, 0.0, 0.0),
                        color: Vec4::new(ink.red, ink.green, ink.blue, ink.alpha),
                    };
                    while self.shapes.len() <= slot {
                        self.shapes.push(None);
                    }
                    let handle = self.shapes[slot]
                        .get_or_insert_with(|| {
                            world
                                .resource_mut::<Assets<DirectionMaterial>>()
                                .add(DirectionMaterial::default())
                        })
                        .clone();
                    *world
                        .resource_mut::<Assets<DirectionMaterial>>()
                        .get_mut(&handle)
                        .ok_or("XR shape material was released")? = shape;
                    world.entity_mut(entity).insert(MaterialNode(handle));
                }
            }
        }
        Ok(())
    }

    pub(super) fn clip_node(
        &mut self,
        world: &mut World,
        clip: MenuRect,
        parent: Entity,
        origin: Vec2,
        scale: Vec2,
        used: &mut usize,
    ) -> Entity {
        self.node(
            world,
            used,
            parent,
            Node {
                position_type: PositionType::Absolute,
                left: px(clip.x as f32 * scale.x - origin.x),
                top: px(clip.y as f32 * scale.y - origin.y),
                width: px(clip.width as f32 * scale.x),
                height: px(clip.height as f32 * scale.y),
                overflow: Overflow::clip(),
                ..default()
            },
        )
    }
}
