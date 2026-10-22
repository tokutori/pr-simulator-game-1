use super::*;

pub(super) fn project_chart(
    world: &mut World,
    parent: Entity,
    title: Entity,
    chart: &GameUiChart,
    root_font_size: f64,
    legend_ink: Color,
    font: &Handle<Font>,
) -> Result<(), String> {
    let retained_surface = world
        .get::<RetainedChart>(parent)
        .map(|chart| chart.surface);
    let children = world
        .entity(parent)
        .get::<Children>()
        .map(|children| {
            children
                .iter()
                .filter(|entity| *entity != title && Some(*entity) != retained_surface)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for child in children {
        world.despawn(child);
    }
    let surface = retained_surface.unwrap_or_else(|| {
        world
            .spawn((ChildOf(parent), Node::default(), ChartViewport))
            .id()
    });
    let children = world
        .get::<Children>(surface)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        world.despawn(child);
    }
    let presentation = &chart.presentation;
    world.entity_mut(surface).insert(Node {
        width: percent(100),
        min_width: px(presentation.viewport_minimum_rem[0] * root_font_size),
        min_height: px(presentation.viewport_minimum_rem[1] * root_font_size),
        flex_grow: 1.0,
        flex_basis: px(presentation.viewport_minimum_rem[1] * root_font_size),
        flex_shrink: 0.0,
        overflow: Overflow::clip(),
        ..default()
    });
    let plot = presentation.plot;
    let screen_position = |point: GameUiChartPoint| Vec2::new(point.x as f32, point.y as f32);
    let position = |point: GameUiChartPoint| {
        Vec2::new(
            (plot[0]
                + (point.x - chart.x_minimum) / (chart.x_maximum - chart.x_minimum)
                    * (plot[2] - plot[0])) as f32,
            (plot[3]
                - (point.y - chart.y_minimum) / (chart.y_maximum - chart.y_minimum)
                    * (plot[3] - plot[1])) as f32,
        )
    };
    let axes = color(&presentation.axes_color, 1.0)?;
    for [start, end] in &presentation.grid_lines {
        chart_line(
            world,
            surface,
            screen_position(*start),
            screen_position(*end),
            color(&presentation.grid_color, presentation.grid_opacity)?,
        );
    }
    chart_line(
        world,
        surface,
        Vec2::new(plot[0] as f32, plot[1] as f32),
        Vec2::new(plot[0] as f32, plot[3] as f32),
        axes,
    );
    chart_line(
        world,
        surface,
        Vec2::new(plot[0] as f32, plot[3] as f32),
        Vec2::new(plot[2] as f32, plot[3] as f32),
        axes,
    );
    for tick in presentation.x_ticks.iter().chain(&presentation.y_ticks) {
        project_chart_text(world, surface, &tick.text, font)?;
    }
    for series in &chart.series {
        for (index, pair) in series.points.windows(2).enumerate() {
            let ink = color(
                series.segment_colors.get(index).unwrap_or(&series.color),
                1.0,
            )?;
            chart_line(world, surface, position(pair[0]), position(pair[1]), ink);
        }
    }
    for vector in chart.vectors.iter().skip(1) {
        if let Some([start, end]) = vector.segment {
            let ink = color(&vector.color, 1.0)?;
            let start = position(start);
            let end = position(end);
            chart_line(world, surface, start, end, ink);
            let direction = (end - start).normalize_or_zero();
            let side = Vec2::new(-direction.y, direction.x);
            chart_line(world, surface, end, end - direction * 7.0 + side * 3.0, ink);
            chart_line(world, surface, end, end - direction * 7.0 - side * 3.0, ink);
        }
    }
    for marker in chart.markers.iter().chain(&chart.time_markers) {
        let point = position(marker.point);
        world.spawn((
            ChildOf(surface),
            Node {
                position_type: PositionType::Absolute,
                left: px(point.x - 3.0),
                top: px(point.y - 3.0),
                width: px(6),
                height: px(6),
                ..default()
            },
            BackgroundColor(color(&marker.color, 1.0)?),
        ));
    }
    for label in presentation
        .marker_labels
        .iter()
        .chain(&presentation.time_marker_labels)
    {
        project_chart_text(world, surface, label, font)?;
    }
    for reference in &chart.reference_lines {
        let point = position(GameUiChartPoint {
            x: chart.x_minimum,
            y: reference.value,
        });
        let ink = color(&reference.color, 1.0)?;
        chart_line(
            world,
            surface,
            point,
            position(GameUiChartPoint {
                x: chart.x_maximum,
                y: reference.value,
            }),
            ink,
        );
    }
    for label in &presentation.reference_labels {
        project_chart_text(world, surface, label, font)?;
    }
    project_chart_text(world, surface, &presentation.x_axis, font)?;
    project_chart_text(world, surface, &presentation.y_axis, font)?;
    if let Some(guides) = &presentation.map_guides {
        for (segments, label) in [
            (&guides.scale_segments, &guides.scale_label),
            (&guides.north_segments, &guides.north_label),
        ] {
            for [start, end] in segments {
                chart_line(
                    world,
                    surface,
                    screen_position(*start),
                    screen_position(*end),
                    color(&label.color, 1.0)?,
                );
            }
        }
        project_chart_text(world, surface, &guides.scale_label, font)?;
        project_chart_text(world, surface, &guides.north_label, font)?;
    }
    let legend = world
        .spawn((
            ChildOf(parent),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_wrap: FlexWrap::Wrap,
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: px(presentation.legend_gap_rem * root_font_size),
                row_gap: px(presentation.legend_gap_rem * root_font_size),
                ..default()
            },
        ))
        .id();
    for entry in &presentation.legend {
        let item = world
            .spawn((
                ChildOf(legend),
                Node {
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    column_gap: px(presentation.legend_swatch_gap_rem * root_font_size),
                    ..default()
                },
            ))
            .id();
        let dimensions = if entry.segment_gradient.is_some() {
            presentation.legend_gradient_rem
        } else {
            presentation.legend_swatch_rem
        };
        let swatch = world
            .spawn((
                ChildOf(item),
                Node {
                    width: px(dimensions[0] * root_font_size),
                    height: px(dimensions[1] * root_font_size),
                    flex_shrink: 0.0,
                    border_radius: if entry.segment_gradient.is_some() {
                        BorderRadius::all(px(
                            presentation.legend_gradient_radius_rem * root_font_size
                        ))
                    } else {
                        BorderRadius::MAX
                    },
                    ..default()
                },
                BackgroundColor(color(&entry.color, 1.0)?),
            ))
            .id();
        if let Some([start, end]) = &entry.segment_gradient {
            world
                .entity_mut(swatch)
                .insert(BackgroundGradient::from(LinearGradient::to_right(vec![
                    ColorStop::new(color(start, 1.0)?, percent(0)),
                    ColorStop::new(color(end, 1.0)?, percent(100)),
                ])));
        }
        let text = spawn_text(
            world,
            item,
            &entry.label,
            font,
            (presentation.legend_font_rem * root_font_size) as f32,
            legend_ink,
        );
        world.entity_mut(text).insert(TextLayout::no_wrap());
    }
    let cursor = chart_line(world, surface, Vec2::ZERO, Vec2::ZERO, Color::WHITE);
    let wind =
        std::array::from_fn(|_| chart_line(world, surface, Vec2::ZERO, Vec2::ZERO, Color::WHITE));
    world.entity_mut(parent).insert(RetainedChart {
        surface,
        cursor,
        points: Vec::new(),
        wind,
        plot,
        x: [chart.x_minimum, chart.x_maximum],
        y: [chart.y_minimum, chart.y_maximum],
    });
    project_chart_cursor(
        world,
        parent,
        &GameUiChartCursor {
            id: String::new(),
            cursor_x: chart.cursor_x,
            cursor_points: chart.cursor_points.clone(),
            selected_wind_vector: chart.vectors.first().cloned(),
        },
    )?;
    Ok(())
}

