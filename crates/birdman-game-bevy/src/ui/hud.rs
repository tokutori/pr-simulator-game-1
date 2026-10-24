use super::super::NativeFont;
use super::{DirectionMaterial, color, retained_node, retained_ui_transform, set_text, spawn_text};
use bevy::{picking::Pickable, prelude::*};
use birdman_game_presentation::game_ui::{
    GameUiChartPoint, GameUiHud, GameUiHudNode, GameUiHudNodeContent, GameUiHudTone, GameUiTheme,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct RetainedHud {
    pub(super) shell: Entity,
    pub(super) previous: Option<GameUiHud>,
    groups: BTreeMap<String, Entity>,
    nodes: BTreeMap<String, HudNode>,
}

struct HudNode {
    card: Entity,
    value: Entity,
    widget: HudWidget,
}

enum HudWidget {
    Text,
    Attitude {
        surface: Entity,
        horizon: Entity,
        outline: Entity,
        aircraft: [Entity; 6],
        marker: Entity,
        ticks: Box<[(Entity, Entity, Entity); 7]>,
    },
    HeadingTape {
        surface: Entity,
        canvas: Entity,
        pointer: Entity,
        ticks: Vec<(Entity, Entity, Entity)>,
    },
    Direction {
        surface: Entity,
        canvas: Entity,
        ring: Entity,
        cross: [Entity; 2],
        north: (Entity, Entity),
        arrow: Entity,
    },
    LinearGauge {
        surface: Entity,
        canvas: Entity,
        track: Entity,
        ticks: [Entity; 7],
        indicator: Entity,
        minimum: (Entity, Entity),
        maximum: (Entity, Entity),
    },
}

impl HudWidget {
    fn surface(&self) -> Option<Entity> {
        match self {
            Self::Text => None,
            Self::Attitude { surface, .. }
            | Self::HeadingTape { surface, .. }
            | Self::Direction { surface, .. }
            | Self::LinearGauge { surface, .. } => Some(*surface),
        }
    }

    fn accepts(&self, content: &GameUiHudNodeContent) -> bool {
        matches!(
            (self, content),
            (Self::Text, GameUiHudNodeContent::Text { .. })
                | (Self::Attitude { .. }, GameUiHudNodeContent::Attitude { .. })
                | (
                    Self::HeadingTape { .. },
                    GameUiHudNodeContent::HeadingTape { .. }
                )
                | (
                    Self::Direction { .. },
                    GameUiHudNodeContent::Direction { .. }
                )
                | (
                    Self::LinearGauge { .. },
                    GameUiHudNodeContent::LinearGauge { .. }
                )
        )
    }
}

pub(super) fn setup_hud(commands: &mut Commands) -> RetainedHud {
    let shell = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ))
        .id();
    RetainedHud {
        shell,
        previous: None,
        groups: BTreeMap::new(),
        nodes: BTreeMap::new(),
    }
}

pub(super) fn project_hud(
    world: &mut World,
    retained: &mut RetainedHud,
    hud: Option<&GameUiHud>,
    theme: &GameUiTheme,
) -> Result<(), String> {
    let Some(hud) = hud else {
        retained_node(world, retained.shell)
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(Display::None);
        retained.previous = None;
        return Ok(());
    };
    if retained.previous.as_ref() == Some(hud) {
        return Ok(());
    }
    let font = world.resource::<NativeFont>().0.clone();
    let ink = color(&theme.text, 1.0)?;
    let muted = color(&theme.muted_text, 1.0)?;
    let background = color(&theme.panel_background, hud.layout.opacity)?;
    if retained
        .previous
        .as_ref()
        .is_none_or(|previous| !same_structure(previous, hud))
    {
        project_structure(world, retained, hud, &font, background);
    }
    for group in &hud.groups {
        for node in &group.nodes {
            let retained = retained
                .nodes
                .get_mut(&node.id)
                .expect("projected HUD node exists");
            set_text(
                world,
                retained.value,
                if !node.label.is_empty() {
                    format!("{} {}", node.label, node.content.value())
                } else {
                    node.content.value().to_owned()
                },
            );
            let tone = match &node.content {
                GameUiHudNodeContent::Text { tone, .. } => *tone,
                GameUiHudNodeContent::Attitude { .. }
                | GameUiHudNodeContent::HeadingTape { .. }
                | GameUiHudNodeContent::Direction { .. }
                | GameUiHudNodeContent::LinearGauge { .. } => GameUiHudTone::Normal,
            };
            let tone_color = match tone {
                GameUiHudTone::Normal => ink,
                GameUiHudTone::Muted => muted,
                GameUiHudTone::Warning => Color::srgb(1.0, 0.6, 0.4),
            };
            world
                .get_mut::<TextColor>(retained.value)
                .expect("HUD text color exists")
                .set_if_neq(TextColor(tone_color));
            project_widget(
                world,
                retained,
                node,
                hud,
                &font,
                [ink, color(&theme.focus_outline, 1.0)?, muted],
            )?;
        }
    }
    retained.previous = Some(hud.clone());
    Ok(())
}

