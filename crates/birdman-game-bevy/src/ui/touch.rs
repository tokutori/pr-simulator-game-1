use super::*;

#[derive(Component)]
pub(super) struct RetainedFlightTouch {
    indicator: Entity,
    origin: Entity,
}

pub(super) fn project(
    world: &mut World,
    node: Entity,
    position: [f64; 2],
    origin: Option<[f64; 2]>,
    radius: f64,
    ink: Color,
) {
    let (indicator, origin_node) = if let Some(retained) = world.get::<RetainedFlightTouch>(node) {
        (retained.indicator, retained.origin)
    } else {
        let indicator = world
            .spawn((ChildOf(node), Node::default(), Pickable::IGNORE))
            .id();
        let origin = world
            .spawn((ChildOf(node), Node::default(), Pickable::IGNORE))
            .id();
        world
            .entity_mut(node)
            .insert(RetainedFlightTouch { indicator, origin });
        (indicator, origin)
    };
    for (entity, center, fill) in [
        (indicator, Some(position), ink),
        (origin_node, origin, Color::NONE),
    ] {
        let center = center.unwrap_or([0.0; 2]);
        world.entity_mut(entity).insert((
            Node {
                position_type: PositionType::Absolute,
                display: if entity == origin_node && origin.is_none() {
                    Display::None
                } else {
                    Display::Flex
                },
                left: px(center[0]),
                top: px(center[1]),
                width: px(radius * 2.0),
                height: px(radius * 2.0),
                border_radius: BorderRadius::all(percent(50)),
                border: UiRect::all(px(1)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50, -50)),
            BackgroundColor(fill),
            BorderColor::all(ink),
        ));
    }
}

pub(super) fn project_frame(
    world: &mut World,
    node: Entity,
    value: &birdman_game_presentation::game_ui::GameUiFlightTouchValue,
) {
    let Some(retained) = world.get::<RetainedFlightTouch>(node) else {
        return;
    };
    let (indicator, origin) = (retained.indicator, retained.origin);
    let mut indicator_node = retained_node(world, indicator);
    indicator_node
        .reborrow()
        .map_unchanged(|node| &mut node.left)
        .set_if_neq(px(value.position[0]));
    indicator_node
        .reborrow()
        .map_unchanged(|node| &mut node.top)
        .set_if_neq(px(value.position[1]));
    let mut origin_node = retained_node(world, origin);
    origin_node
        .reborrow()
        .map_unchanged(|node| &mut node.display)
        .set_if_neq(if value.origin.is_some() {
            Display::Flex
        } else {
            Display::None
        });
    if let Some(center) = value.origin {
        origin_node
            .reborrow()
            .map_unchanged(|node| &mut node.left)
            .set_if_neq(px(center[0]));
        origin_node
            .reborrow()
            .map_unchanged(|node| &mut node.top)
            .set_if_neq(px(center[1]));
    }
}
