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
    #[cfg(test)]
    structure_projections: usize,
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
        #[cfg(test)]
        structure_projections: 0,
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
    #[cfg(test)]
    {
        retained.structure_projections += 1;
    }
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
    use super::super::{
        RetainedUi,
        tests::{add_ui_layout, app, press},
    };
    use super::*;
    use birdman_game_presentation::game_ui::{GameUiHudAdiGeometry, GameUiHudHeadingTick};

    fn flying_app() -> App {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        for _step in 0..3 {
            app.world_mut()
                .resource_mut::<super::super::super::native_session::NativeSession>()
                .presentation_frame(1.0);
        }
        app.update();
        app
    }

    fn descendants(world: &World, root: Entity) -> Vec<Entity> {
        let mut entities = vec![root];
        if let Some(children) = world.get::<Children>(root) {
            for child in children.iter() {
                entities.extend(descendants(world, child));
            }
        }
        entities
    }

    fn assert_order_and_text(world: &World, retained: &RetainedHud, hud: &GameUiHud) {
        let groups = world.get::<Children>(retained.shell).unwrap();
        assert_eq!(
            groups.iter().collect::<Vec<_>>(),
            hud.groups
                .iter()
                .map(|group| retained.groups[&group.id])
                .collect::<Vec<_>>()
        );
        for group in &hud.groups {
            let children = world.get::<Children>(retained.groups[&group.id]).unwrap();
            assert_eq!(
                children.iter().collect::<Vec<_>>(),
                group
                    .nodes
                    .iter()
                    .map(|node| retained.nodes[&node.id].card)
                    .collect::<Vec<_>>()
            );
            for node in &group.nodes {
                let projected = &retained.nodes[&node.id];
                assert_eq!(
                    world.get::<Text>(projected.value).unwrap().0,
                    if !node.label.is_empty() {
                        format!("{} {}", node.label, node.content.value())
                    } else {
                        node.content.value().to_owned()
                    }
                );
                let card = world.get::<Node>(projected.card).unwrap();
                assert_eq!(card.left, percent(node.bounds.x * 100.0));
                assert_eq!(card.top, percent(node.bounds.y * 100.0));
                assert_eq!(card.width, percent(node.bounds.width * 100.0));
                assert_eq!(card.height, percent(node.bounds.height * 100.0));
                assert_eq!(card.overflow, Overflow::default());
                if let Some(surface) = projected.widget.surface() {
                    assert_eq!(
                        world.get::<Node>(surface).unwrap().height,
                        px(node.instrument_height_em * hud.layout.font_size_px)
                    );
                }
                for entity in descendants(world, projected.card) {
                    if world
                        .get::<Text>(entity)
                        .is_some_and(|text| !text.0.is_empty())
                    {
                        assert_eq!(
                            world.get::<TextFont>(entity).unwrap().font_size,
                            FontSize::Px(hud.layout.font_size_px as f32)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_shared_widget_instance_is_projected_in_order_and_removed_subtrees_are_released() {
        let mut app = flying_app();
        let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
        let mut hud = retained.hud.previous.clone().unwrap();
        let theme = retained.view.as_ref().unwrap().theme.clone();
        assert_order_and_text(app.world(), &retained.hud, &hud);
        let original = retained
            .hud
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.card))
            .collect::<BTreeMap<_, _>>();
        let mut extras = Vec::new();
        for (index, node) in hud.groups.iter().flat_map(|group| &group.nodes).enumerate() {
            let mut node = node.clone();
            node.id = format!("new-instance-{index}");
            node.label = format!("共有の見出し {index}");
            extras.push(node);
        }
        hud.groups[1].nodes.extend(extras);
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_order_and_text(app.world(), &retained.hud, &hud);
        for (id, entity) in &original {
            assert_eq!(retained.hud.nodes[id].card, *entity);
        }
        hud.groups.reverse();
        for group in &mut hud.groups {
            group.nodes.reverse();
        }
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_order_and_text(app.world(), &retained.hud, &hud);
        let retired = hud.groups.remove(1);
        let retired_entities = descendants(app.world(), retained.hud.groups[&retired.id]);
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_order_and_text(app.world(), &retained.hud, &hud);
        assert!(
            retired_entities
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        assert!(
            retired
                .nodes
                .iter()
                .all(|node| !retained.hud.nodes.contains_key(&node.id))
        );
        let node = &mut hud.groups[0].nodes[0];
        let old_card = retained.hud.nodes[&node.id].card;
        let old_tree = descendants(app.world(), old_card);
        node.label = "任意の型変更".to_owned();
        node.content = GameUiHudNodeContent::Direction {
            value: "共有値".to_owned(),
            rotation_degrees: Some(17.0),
        };
        node.instrument_height_em = 3.0;
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert!(
            old_tree
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        assert_order_and_text(app.world(), &retained.hud, &hud);
    }

    #[test]
    fn dynamic_values_preserve_static_nodes_fonts_and_retained_identity_across_hide_and_resize() {
        let mut app = flying_app();
        let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
        let mut hud = retained.hud.previous.clone().unwrap();
        let theme = retained.view.as_ref().unwrap().theme.clone();
        let direction = hud
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.nodes)
            .find(|node| matches!(node.content, GameUiHudNodeContent::Direction { .. }))
            .unwrap();
        let direction_id = direction.id.clone();
        let GameUiHudNodeContent::Direction {
            rotation_degrees, ..
        } = &mut direction.content
        else {
            unreachable!()
        };
        *rotation_degrees = Some(0.0);
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        let HudWidget::Direction { needle, .. } = retained.hud.nodes[&direction_id].widget else {
            unreachable!()
        };
        let entities = [retained.hud.shell]
            .into_iter()
            .chain(retained.hud.groups.values().copied())
            .chain(
                retained
                    .hud
                    .nodes
                    .values()
                    .flat_map(|node| [node.card, node.value]),
            )
            .collect::<Vec<_>>();
        let observed = |world: &World| {
            entities
                .iter()
                .map(|entity| {
                    let entity = world.entity(*entity);
                    let node = entity.get_ref::<Node>().unwrap();
                    let text = entity
                        .get_ref::<Text>()
                        .map(|text| (text.0.clone(), text.last_changed()));
                    let font = entity.get_ref::<TextFont>().map(|font| font.last_changed());
                    ((*node).clone(), node.last_changed(), text, font)
                })
                .collect::<Vec<_>>()
        };
        let before = observed(app.world());
        let structure_count = retained.hud.structure_projections;
        let direction = hud
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.nodes)
            .find(|node| node.id == direction_id)
            .unwrap();
        let GameUiHudNodeContent::Direction {
            rotation_degrees, ..
        } = &mut direction.content
        else {
            unreachable!()
        };
        *rotation_degrees = Some(90.0);
        app.world_mut().increment_change_tick();
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_eq!(observed(app.world()), before);
        assert_eq!(retained.hud.structure_projections, structure_count);
        assert_eq!(
            app.world().get::<UiTransform>(needle).unwrap().rotation,
            Rot2::degrees(90.0)
        );
        let node = hud
            .groups
            .iter_mut()
            .flat_map(|group| &mut group.nodes)
            .find(|node| matches!(node.content, GameUiHudNodeContent::Text { .. }))
            .unwrap();
        let value_entity = retained.hud.nodes[&node.id].value;
        let GameUiHudNodeContent::Text { value, .. } = &mut node.content else {
            unreachable!()
        };
        value.push_str("\n姿勢 PITCH wording is opaque");
        let expected = if node.label.is_empty() {
            value.clone()
        } else {
            format!("{} {value}", node.label)
        };
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_eq!(app.world().get::<Text>(value_entity).unwrap().0, expected);
        assert_eq!(retained.hud.structure_projections, structure_count);
        let cards = retained
            .hud
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.card))
            .collect::<BTreeMap<_, _>>();
        hud.layout.width_px -= 13.0;
        hud.layout.height_px = 100.0;
        hud.layout.opacity = 0.123;
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_eq!(retained.hud.structure_projections, structure_count + 1);
        let shell = app.world().get::<Node>(retained.hud.shell).unwrap();
        assert_eq!(shell.width, px(hud.layout.width_px));
        assert_eq!(shell.height, px(100));
        assert_eq!(shell.top, auto());
        assert_eq!(shell.bottom, px(hud.layout.offset_px[1]));
        assert_eq!(shell.overflow, Overflow::default());
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(retained.hud.shell)
                .unwrap()
                .0,
            Color::NONE
        );
        for node in hud.groups.iter().flat_map(|group| &group.nodes) {
            let expected = if matches!(node.content, GameUiHudNodeContent::Text { .. }) {
                Color::NONE
            } else {
                color(&theme.panel_background, 0.123).unwrap()
            };
            assert_eq!(
                app.world()
                    .get::<BackgroundColor>(retained.hud.nodes[&node.id].card)
                    .unwrap()
                    .0,
                expected
            );
        }
        project_hud(app.world_mut(), &mut retained.hud, None, &theme).unwrap();
        assert_eq!(
            app.world().get::<Node>(retained.hud.shell).unwrap().display,
            Display::None
        );
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        assert_eq!(
            app.world().get::<Node>(retained.hud.shell).unwrap().display,
            Display::Flex
        );
        for (id, entity) in cards {
            assert_eq!(retained.hud.nodes[&id].card, entity);
        }
        assert_order_and_text(app.world(), &retained.hud, &hud);
    }

    #[test]
    fn shared_geometry_labels_and_unavailability_are_projected_without_fixed_instrument_limits() {
        let mut app = flying_app();
        let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
        let mut hud = retained.hud.previous.clone().unwrap();
        let theme = retained.view.as_ref().unwrap().theme.clone();
        for node in hud.groups.iter_mut().flat_map(|group| &mut group.nodes) {
            match &mut node.content {
                GameUiHudNodeContent::HeadingTape { ticks, .. } => {
                    *ticks = (0..21)
                        .map(|index| GameUiHudHeadingTick {
                            position_ratio: index as f64 / 20.0,
                            major: index % 2 == 0,
                            label: format!("共有 {index}"),
                        })
                        .collect();
                }
                GameUiHudNodeContent::LinearGauge {
                    minimum_label,
                    maximum_label,
                    position_ratio,
                    ..
                } => {
                    *minimum_label = "任意下端".to_owned();
                    *maximum_label = "任意上端".to_owned();
                    *position_ratio = Some(0.25);
                }
                GameUiHudNodeContent::Text { .. }
                | GameUiHudNodeContent::Attitude { .. }
                | GameUiHudNodeContent::Direction { .. } => {}
            }
        }
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        let mut retired_ticks = Vec::new();
        for node in hud.groups.iter().flat_map(|group| &group.nodes) {
            let projected = &retained.hud.nodes[&node.id];
            match (&node.content, &projected.widget) {
                (
                    GameUiHudNodeContent::Attitude { geometry, .. },
                    HudWidget::Attitude {
                        surface,
                        horizon,
                        marker,
                        ticks,
                    },
                ) => {
                    let layer = geometry.horizon().unwrap();
                    let offset = layer.pitch_offset_ratio
                        * node.instrument_height_em
                        * hud.layout.font_size_px;
                    let rotation = layer.rotation_degrees.to_radians();
                    assert_eq!(
                        app.world()
                            .get::<UiTransform>(*horizon)
                            .unwrap()
                            .translation,
                        Val2::px(-offset * rotation.sin(), offset * rotation.cos())
                    );
                    assert_eq!(
                        app.world().get::<Node>(*marker).unwrap().top,
                        percent(geometry.flight_path_y_ratio().unwrap() * 100.0)
                    );
                    assert_eq!(
                        app.world().get::<BackgroundColor>(*surface).unwrap().0,
                        Color::NONE
                    );
                    for ((line, label), tick) in ticks.iter().zip(layer.pitch_ticks) {
                        let line = app.world().get::<Node>(*line).unwrap();
                        assert_eq!(line.top, percent(50.0 + tick.offset_ratio * 50.0));
                        assert_eq!(line.width, percent(tick.half_width_ratio * 100.0));
                        assert_eq!(
                            app.world().get::<Text>(*label).unwrap().0,
                            tick.label_degrees
                                .map_or_else(String::new, |value| value.to_string())
                        );
                    }
                }
                (
                    GameUiHudNodeContent::HeadingTape { ticks, .. },
                    HudWidget::HeadingTape {
                        ticks: entities, ..
                    },
                ) => {
                    assert_eq!(entities.len(), 21);
                    for ((line, label), tick) in entities.iter().zip(ticks) {
                        assert_eq!(
                            app.world().get::<Node>(*line).unwrap().left,
                            percent(tick.position_ratio * 100.0)
                        );
                        assert_eq!(app.world().get::<Text>(*label).unwrap().0, tick.label);
                        assert_eq!(
                            app.world().get::<Node>(*label).unwrap().display,
                            Display::Flex
                        );
                    }
                    retired_ticks.extend(entities.iter().flat_map(|(line, label)| [*line, *label]));
                }
                (
                    GameUiHudNodeContent::LinearGauge {
                        minimum_label,
                        maximum_label,
                        ..
                    },
                    HudWidget::LinearGauge {
                        indicator,
                        minimum,
                        maximum,
                        ..
                    },
                ) => {
                    assert_eq!(
                        app.world().get::<Node>(*indicator).unwrap().left,
                        percent(27.5)
                    );
                    assert_eq!(app.world().get::<Text>(*minimum).unwrap().0, *minimum_label);
                    assert_eq!(app.world().get::<Text>(*maximum).unwrap().0, *maximum_label);
                    assert_eq!(
                        app.world()
                            .get::<UiTransform>(*minimum)
                            .unwrap()
                            .translation,
                        Val2::percent(0, 0)
                    );
                    assert_eq!(
                        app.world()
                            .get::<UiTransform>(*maximum)
                            .unwrap()
                            .translation,
                        Val2::percent(-100, 0)
                    );
                }
                _ => {}
            }
        }
        for node in hud.groups.iter_mut().flat_map(|group| &mut group.nodes) {
            match &mut node.content {
                GameUiHudNodeContent::Attitude { geometry, .. } => {
                    *geometry = GameUiHudAdiGeometry::FlightPathOnly { y_ratio: 0.3 }
                }
                GameUiHudNodeContent::HeadingTape { ticks, .. } => ticks.clear(),
                GameUiHudNodeContent::Direction {
                    rotation_degrees, ..
                } => *rotation_degrees = None,
                GameUiHudNodeContent::LinearGauge { position_ratio, .. } => *position_ratio = None,
                GameUiHudNodeContent::Text { .. } => {}
            }
        }
        project_hud(app.world_mut(), &mut retained.hud, Some(&hud), &theme).unwrap();
        for node in retained.hud.nodes.values() {
            match &node.widget {
                HudWidget::Attitude {
                    surface,
                    horizon,
                    marker,
                    ..
                } => {
                    assert_eq!(
                        app.world().get::<Node>(*surface).unwrap().display,
                        Display::Flex
                    );
                    assert_eq!(
                        app.world().get::<Node>(*horizon).unwrap().display,
                        Display::None
                    );
                    assert_eq!(app.world().get::<Node>(*marker).unwrap().top, percent(30));
                }
                HudWidget::HeadingTape { ticks, .. } => assert!(ticks.is_empty()),
                HudWidget::Direction { needle, .. } => assert_eq!(
                    app.world().get::<Node>(*needle).unwrap().display,
                    Display::None
                ),
                HudWidget::LinearGauge { indicator, .. } => assert_eq!(
                    app.world().get::<Node>(*indicator).unwrap().display,
                    Display::None
                ),
                HudWidget::Text => {}
            }
        }
        assert!(
            retired_ticks
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
    }

    #[test]
    fn custom_flight_path_only_retains_visible_ancestors_from_the_shared_provider() {
        let mut app = app(1280.0, 720.0);
        press(&mut app, "game-title-start");
        press(&mut app, "game-setup-select-information-4");
        press(&mut app, "game-setup-information-attitude");
        press(&mut app, "game-setup-start");
        press(&mut app, "game-briefing-start");
        for _step in 0..3 {
            app.world_mut()
                .resource_mut::<super::super::super::native_session::NativeSession>()
                .presentation_frame(1.0);
        }
        app.update();
        let retained = app.world().resource::<RetainedUi>();
        let hud = retained.hud.previous.as_ref().unwrap();
        let shared = hud
            .groups
            .iter()
            .flat_map(|group| &group.nodes)
            .find(|node| matches!(node.content, GameUiHudNodeContent::Attitude { .. }))
            .unwrap();
        let GameUiHudNodeContent::Attitude {
            geometry: GameUiHudAdiGeometry::FlightPathOnly { y_ratio },
            ..
        } = shared.content
        else {
            panic!("the shared provider must retain the independent flight-path cue")
        };
        let HudWidget::Attitude {
            surface,
            horizon,
            marker,
            ..
        } = retained.hud.nodes[&shared.id].widget
        else {
            unreachable!()
        };
        let mut ancestor = marker;
        loop {
            assert_ne!(
                app.world().get::<Node>(ancestor).unwrap().display,
                Display::None
            );
            if ancestor == retained.hud.shell {
                break;
            }
            ancestor = app.world().get::<ChildOf>(ancestor).unwrap().parent();
        }
        assert_eq!(
            app.world().get::<BackgroundColor>(surface).unwrap().0,
            Color::NONE
        );
        assert_eq!(
            app.world().get::<Node>(horizon).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(marker).unwrap().top,
            percent(y_ratio * 100.0)
        );
    }

    #[test]
    fn shared_bottom_bounds_leave_forward_view_clear_without_scrolling_or_card_overlap() {
        let mut app = flying_app();
        let camera = add_ui_layout(&mut app, 1280, 720);
        let before = app
            .world()
            .resource::<super::super::super::native_session::NativeSession>()
            .game
            .core_session()
            .snapshot();
        let bounds = |world: &World, entity: Entity| {
            Rect::from_center_size(
                world.get::<UiGlobalTransform>(entity).unwrap().translation,
                world.get::<ComputedNode>(entity).unwrap().size(),
            )
        };
        for (width, height, scale) in [
            (1280, 720, 1.0),
            (1920, 1080, 1.0),
            (1600, 900, 1.25),
            (1920, 1080, 1.5),
            (1280, 720, 1.0),
        ] {
            for mut window in app
                .world_mut()
                .query::<&mut Window>()
                .iter_mut(app.world_mut())
            {
                window.resolution.set(width as f32, height as f32);
            }
            {
                let mut camera = app.world_mut().get_mut::<Camera>(camera).unwrap();
                let dimensions = UVec2::new(width, height);
                camera.computed.target_info.as_mut().unwrap().physical_size = dimensions;
                camera.viewport.as_mut().unwrap().physical_size = dimensions;
            }
            app.world_mut().resource_mut::<UiScale>().0 = scale;
            for _frame in 0..4 {
                app.update();
            }
            let retained = &app.world().resource::<RetainedUi>().hud;
            let hud = retained.previous.as_ref().unwrap();
            assert_order_and_text(app.world(), retained, hud);
            let rail = bounds(app.world(), retained.shell);
            let shell = app.world().get::<Node>(retained.shell).unwrap();
            assert_eq!(shell.bottom, px(hud.layout.offset_px[1]));
            assert_eq!(shell.top, auto());
            assert_eq!(shell.overflow, Overflow::default());
            assert_eq!(
                app.world()
                    .get::<BackgroundColor>(retained.shell)
                    .unwrap()
                    .0,
                Color::NONE
            );
            assert!(
                rail.min.y >= height as f32 * 0.5,
                "{width}x{height}: {rail:?}"
            );
            assert!(rail.min.x >= -1.0 && rail.max.x <= width as f32 + 1.0);
            assert!(rail.max.y <= height as f32 + 1.0);
            let cards = retained.nodes.values().collect::<Vec<_>>();
            for (index, card) in cards.iter().enumerate() {
                let card_bounds = bounds(app.world(), card.card);
                assert!(card_bounds.min.x >= rail.min.x - 1.0);
                assert!(card_bounds.max.x <= rail.max.x + 1.0);
                assert!(card_bounds.min.y >= rail.min.y - 1.0);
                assert!(card_bounds.max.y <= rail.max.y + 1.0);
                let computed = app.world().get::<ComputedNode>(card.card).unwrap();
                assert!(
                    computed.content_size.y <= computed.size().y + 1.0,
                    "{width}x{height}: card={card_bounds:?}, content={:?}",
                    computed.content_size
                );
                for other in &cards[index + 1..] {
                    let other_bounds = bounds(app.world(), other.card);
                    assert!(
                        card_bounds.max.x <= other_bounds.min.x + 1.0
                            || other_bounds.max.x <= card_bounds.min.x + 1.0
                            || card_bounds.max.y <= other_bounds.min.y + 1.0
                            || other_bounds.max.y <= card_bounds.min.y + 1.0,
                        "{width}x{height}: {card_bounds:?} overlaps {other_bounds:?}"
                    );
                }
            }
            assert_eq!(
                app.world()
                    .resource::<super::super::super::native_session::NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                before
            );
        }
    }
}