fn same_structure(previous: &GameUiHud, next: &GameUiHud) -> bool {
    previous.layout == next.layout
        && previous.groups.len() == next.groups.len()
        && previous
            .groups
            .iter()
            .zip(&next.groups)
            .all(|(previous, next)| {
                previous.id == next.id
                    && previous.placement == next.placement
                    && previous.nodes.len() == next.nodes.len()
                    && previous
                        .nodes
                        .iter()
                        .zip(&next.nodes)
                        .all(|(previous, next)| {
                            previous.id == next.id
                                && previous.bounds == next.bounds
                                && previous.instrument_height_em == next.instrument_height_em
                                && std::mem::discriminant(&previous.content)
                                    == std::mem::discriminant(&next.content)
                        })
            })
}

fn project_structure(
    world: &mut World,
    retained: &mut RetainedHud,
    hud: &GameUiHud,
    font: &Handle<Font>,
    background: Color,
) {
    let layout = &hud.layout;
    retained_node(world, retained.shell).set_if_neq(Node {
        position_type: PositionType::Absolute,
        left: px(layout.offset_px[0]),
        bottom: px(layout.offset_px[1]),
        width: px(layout.width_px),
        height: px(layout.height_px),
        ..default()
    });
    world
        .get_mut::<BackgroundColor>(retained.shell)
        .expect("HUD background exists")
        .set_if_neq(BackgroundColor(Color::NONE));
    let mut group_ids = BTreeSet::new();
    let mut node_ids = BTreeSet::new();
    let mut group_entities = Vec::new();
    for group in &hud.groups {
        group_ids.insert(group.id.clone());
        let entity = *retained
            .groups
            .entry(group.id.clone())
            .or_insert_with(|| shape(world, retained.shell, Node::default(), Color::NONE));
        retained_node(world, entity).set_if_neq(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            ..default()
        });
        group_entities.push(entity);
        let mut node_entities = Vec::new();
        for node in &group.nodes {
            node_ids.insert(node.id.clone());
            if retained
                .nodes
                .get(&node.id)
                .is_some_and(|retained| !retained.widget.accepts(&node.content))
            {
                let previous = retained
                    .nodes
                    .remove(&node.id)
                    .expect("retained node exists");
                world.despawn(previous.card);
            }
            let retained_node = retained
                .nodes
                .entry(node.id.clone())
                .or_insert_with(|| spawn_node(world, entity, node, font));
            world.entity_mut(retained_node.card).insert(ChildOf(entity));
            let boxed = !matches!(node.content, GameUiHudNodeContent::Text { .. });
            world
                .get_mut::<Node>(retained_node.card)
                .expect("HUD card exists")
                .set_if_neq(Node {
                    position_type: PositionType::Absolute,
                    left: percent(node.bounds.x * 100.0),
                    top: percent(node.bounds.y * 100.0),
                    min_width: px(0),
                    width: percent(node.bounds.width * 100.0),
                    height: percent(node.bounds.height * 100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    padding: if boxed {
                        UiRect::axes(px(layout.padding_px[0]), px(layout.padding_px[1]))
                    } else {
                        UiRect::ZERO
                    },
                    ..default()
                });
            world
                .get_mut::<BackgroundColor>(retained_node.card)
                .expect("HUD card background exists")
                .set_if_neq(BackgroundColor(if boxed {
                    background
                } else {
                    Color::NONE
                }));
            world
                .get_mut::<TextFont>(retained_node.value)
                .expect("HUD text font exists")
                .map_unchanged(|font| &mut font.font_size)
                .set_if_neq(FontSize::Px(layout.font_size_px as f32));
            if let Some(surface) = retained_node.widget.surface() {
                let surface_width = match retained_node.widget {
                    HudWidget::Attitude { .. } => {
                        px(node.bounds.width * layout.width_px - layout.padding_px[0] * 2.0)
                    }
                    HudWidget::Direction { .. }
                    | HudWidget::HeadingTape { .. }
                    | HudWidget::LinearGauge { .. } => percent(100),
                    HudWidget::Text => unreachable!(),
                };
                let mut surface = world.get_mut::<Node>(surface).expect("HUD surface exists");
                surface.width = surface_width;
                surface.height = px(node.instrument_height_em * layout.font_size_px);
                surface.align_self = AlignSelf::Center;
                surface.flex_shrink = 0.0;
            }
            node_entities.push(retained_node.card);
        }
        world.entity_mut(entity).replace_children(&node_entities);
    }
    retained.nodes.retain(|id, node| {
        if node_ids.contains(id) {
            true
        } else {
            world.despawn(node.card);
            false
        }
    });
    retained.groups.retain(|id, entity| {
        if group_ids.contains(id) {
            true
        } else {
            world.despawn(*entity);
            false
        }
    });
    world
        .entity_mut(retained.shell)
        .replace_children(&group_entities);
}