fn project_chart_text(
    world: &mut World,
    parent: Entity,
    text: &GameUiChartText,
    font: &Handle<Font>,
) -> Result<Entity, String> {
    let anchor = world
        .spawn((
            ChildOf(parent),
            Node {
                position_type: PositionType::Absolute,
                left: px(text.position.x),
                top: px(text.position.y),
                width: px(0),
                height: px(0),
                ..default()
            },
            UiTransform::from_rotation(Rot2::radians(text.rotation_degrees.to_radians() as f32)),
        ))
        .id();
    let entity = spawn_text(
        world,
        anchor,
        &text.label,
        font,
        text.font_size as f32,
        color(&text.color, 1.0)?,
    );
    world.entity_mut(entity).insert((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            ..default()
        },
        TextLayout::no_wrap(),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(text.font_size as f32),
            weight: FontWeight(text.font_weight),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(
            match text.anchor {
                GameUiChartTextAnchor::Start => 0.0,
                GameUiChartTextAnchor::Middle => -50.0,
                GameUiChartTextAnchor::End => -100.0,
            },
            0.0,
        )),
        Visibility::Hidden,
        ChartTextBaseline,
    ));
    Ok(entity)
}

pub(super) fn align_chart_text_baselines(world: &mut World) {
    for (block, layout, mut node, mut visibility) in world
        .query_filtered::<(
            &ComputedTextBlock,
            &TextLayoutInfo,
            &mut Node,
            &mut Visibility,
        ), With<ChartTextBaseline>>()
        .iter_mut(world)
    {
        if let Some(line) = block.buffer().lines().next()
            && layout.scale_factor.is_finite()
            && layout.scale_factor > 0.0
        {
            let top = px(-line.metrics().baseline / layout.scale_factor);
            if node.top != top {
                node.top = top;
            }
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        }
    }
}

