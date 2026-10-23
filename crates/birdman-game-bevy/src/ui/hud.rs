use super::super::NativeFont;
use super::{color, retained_node, retained_ui_transform, set_text, spawn_text};
use bevy::{picking::Pickable, prelude::*};
use birdman_game_presentation::game_ui::{
    GameUiHud, GameUiHudNode, GameUiHudNodeContent, GameUiHudTone, GameUiTheme,
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
        marker: Entity,
        ticks: [(Entity, Entity); 7],
    },
    HeadingTape {
        surface: Entity,
        ticks: Vec<(Entity, Entity)>,
    },
    Direction {
        surface: Entity,
        needle: Entity,
    },
    LinearGauge {
        surface: Entity,
        indicator: Entity,
        minimum: Entity,
        maximum: Entity,
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
            project_widget(world, retained, node, hud, &font, ink)?;
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
                    HudWidget::Attitude { .. } => px(node.instrument_height_em
                        * layout.font_size_px
                        * layout.attitude_size_px[0]
                        / layout.attitude_size_px[1]),
                    HudWidget::Direction { .. } => {
                        px(node.instrument_height_em * layout.font_size_px)
                    }
                    HudWidget::HeadingTape { .. } | HudWidget::LinearGauge { .. } => percent(100),
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
                    width: percent(200),
                    height: percent(200),
                    left: percent(-50),
                    top: percent(-50),
                    ..default()
                },
                Color::NONE,
            );
            let ticks = std::array::from_fn(|_| {
                (
                    shape(world, horizon, Node::default(), Color::WHITE),
                    text(world, horizon, font, 12.0),
                )
            });
            for (left, width) in [(30.0, 15.0), (55.0, 15.0)] {
                shape(
                    world,
                    surface,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(left),
                        top: percent(50),
                        width: percent(width),
                        height: px(2),
                        ..default()
                    },
                    Color::srgb(1.0, 0.85, 0.25),
                );
            }
            let marker = shape(
                world,
                surface,
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
            let pointer = shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    top: px(23),
                    width: px(5),
                    height: px(5),
                    ..default()
                },
                Color::srgb(1.0, 0.85, 0.25),
            );
            world.entity_mut(pointer).insert(UiTransform {
                translation: Val2::percent(-50, 0),
                rotation: Rot2::degrees(45.0),
                ..default()
            });
            HudWidget::HeadingTape {
                surface,
                ticks: Vec::new(),
            }
        }
        GameUiHudNodeContent::Direction { .. } => {
            let surface = shape(
                world,
                card,
                Node {
                    width: px(56),
                    height: px(56),
                    align_self: AlignSelf::Center,
                    ..default()
                },
                Color::NONE,
            );
            let ring = shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(12.5),
                    top: percent(12.5),
                    width: percent(75),
                    height: percent(75),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Color::NONE,
            );
            world
                .entity_mut(ring)
                .insert(BorderColor::all(Color::srgb(0.725, 0.788, 0.761)));
            let needle = shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                Color::NONE,
            );
            for (left, top, width, height, rotation) in [
                (48.5, 22.0, 3.0, 32.0, 0.0),
                (40.0, 25.0, 13.0, 3.0, -45.0),
                (48.0, 25.0, 13.0, 3.0, 45.0),
            ] {
                let arm = shape(
                    world,
                    needle,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(left),
                        top: percent(top),
                        width: percent(width),
                        height: percent(height),
                        ..default()
                    },
                    Color::srgb(1.0, 0.85, 0.25),
                );
                world
                    .entity_mut(arm)
                    .insert(UiTransform::from_rotation(Rot2::degrees(rotation)));
            }
            HudWidget::Direction { surface, needle }
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
            shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(5),
                    top: px(16),
                    width: percent(90),
                    height: px(2),
                    ..default()
                },
                Color::srgb(0.725, 0.788, 0.761),
            );
            let minimum = text(world, surface, font, 8.0);
            let maximum = text(world, surface, font, 8.0);
            for (entity, left, anchor) in [(minimum, 5, 0), (maximum, 95, -100)] {
                retained_node(world, entity).set_if_neq(Node {
                    position_type: PositionType::Absolute,
                    left: percent(left),
                    top: px(20),
                    ..default()
                });
                world
                    .entity_mut(entity)
                    .insert(UiTransform::from_translation(Val2::percent(anchor, 0)));
            }
            let indicator = shape(
                world,
                surface,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(5),
                    top: px(3),
                    width: px(7),
                    height: px(7),
                    ..default()
                },
                Color::srgb(1.0, 0.85, 0.25),
            );
            world.entity_mut(indicator).insert(UiTransform {
                translation: Val2::percent(-50, 0),
                rotation: Rot2::degrees(45.0),
                ..default()
            });
            HudWidget::LinearGauge {
                surface,
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

fn project_widget(
    world: &mut World,
    node: &mut HudNode,
    source: &GameUiHudNode,
    hud: &GameUiHud,
    font: &Handle<Font>,
    ink: Color,
) -> Result<(), String> {
    match &source.content {
        GameUiHudNodeContent::Text { .. } => {}
        GameUiHudNodeContent::Attitude { geometry, .. } => {
            let HudWidget::Attitude {
                surface,
                horizon,
                marker,
                ticks,
            } = node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            let layer = geometry.horizon();
            let marker_ratio = geometry.flight_path_y_ratio();
            let height = source.instrument_height_em * hud.layout.font_size_px;
            world
                .get_mut::<BackgroundColor>(surface)
                .expect("ADI background exists")
                .set_if_neq(BackgroundColor(Color::NONE));
            retained_node(world, horizon)
                .map_unchanged(|node| &mut node.display)
                .set_if_neq(if layer.is_some() {
                    Display::Flex
                } else {
                    Display::None
                });
            if let Some(layer) = layer {
                let offset = layer.pitch_offset_ratio * height;
                let rotation = layer.rotation_degrees.to_radians();
                retained_ui_transform(world, horizon).set_if_neq(UiTransform {
                    translation: Val2::px(-offset * rotation.sin(), offset * rotation.cos()),
                    rotation: Rot2::degrees(layer.rotation_degrees as f32),
                    ..default()
                });
                for ((line, label), tick) in ticks.into_iter().zip(layer.pitch_ticks) {
                    retained_node(world, line).set_if_neq(Node {
                        position_type: PositionType::Absolute,
                        left: percent(50.0 - tick.half_width_ratio * 50.0),
                        top: percent(50.0 + tick.offset_ratio * 50.0),
                        width: percent(tick.half_width_ratio * 100.0),
                        height: px(1),
                        ..default()
                    });
                    retained_node(world, label).set_if_neq(Node {
                        position_type: PositionType::Absolute,
                        display: if tick.label_degrees.is_some() {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        left: percent(50.0 + tick.half_width_ratio * 50.0),
                        top: percent(50.0 + tick.offset_ratio * 50.0),
                        ..default()
                    });
                    retained_ui_transform(world, label)
                        .set_if_neq(UiTransform::from_translation(Val2::percent(0, -50)));
                    set_text(
                        world,
                        label,
                        tick.label_degrees
                            .map_or_else(String::new, |value| value.to_string()),
                    );
                    world
                        .get_mut::<TextFont>(label)
                        .expect("pitch label font exists")
                        .map_unchanged(|font| &mut font.font_size)
                        .set_if_neq(FontSize::Px(hud.layout.font_size_px as f32));
                }
            }
            retained_node(world, marker)
                .map_unchanged(|node| &mut node.display)
                .set_if_neq(if marker_ratio.is_some() {
                    Display::Flex
                } else {
                    Display::None
                });
            retained_node(world, marker)
                .map_unchanged(|node| &mut node.top)
                .set_if_neq(percent(marker_ratio.unwrap_or(0.5) * 100.0));
        }
        GameUiHudNodeContent::HeadingTape { ticks, .. } => {
            let HudWidget::HeadingTape {
                surface,
                ticks: entities,
            } = &mut node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            while entities.len() > ticks.len() {
                let (line, label) = entities.pop().expect("extra tick exists");
                world.despawn(line);
                world.despawn(label);
            }
            while entities.len() < ticks.len() {
                let line = shape(world, *surface, Node::default(), ink);
                let label = text(world, *surface, font, hud.layout.font_size_px as f32);
                world
                    .entity_mut(label)
                    .insert(UiTransform::from_translation(Val2::percent(-50, 0)));
                entities.push((line, label));
            }
            for ((line, label), tick) in entities.iter().copied().zip(ticks) {
                retained_node(world, line).set_if_neq(Node {
                    position_type: PositionType::Absolute,
                    left: percent(tick.position_ratio * 100.0),
                    top: px(if tick.major { 19 } else { 23 }),
                    width: px(1),
                    height: px(if tick.major { 8 } else { 4 }),
                    ..default()
                });
                retained_node(world, label).set_if_neq(Node {
                    position_type: PositionType::Absolute,
                    display: if !tick.label.is_empty() {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    left: percent(tick.position_ratio * 100.0),
                    top: px(4),
                    ..default()
                });
                set_text(world, label, tick.label.clone());
                world
                    .get_mut::<TextColor>(label)
                    .expect("heading label color exists")
                    .set_if_neq(TextColor(ink));
                world
                    .get_mut::<TextFont>(label)
                    .expect("heading label font exists")
                    .map_unchanged(|font| &mut font.font_size)
                    .set_if_neq(FontSize::Px(hud.layout.font_size_px as f32));
                world
                    .get_mut::<BackgroundColor>(line)
                    .expect("heading tick color exists")
                    .set_if_neq(BackgroundColor(ink));
            }
        }
        GameUiHudNodeContent::Direction {
            rotation_degrees, ..
        } => {
            let HudWidget::Direction { needle, .. } = node.widget else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            retained_node(world, needle)
                .map_unchanged(|node| &mut node.display)
                .set_if_neq(if rotation_degrees.is_some() {
                    Display::Flex
                } else {
                    Display::None
                });
            retained_ui_transform(world, needle).set_if_neq(UiTransform::from_rotation(
                Rot2::degrees(rotation_degrees.unwrap_or(0.0) as f32),
            ));
        }
        GameUiHudNodeContent::LinearGauge {
            minimum_label,
            maximum_label,
            position_ratio,
            ..
        } => {
            let HudWidget::LinearGauge {
                indicator,
                minimum,
                maximum,
                ..
            } = node.widget
            else {
                return Err("HUD widget kindが一致しない".to_owned());
            };
            set_text(world, minimum, minimum_label.clone());
            set_text(world, maximum, maximum_label.clone());
            for label in [minimum, maximum] {
                world
                    .get_mut::<TextColor>(label)
                    .expect("gauge label color exists")
                    .set_if_neq(TextColor(ink));
                world
                    .get_mut::<TextFont>(label)
                    .expect("gauge label font exists")
                    .map_unchanged(|font| &mut font.font_size)
                    .set_if_neq(FontSize::Px(hud.layout.font_size_px as f32));
            }
            retained_node(world, indicator)
                .map_unchanged(|node| &mut node.display)
                .set_if_neq(if position_ratio.is_some() {
                    Display::Flex
                } else {
                    Display::None
                });
            retained_node(world, indicator)
                .map_unchanged(|node| &mut node.left)
                .set_if_neq(percent(5.0 + position_ratio.unwrap_or(0.0) * 90.0));
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
                geometry: GameUiHudAdiGeometry::FlightPathOnly { y_ratio: 0.25 },
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
        project_widget(&mut world, &mut widget, &source, &hud, &font, Color::WHITE).unwrap();
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
        assert_eq!(world.get::<Node>(horizon).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(marker).unwrap().top, percent(25.0));
        source.content = GameUiHudNodeContent::Attitude {
            value: String::new(),
            geometry: GameUiHudAdiGeometry::FlightPathOnly { y_ratio: 0.75 },
        };
        project_widget(&mut world, &mut widget, &source, &hud, &font, Color::WHITE).unwrap();
        assert_eq!(world.get::<Node>(marker).unwrap().top, percent(75.0));
        assert!(world.get_entity(surface).is_ok());
    }
}