fn shape(world: &mut World, parent: Entity, node: Node, fill: Color) -> Entity {
    world
        .spawn((
            ChildOf(parent),
            node,
            BackgroundColor(fill),
            Pickable::IGNORE,
        ))
        .id()
}

fn text(world: &mut World, parent: Entity, font: &Handle<Font>, size: f32) -> Entity {
    let entity = spawn_text(world, parent, "", font, size, Color::WHITE);
    world.entity_mut(entity).insert(Pickable::IGNORE);
    entity
}

fn spawn_node(
    world: &mut World,
    parent: Entity,
    node: &GameUiHudNode,
    font: &Handle<Font>,
) -> HudNode {
    let card = shape(world, parent, Node::default(), Color::NONE);
    let value = text(world, card, font, 12.0);
    let widget = match &node.content {
        GameUiHudNodeContent::Text { .. } => HudWidget::Text,
        GameUiHudNodeContent::Attitude { .. } => {
            let surface = shape(
                world,
                card,
                Node {
                    overflow: Overflow::clip(),
                    align_self: AlignSelf::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                Color::NONE,
            );
            let horizon = shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                Color::NONE,
            );
            let ticks = Box::new(std::array::from_fn(|_| {
                let (anchor, label) = instrument_text(world, horizon, font);
                (
                    shape(world, horizon, Node::default(), Color::WHITE),
                    anchor,
                    label,
                )
            }));
            let outline = shape(world, horizon, Node::default(), Color::NONE);
            world
                .entity_mut(outline)
                .insert(BorderColor::all(Color::WHITE));
            let aircraft =
                std::array::from_fn(|_| shape(world, horizon, Node::default(), Color::WHITE));
            let marker = shape(
                world,
                horizon,
                Node {
                    position_type: PositionType::Absolute,
                    display: Display::None,
                    left: percent(50),
                    top: percent(50),
                    width: px(7),
                    height: px(7),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Color::NONE,
            );
            world.entity_mut(marker).insert((
                UiTransform::from_translation(Val2::percent(-50, -50)),
                BorderColor::all(Color::srgb(0.490, 0.957, 0.773)),
            ));
            HudWidget::Attitude {
                surface,
                horizon,
                outline,
                aircraft,
                marker,
                ticks,
            }
        }
        GameUiHudNodeContent::HeadingTape { .. } => {
            let surface = shape(
                world,
                card,
                Node {
                    width: percent(100),
                    height: px(34),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Color::NONE,
            );
            let canvas = shape(world, surface, Node::default(), Color::NONE);
            let pointer = shape(world, canvas, Node::default(), Color::NONE);
            HudWidget::HeadingTape {
                surface,
                canvas,
                pointer,
                ticks: Vec::new(),
            }
        }
        GameUiHudNodeContent::Direction { .. } => {
            let surface = shape(
                world,
                card,
                Node {
                    align_self: AlignSelf::Center,
                    ..default()
                },
                Color::NONE,
            );
            let canvas = shape(world, surface, Node::default(), Color::NONE);
            let ring = shape(world, canvas, Node::default(), Color::NONE);
            let cross = std::array::from_fn(|_| shape(world, canvas, Node::default(), Color::NONE));
            let north = instrument_text(world, canvas, font);
            let material = world
                .resource_mut::<Assets<DirectionMaterial>>()
                .add(DirectionMaterial::default());
            let arrow = shape(
                world,
                canvas,
                Node {
                    display: Display::None,
                    ..default()
                },
                Color::NONE,
            );
            world.entity_mut(arrow).insert(MaterialNode(material));
            HudWidget::Direction {
                surface,
                canvas,
                ring,
                cross,
                north,
                arrow,
            }
        }
        GameUiHudNodeContent::LinearGauge { .. } => {
            let surface = shape(
                world,
                card,
                Node {
                    width: percent(100),
                    height: px(34),
                    ..default()
                },
                Color::NONE,
            );
            let canvas = shape(world, surface, Node::default(), Color::NONE);
            let track = shape(world, canvas, Node::default(), Color::NONE);
            let ticks = std::array::from_fn(|_| shape(world, canvas, Node::default(), Color::NONE));
            let minimum = instrument_text(world, canvas, font);
            let maximum = instrument_text(world, canvas, font);
            let indicator = shape(world, canvas, Node::default(), Color::NONE);
            HudWidget::LinearGauge {
                surface,
                canvas,
                track,
                ticks,
                indicator,
                minimum,
                maximum,
            }
        }
    };
    HudNode {
        card,
        value,
        widget,
    }
}

fn adi_line(
    world: &mut World,
    entity: Entity,
    segment: Option<[GameUiChartPoint; 2]>,
    width: f64,
    ink: Color,
) {
    let Some([start, end]) = segment else {
        retained_node(world, entity).display = Display::None;
        return;
    };
    let start = Vec2::new(start.x as f32, start.y as f32);
    let end = Vec2::new(end.x as f32, end.y as f32);
    super::chart::set_chart_line(world, entity, start, end, ink, width as f32);
}

fn instrument_text(world: &mut World, parent: Entity, font: &Handle<Font>) -> (Entity, Entity) {
    let anchor = shape(world, parent, Node::default(), Color::NONE);
    let label = text(world, anchor, font, 12.0);
    world
        .entity_mut(label)
        .insert((TextLayout::no_wrap(), super::ChartTextBaseline));
    (anchor, label)
}

fn fit_instrument(
    world: &mut World,
    canvas: Entity,
    source: &GameUiHudNode,
    hud: &GameUiHud,
    size: [f64; 2],
) -> f64 {
    let height = source.instrument_height_em * hud.layout.font_size_px;
    let width = source.bounds.width * hud.layout.width_px - hud.layout.padding_px[0] * 2.0;
    let scale = (width / size[0]).min(height / size[1]);
    retained_node(world, canvas).set_if_neq(Node {
        position_type: PositionType::Absolute,
        width: px(size[0]),
        height: px(size[1]),
        left: px((width - size[0]) / 2.0),
        top: px((height - size[1]) / 2.0),
        ..default()
    });
    retained_ui_transform(world, canvas).set_if_neq(UiTransform {
        scale: Vec2::splat(scale as f32),
        ..default()
    });
    scale
}

fn project_instrument_label(
    world: &mut World,
    (anchor, label): (Entity, Entity),
    point: Option<GameUiChartPoint>,
    value: &str,
    (align, rotation, size, ink): (f32, f32, f64, Color),
) {
    let position = point.unwrap_or(GameUiChartPoint { x: 0.0, y: 0.0 });
    retained_node(world, anchor).set_if_neq(Node {
        position_type: PositionType::Absolute,
        display: if point.is_some() {
            Display::Flex
        } else {
            Display::None
        },
        left: px(position.x),
        top: px(position.y),
        width: px(0),
        height: px(0),
        ..default()
    });
    retained_ui_transform(world, anchor)
        .set_if_neq(UiTransform::from_rotation(Rot2::degrees(rotation)));
    retained_node(world, label).position_type = PositionType::Absolute;
    retained_ui_transform(world, label)
        .set_if_neq(UiTransform::from_translation(Val2::percent(align, 0)));
    set_text(world, label, value.to_owned());
    world
        .get_mut::<TextColor>(label)
        .expect("instrument label color exists")
        .set_if_neq(TextColor(ink));
    world
        .get_mut::<TextFont>(label)
        .expect("instrument label font exists")
        .map_unchanged(|font| &mut font.font_size)
        .set_if_neq(FontSize::Px(size as f32));
}

fn project_triangle(
    world: &mut World,
    entity: Entity,
    points: Option<[GameUiChartPoint; 3]>,
    ink: Color,
) {
    let Some([tip, left, right]) = points else {
        retained_node(world, entity).display = Display::None;
        return;
    };
    let height = (tip.y - left.y).abs();
    let downward = tip.y > left.y;
    retained_node(world, entity).set_if_neq(Node {
        position_type: PositionType::Absolute,
        box_sizing: BoxSizing::ContentBox,
        left: px(left.x),
        top: px(tip.y.min(left.y)),
        width: px(0),
        height: px(0),
        border: UiRect {
            left: px(tip.x - left.x),
            right: px(right.x - tip.x),
            top: px(if downward { height } else { 0.0 }),
            bottom: px(if downward { 0.0 } else { height }),
        },
        ..default()
    });
    world.entity_mut(entity).insert(BorderColor {
        top: if downward { ink } else { Color::NONE },
        bottom: if downward { Color::NONE } else { ink },
        left: Color::NONE,
        right: Color::NONE,
    });
}

fn project_widget(
    world: &mut World,
    node: &mut HudNode,
    source: &GameUiHudNode,
    hud: &GameUiHud,
    font: &Handle<Font>,
    [ink, focus, muted]: [Color; 3],
) -> Result<(), String> {
    match &source.content {
        GameUiHudNodeContent::Text { .. } => {}
        GameUiHudNodeContent::Attitude { geometry, .. } => {
            let HudWidget::Attitude {
                surface,
                horizon,
                outline,
                aircraft,
                marker,
                ticks,
            } = &node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            let (surface, horizon, outline, aircraft, marker, ticks) =
                (*surface, *horizon, *outline, *aircraft, *marker, **ticks);
            let layer = geometry.horizon;
            let scale = fit_instrument(world, horizon, source, hud, geometry.canvas_size);
            world
                .get_mut::<BackgroundColor>(surface)
                .expect("ADI background exists")
                .set_if_neq(BackgroundColor(Color::NONE));
            retained_node(world, outline).set_if_neq(Node {
                position_type: PositionType::Absolute,
                left: px(geometry.clip.x - 1.5),
                top: px(geometry.clip.y - 1.5),
                width: px(geometry.clip.width + 3.0),
                height: px(geometry.clip.height + 3.0),
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::all(px(geometry.clip_radius + 1.5)),
                ..default()
            });
            world.entity_mut(outline).insert(BorderColor::all(ink));
            for (entity, segment) in aircraft.into_iter().zip(geometry.aircraft_segments) {
                adi_line(world, entity, Some(segment), 5.0, focus);
            }
            for (line, anchor, _) in ticks {
                retained_node(world, line).display = Display::None;
                retained_node(world, anchor).display = Display::None;
            }
            if let Some(layer) = layer {
                for ((line, anchor, label), tick) in ticks.into_iter().zip(layer.pitch_ticks) {
                    adi_line(world, line, tick.segment, 2.0, ink);
                    project_instrument_label(
                        world,
                        (anchor, label),
                        tick.label_anchor.filter(|_| tick.label_degrees.is_some()),
                        &tick
                            .label_degrees
                            .map_or_else(String::new, |value| value.to_string()),
                        (
                            -50.0,
                            layer.label_rotation_degrees as f32,
                            hud.layout.font_size_px / scale,
                            ink,
                        ),
                    );
                }
            }
            retained_node(world, marker)
                .map_unchanged(|node| &mut node.display)
                .set_if_neq(if geometry.flight_path_center.is_some() {
                    Display::Flex
                } else {
                    Display::None
                });
            if let Some(center) = geometry.flight_path_center {
                let radius = geometry.flight_path_radius;
                retained_node(world, marker).set_if_neq(Node {
                    position_type: PositionType::Absolute,
                    left: px(center.x),
                    top: px(center.y),
                    width: px(radius * 2.0 + 3.0),
                    height: px(radius * 2.0 + 3.0),
                    border: UiRect::all(px(3.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                });
            }
        }
        GameUiHudNodeContent::HeadingTape {
            canvas_size,
            ticks,
            pointer,
            ..
        } => {
            let HudWidget::HeadingTape {
                canvas,
                pointer: marker,
                ticks: entities,
                ..
            } = &mut node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            let scale = fit_instrument(world, *canvas, source, hud, *canvas_size);
            project_triangle(world, *marker, Some(*pointer), focus);
            while entities.len() > ticks.len() {
                let (line, anchor, _) = entities.pop().expect("extra tick exists");
                world.despawn(line);
                world.despawn(anchor);
            }
            while entities.len() < ticks.len() {
                let line = shape(world, *canvas, Node::default(), ink);
                let (anchor, label) = instrument_text(world, *canvas, font);
                entities.push((line, anchor, label));
            }
            for ((line, anchor, label), tick) in entities.iter().copied().zip(ticks) {
                adi_line(world, line, Some(tick.segment), tick.stroke_width, ink);
                project_instrument_label(
                    world,
                    (anchor, label),
                    tick.label_anchor,
                    &tick.label,
                    (-50.0, 0.0, hud.layout.font_size_px / scale, ink),
                );
            }
        }
        GameUiHudNodeContent::Direction {
            canvas_size,
            circle_bounds,
            circle_radius,
            circle_stroke_width,
            cross_segments,
            cross_stroke_width,
            north_label,
            north_anchor,
            arrow,
            ..
        } => {
            let HudWidget::Direction {
                canvas,
                ring,
                cross,
                north,
                arrow: arrow_entity,
                ..
            } = node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            let scale = fit_instrument(world, canvas, source, hud, *canvas_size);
            let half_stroke = circle_stroke_width / 2.0;
            retained_node(world, ring).set_if_neq(Node {
                position_type: PositionType::Absolute,
                left: px(circle_bounds.x - half_stroke),
                top: px(circle_bounds.y - half_stroke),
                width: px(circle_bounds.width + circle_stroke_width),
                height: px(circle_bounds.height + circle_stroke_width),
                border: UiRect::all(px(*circle_stroke_width)),
                border_radius: BorderRadius::all(px(circle_radius + half_stroke)),
                ..default()
            });
            world.entity_mut(ring).insert(BorderColor::all(muted));
            for (entity, segment) in cross.into_iter().zip(cross_segments) {
                adi_line(world, entity, Some(*segment), *cross_stroke_width, muted);
            }
            project_instrument_label(
                world,
                north,
                Some(*north_anchor),
                north_label,
                (-50.0, 0.0, hud.layout.font_size_px / scale, ink),
            );
            retained_node(world, arrow_entity).set_if_neq(Node {
                position_type: PositionType::Absolute,
                width: px(canvas_size[0]),
                height: px(canvas_size[1]),
                display: if arrow.is_some() {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            });
            if let Some(points) = arrow {
                let material = DirectionMaterial {
                    points01: Vec4::new(
                        points[0].x as f32,
                        points[0].y as f32,
                        points[1].x as f32,
                        points[1].y as f32,
                    ),
                    points23: Vec4::new(
                        points[2].x as f32,
                        points[2].y as f32,
                        points[3].x as f32,
                        points[3].y as f32,
                    ),
                    canvas: Vec4::new(canvas_size[0] as f32, canvas_size[1] as f32, 0.0, 0.0),
                    color: focus.to_linear().to_vec4(),
                };
                let id = world
                    .get::<MaterialNode<DirectionMaterial>>(arrow_entity)
                    .expect("Direction material")
                    .0
                    .id();
                let mut materials = world.resource_mut::<Assets<DirectionMaterial>>();
                if materials.get(id) != Some(&material) {
                    *materials.get_mut(id).expect("retained Direction material") = material;
                }
            }
        }
        GameUiHudNodeContent::LinearGauge {
            minimum_label,
            maximum_label,
            canvas_size,
            track,
            ticks,
            label_anchors,
            marker,
            ..
        } => {
            let HudWidget::LinearGauge {
                canvas,
                track: track_entity,
                ticks: tick_entities,
                indicator,
                minimum,
                maximum,
                ..
            } = node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            let scale = fit_instrument(world, canvas, source, hud, *canvas_size);
            adi_line(world, track_entity, Some(*track), 3.0, muted);
            for (index, entity) in tick_entities.into_iter().enumerate() {
                adi_line(world, entity, ticks.get(index).copied(), 1.0, muted);
            }
            project_triangle(world, indicator, *marker, focus);
            project_instrument_label(
                world,
                minimum,
                Some(label_anchors[0]),
                minimum_label,
                (0.0, 0.0, hud.layout.font_size_px / scale, ink),
            );
            project_instrument_label(
                world,
                maximum,
                Some(label_anchors[1]),
                maximum_label,
                (-100.0, 0.0, hud.layout.font_size_px / scale, ink),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_presentation::game_ui::{GameUiHudAdiGeometry, GameUiHudLayout, GameUiRect};

    #[test]
    fn transparent_flight_path_widget_applies_final_shared_geometry_and_retains_its_entities() {
        let mut world = World::new();
        world.init_resource::<Assets<DirectionMaterial>>();
        let parent = world.spawn_empty().id();
        let mut source = GameUiHudNode {
            id: "hud-attitude".into(),
            label: String::new(),
            bounds: GameUiRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            instrument_height_em: 4.0,
            content: GameUiHudNodeContent::Attitude {
                value: String::new(),
                geometry: Box::new(GameUiHudAdiGeometry {
                    canvas_size: [240.0, 180.0],
                    clip: GameUiRect {
                        x: 26.0,
                        y: 18.0,
                        width: 188.0,
                        height: 144.0,
                    },
                    clip_radius: 72.0,
                    horizon: None,
                    aircraft_segments: [[GameUiChartPoint { x: 0.0, y: 0.0 }; 2]; 6],
                    flight_path_center: Some(GameUiChartPoint { x: 120.0, y: 45.0 }),
                    flight_path_radius: 5.0,
                }),
            },
        };
        let font = Handle::default();
        let mut widget = spawn_node(&mut world, parent, &source, &font);
        let hud = GameUiHud {
            layout: GameUiHudLayout {
                offset_px: [12.0; 2],
                width_px: 800.0,
                height_px: 100.0,
                padding_px: [6.0; 2],
                gap_px: [4.0; 2],
                attitude_size_px: [80.0, 48.0],
                font_size_px: 12.0,
                opacity: 0.48,
            },
            groups: Vec::new(),
        };
        project_widget(
            &mut world,
            &mut widget,
            &source,
            &hud,
            &font,
            [Color::WHITE; 3],
        )
        .unwrap();
        let HudWidget::Attitude {
            surface,
            horizon,
            marker,
            ..
        } = widget.widget
        else {
            unreachable!()
        };
        assert_eq!(
            world.get::<BackgroundColor>(surface).unwrap().0,
            Color::NONE
        );
        assert_eq!(world.get::<Node>(horizon).unwrap().width, px(240));
        assert_eq!(world.get::<Node>(marker).unwrap().top, px(45));
        let GameUiHudNodeContent::Attitude { geometry, .. } = &mut source.content else {
            unreachable!()
        };
        geometry.flight_path_center = Some(GameUiChartPoint { x: 120.0, y: 135.0 });
        project_widget(
            &mut world,
            &mut widget,
            &source,
            &hud,
            &font,
            [Color::WHITE; 3],
        )
        .unwrap();
        assert_eq!(world.get::<Node>(marker).unwrap().top, px(135));
        let GameUiHudNodeContent::Attitude { geometry, .. } = &mut source.content else {
            unreachable!()
        };
        geometry.horizon = Some(
            birdman_game_presentation::game_ui::GameUiHudAttitudeGeometry {
                label_rotation_degrees: 0.0,
                pitch_ticks: std::array::from_fn(|_| {
                    birdman_game_presentation::game_ui::GameUiHudPitchTick {
                        segment: Some([
                            GameUiChartPoint { x: 26.0, y: 112.0 },
                            GameUiChartPoint { x: 214.0, y: 112.0 },
                        ]),
                        label_anchor: Some(GameUiChartPoint { x: 165.0, y: 116.0 }),
                        label_degrees: Some(20),
                    }
                }),
            },
        );
        project_widget(
            &mut world,
            &mut widget,
            &source,
            &hud,
            &font,
            [Color::WHITE; 3],
        )
        .unwrap();
        let HudWidget::Attitude { ticks, .. } = widget.widget else {
            unreachable!()
        };
        assert_eq!(world.get::<Node>(ticks[3].0).unwrap().width, px(188));
        assert_eq!(world.get::<Node>(ticks[3].0).unwrap().top, px(111));
        assert_eq!(world.get::<Node>(ticks[3].1).unwrap().top, px(116));
        assert_eq!(
            world.get::<TextLayout>(ticks[3].2).unwrap().linebreak,
            bevy::text::LineBreak::NoWrap
        );
        assert!(world.get_entity(surface).is_ok());
        let pointer = shape(&mut world, parent, Node::default(), Color::NONE);
        project_triangle(
            &mut world,
            pointer,
            Some([
                GameUiChartPoint { x: 110.0, y: 28.0 },
                GameUiChartPoint { x: 104.0, y: 38.0 },
                GameUiChartPoint { x: 116.0, y: 38.0 },
            ]),
            Color::WHITE,
        );
        let projected = world.get::<Node>(pointer).unwrap();
        assert_eq!(projected.left, px(104));
        assert_eq!(projected.top, px(28));
        assert_eq!(projected.border, UiRect::new(px(6), px(6), px(0), px(10)));
        assert_eq!(
            world.get::<BorderColor>(pointer).unwrap().bottom,
            Color::WHITE
        );
        project_triangle(
            &mut world,
            pointer,
            Some([
                GameUiChartPoint { x: 145.0, y: 5.0 },
                GameUiChartPoint { x: 139.0, y: 1.0 },
                GameUiChartPoint { x: 151.0, y: 1.0 },
            ]),
            Color::WHITE,
        );
        assert_eq!(
            world.get::<Node>(pointer).unwrap().border,
            UiRect::new(px(6), px(6), px(4), px(0))
        );
        assert_eq!(world.get::<BorderColor>(pointer).unwrap().top, Color::WHITE);
        project_triangle(&mut world, pointer, None, Color::WHITE);
        assert_eq!(world.get::<Node>(pointer).unwrap().display, Display::None);
        source.content = GameUiHudNodeContent::Direction {
            value: String::new(),
            canvas_size: [72.0, 72.0],
            circle_bounds: GameUiRect {
                x: 9.0,
                y: 9.0,
                width: 54.0,
                height: 54.0,
            },
            circle_radius: 27.0,
            circle_stroke_width: 1.5,
            cross_segments: [[GameUiChartPoint { x: 0.0, y: 0.0 }; 2]; 2],
            cross_stroke_width: 1.0,
            north_label: "N".to_owned(),
            north_anchor: GameUiChartPoint { x: 36.0, y: 24.0 },
            arrow: None,
        };
        let mut direction = spawn_node(&mut world, parent, &source, &font);
        project_widget(
            &mut world,
            &mut direction,
            &source,
            &hud,
            &font,
            [Color::WHITE; 3],
        )
        .unwrap();
        let HudWidget::Direction {
            ring, north, arrow, ..
        } = direction.widget
        else {
            unreachable!()
        };
        assert_eq!(world.get::<Node>(ring).unwrap().left, px(8.25));
        assert_eq!(world.get::<Node>(ring).unwrap().width, px(55.5));
        assert_eq!(world.get::<Node>(north.0).unwrap().top, px(24));
        assert_eq!(
            world.get::<TextLayout>(north.1).unwrap().linebreak,
            bevy::text::LineBreak::NoWrap
        );
        assert!(
            world
                .get::<super::super::ChartTextBaseline>(north.1)
                .is_some()
        );
        assert_eq!(world.get::<Node>(arrow).unwrap().display, Display::None);
        let id = world
            .get::<MaterialNode<DirectionMaterial>>(arrow)
            .unwrap()
            .0
            .id();
        for offset in [0.0, 1.0] {
            let points = [
                GameUiChartPoint {
                    x: 36.0 + offset,
                    y: 15.0,
                },
                GameUiChartPoint { x: 31.0, y: 38.0 },
                GameUiChartPoint { x: 36.0, y: 34.0 },
                GameUiChartPoint { x: 41.0, y: 38.0 },
            ];
            let GameUiHudNodeContent::Direction {
                arrow: source_arrow,
                ..
            } = &mut source.content
            else {
                unreachable!()
            };
            *source_arrow = Some(points);
            project_widget(
                &mut world,
                &mut direction,
                &source,
                &hud,
                &font,
                [Color::WHITE; 3],
            )
            .unwrap();
            assert_eq!(
                world
                    .get::<MaterialNode<DirectionMaterial>>(arrow)
                    .unwrap()
                    .0
                    .id(),
                id
            );
            let materials = world.resource::<Assets<DirectionMaterial>>();
            assert_eq!(materials.len(), 1);
            assert_eq!(
                materials.get(id).unwrap(),
                &DirectionMaterial {
                    points01: Vec4::new(36.0 + offset as f32, 15.0, 31.0, 38.0),
                    points23: Vec4::new(36.0, 34.0, 41.0, 38.0),
                    canvas: Vec4::new(72.0, 72.0, 0.0, 0.0),
                    color: Color::WHITE.to_linear().to_vec4(),
                }
            );
        }
        let retained = world
            .resource::<Assets<DirectionMaterial>>()
            .get(id)
            .unwrap()
            .clone();
        let GameUiHudNodeContent::Direction {
            arrow: source_arrow,
            ..
        } = &mut source.content
        else {
            unreachable!()
        };
        *source_arrow = None;
        project_widget(
            &mut world,
            &mut direction,
            &source,
            &hud,
            &font,
            [Color::WHITE; 3],
        )
        .unwrap();
        assert_eq!(world.get::<Node>(arrow).unwrap().display, Display::None);
        assert_eq!(
            world.resource::<Assets<DirectionMaterial>>().get(id),
            Some(&retained)
        );
    }
}