fn chart_line(world: &mut World, parent: Entity, start: Vec2, end: Vec2, ink: Color) -> Entity {
    let entity = world
        .spawn((ChildOf(parent), Node::default(), BackgroundColor(ink)))
        .id();
    set_chart_line(world, entity, start, end, ink);
    entity
}

fn set_chart_line(world: &mut World, entity: Entity, start: Vec2, end: Vec2, ink: Color) {
    let difference = end - start;
    let midpoint = (start + end) * 0.5;
    retained_node(world, entity).set_if_neq(Node {
        display: if difference.length() < 0.001 {
            Display::None
        } else {
            Display::Flex
        },
        position_type: PositionType::Absolute,
        left: px(midpoint.x - difference.length() * 0.5),
        top: px(midpoint.y - 0.75),
        width: px(difference.length()),
        height: px(1.5),
        ..default()
    });
    retained_ui_transform(world, entity).set_if_neq(UiTransform::from_rotation(Rot2::radians(
        difference.y.atan2(difference.x),
    )));
    world.entity_mut(entity).insert(BackgroundColor(ink));
}

pub(super) fn project_chart_cursor(
    world: &mut World,
    parent: Entity,
    cursor: &GameUiChartCursor,
) -> Result<(), String> {
    let mut retained = world
        .get::<RetainedChart>(parent)
        .cloned()
        .ok_or_else(|| "Chart静的投影が存在しない".to_owned())?;
    let position = |point: GameUiChartPoint| {
        Vec2::new(
            (retained.plot[0]
                + (point.x - retained.x[0]) / (retained.x[1] - retained.x[0])
                    * (retained.plot[2] - retained.plot[0])) as f32,
            (retained.plot[3]
                - (point.y - retained.y[0]) / (retained.y[1] - retained.y[0])
                    * (retained.plot[3] - retained.plot[1])) as f32,
        )
    };
    let (start, end) = cursor.cursor_x.map_or((Vec2::ZERO, Vec2::ZERO), |x| {
        (
            position(GameUiChartPoint {
                x,
                y: retained.y[0],
            }),
            position(GameUiChartPoint {
                x,
                y: retained.y[1],
            }),
        )
    });
    set_chart_line(world, retained.cursor, start, end, Color::WHITE);
    while retained.points.len() < cursor.cursor_points.len() {
        retained.points.push(
            world
                .spawn((
                    ChildOf(retained.surface),
                    Node::default(),
                    BackgroundColor(Color::WHITE),
                ))
                .id(),
        );
    }
    for (index, entity) in retained.points.iter().enumerate() {
        let sample = cursor.cursor_points.get(index);
        let point = sample.map(|sample| position(sample.point));
        retained_node(world, *entity).set_if_neq(Node {
            display: if point.is_some() {
                Display::Flex
            } else {
                Display::None
            },
            position_type: PositionType::Absolute,
            left: px(point.map_or(0.0, |point| point.x) - 4.0),
            top: px(point.map_or(0.0, |point| point.y) - 4.0),
            width: px(8),
            height: px(8),
            ..default()
        });
        if let Some(sample) = sample {
            world
                .entity_mut(*entity)
                .insert(BackgroundColor(color(&sample.color, 1.0)?));
        }
    }
    let wind = cursor
        .selected_wind_vector
        .as_ref()
        .and_then(|vector| vector.segment);
    if let (Some(vector), Some([start, end])) = (&cursor.selected_wind_vector, wind) {
        let ink = color(&vector.color, 1.0)?;
        let start = position(start);
        let end = position(end);
        let direction = (end - start).normalize_or_zero();
        let side = Vec2::new(-direction.y, direction.x);
        set_chart_line(world, retained.wind[0], start, end, ink);
        set_chart_line(
            world,
            retained.wind[1],
            end,
            end - direction * 7.0 + side * 3.0,
            ink,
        );
        set_chart_line(
            world,
            retained.wind[2],
            end,
            end - direction * 7.0 - side * 3.0,
            ink,
        );
    } else {
        for entity in retained.wind {
            set_chart_line(world, entity, Vec2::ZERO, Vec2::ZERO, Color::WHITE);
        }
    }
    world.entity_mut(parent).insert(retained);
    Ok(())
}
