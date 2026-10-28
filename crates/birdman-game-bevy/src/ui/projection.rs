use super::*;

pub(super) fn project_frame(
    world: &mut World,
    retained: &mut RetainedUi,
    frame: &GameUiFrame,
) -> Result<(), String> {
    let view = retained
        .view
        .as_ref()
        .ok_or_else(|| "UI viewが存在しない".to_owned())?;
    if frame.view_revision != view.view_revision {
        return Err("UI frameのview revisionが一致しない".to_owned());
    }
    project_hud(world, &mut retained.hud, frame.hud.as_ref(), &view.theme)?;
    let controls = view
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .collect::<Vec<_>>();
    for range in &frame.ranges {
        let control = controls
            .iter()
            .find(|control| control.id == range.id)
            .ok_or_else(|| format!("UI rangeが存在しない: {}", range.id))?;
        let widget = retained
            .widgets
            .get(&range.id)
            .ok_or_else(|| format!("UI range entityが存在しない: {}", range.id))?;
        set_text(
            world,
            widget.text,
            format!("{}: {}", control.label, range.display_value),
        );
    }
    for readout in &frame.readouts {
        let control = controls
            .iter()
            .find(|control| control.id == readout.id)
            .ok_or_else(|| format!("UI readoutが存在しない: {}", readout.id))?;
        let widget = retained
            .widgets
            .get(&readout.id)
            .ok_or_else(|| format!("UI readout entityが存在しない: {}", readout.id))?;
        set_text(
            world,
            widget.text,
            format!("{}: {}", control.label, readout.value),
        );
    }
    for button in &frame.button_labels {
        let widget = retained
            .widgets
            .get(&button.id)
            .ok_or_else(|| format!("UI button entityが存在しない: {}", button.id))?;
        set_text(world, widget.text, button.label.clone());
    }
    for group in &frame.fields {
        let widget = retained
            .widgets
            .get(&group.id)
            .ok_or_else(|| format!("UI numeric fields entityが存在しない: {}", group.id))?;
        for field in &group.fields {
            let entities = widget
                .field_text
                .get(&field.id)
                .ok_or_else(|| format!("UI numeric field entityが存在しない: {}", field.id))?;
            set_text(world, entities[0], field.label.clone());
            set_text(world, entities[1], field.value.clone());
            set_text(world, entities[2], field.unit.clone());
        }
    }
    if let Some(cursor) = &frame.chart_cursor {
        let widget = retained
            .widgets
            .get(&cursor.id)
            .ok_or_else(|| format!("UI chart entityが存在しない: {}", cursor.id))?;
        project_chart_cursor(world, widget.node, cursor)?;
    }
    Ok(())
}

pub(super) fn set_text(world: &mut World, entity: Entity, value: String) {
    let mut text = world.get_mut::<Text>(entity).expect("retained text exists");
    if text.0 != value {
        text.0 = value;
    }
}

pub(super) fn retained_node(world: &mut World, entity: Entity) -> Mut<'_, Node> {
    world
        .get_mut::<Node>(entity)
        .expect("retained UI node exists")
}

pub(super) fn retained_ui_transform(world: &mut World, entity: Entity) -> Mut<'_, UiTransform> {
    world
        .get_mut::<UiTransform>(entity)
        .expect("retained UI transform exists")
}

pub(super) fn project_view(
    world: &mut World,
    retained: &mut RetainedUi,
    view: &Arc<GameUiView>,
    font: &Handle<Font>,
) -> Result<(), String> {
    if retained.view.as_ref() == Some(view) {
        return Ok(());
    }
    if retained
        .view
        .as_ref()
        .is_none_or(|previous| previous.theme != view.theme)
    {
        retained.hud.previous = None;
    }
    let text_color = color(&view.theme.text, 1.0)?;
    let background = color(
        &view.theme.panel_background,
        if view.layout.passthrough {
            0.0
        } else {
            view.theme.panel_opacity
        },
    )?;
    world.entity_mut(retained.shell).insert((
        shell_node(&view.layout),
        BackgroundColor(background),
        if view.layout.passthrough {
            Pickable::IGNORE
        } else {
            Pickable::default()
        },
        if matches!(
            view.layout.shell,
            GameUiShell::Center | GameUiShell::Analysis
        ) {
            UiTransform::from_translation(Val2::percent(-50, -50))
        } else {
            UiTransform::IDENTITY
        },
    ));
    let show_headings =
        view.layout.shell == GameUiShell::Center || view.layout.shell == GameUiShell::PauseSettings;
    world.entity_mut(retained.heading).insert((
        Text::new(&view.title),
        TextColor(text_color),
        Node {
            display: if show_headings {
                Display::Flex
            } else {
                Display::None
            },
            flex_shrink: 0.0,
            ..default()
        },
    ));
    world.entity_mut(retained.description).insert((
        Text::new(&view.description),
        TextColor(color(&view.theme.muted_text, 1.0)?),
        Node {
            display: if show_headings {
                Display::Flex
            } else {
                Display::None
            },
            flex_shrink: 0.0,
            ..default()
        },
    ));
    world.entity_mut(retained.content).insert(Node {
        width: percent(100),
        height: auto(),
        position_type: PositionType::Relative,
        flex_direction: FlexDirection::Column,
        row_gap: px(view.layout.gap_px),
        flex_shrink: 0.0,
        ..default()
    });
    let rebuild = retained
        .view
        .as_ref()
        .is_none_or(|previous| structure(previous) != structure(view));
    let focus_id = world.resource::<InputFocus>().get().and_then(|focused| {
        retained
            .widgets
            .iter()
            .find_map(|(id, widget)| (widget.node == focused).then(|| id.clone()))
    });
    if retained
        .view
        .as_ref()
        .is_some_and(|previous| previous.interaction_scope != view.interaction_scope)
        && let Some(mut scroll) = world.get_mut::<ScrollPosition>(retained.shell)
    {
        scroll.y = 0.0;
    }
    if rebuild {
        let children = world
            .entity(retained.content)
            .get::<Children>()
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for child in children {
            world.despawn(child);
        }
        retained.sections.clear();
        retained.widgets.clear();
        for panel in &view.panels {
            for section in &panel.sections {
                let group = world
                    .spawn((
                        ChildOf(retained.content),
                        Node {
                            width: percent(100),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            row_gap: px(view.layout.gap_px),
                            ..default()
                        },
                    ))
                    .id();
                if !section.label.is_empty() {
                    spawn_text(world, group, &section.label, font, 16.0, text_color);
                }
                let columns = world
                    .spawn((ChildOf(group), section_node(section.layout, &view.layout)))
                    .id();
                retained.sections.insert(section.id.clone(), columns);
            }
            for control in &panel.controls {
                let parent = *retained
                    .sections
                    .get(&control.section)
                    .ok_or_else(|| format!("UI sectionが存在しない: {}", control.section))?;
                let node = world
                    .spawn((
                        ChildOf(parent),
                        Node::default(),
                        Name::new(control.id.clone()),
                    ))
                    .id();
                if matches!(control.content, GameUiControlContent::Chart { .. }) {
                    world.entity_mut(node).observe(scroll_content);
                }
                let text = spawn_text(world, node, "", font, 16.0, text_color);
                retained.widgets.insert(
                    control.id.clone(),
                    Widget {
                        node,
                        text,
                        field_text: BTreeMap::new(),
                    },
                );
            }
        }
        if let Some(id) = focus_id
            && let Some(widget) = retained.widgets.get(&id)
        {
            world
                .resource_mut::<InputFocus>()
                .set(widget.node, FocusCause::Navigated);
        }
    }
    for panel in &view.panels {
        for control in &panel.controls {
            let widget = retained
                .widgets
                .get_mut(&control.id)
                .ok_or_else(|| format!("UI controlを取得できない: {}", control.id))?;
            let previous = retained.view.as_ref().and_then(|previous| {
                previous
                    .panels
                    .iter()
                    .flat_map(|panel| &panel.controls)
                    .find(|previous| previous.id == control.id)
            });
            if rebuild
                || previous != Some(control)
                || retained.view.as_ref().is_some_and(|previous| {
                    previous.layout != view.layout || previous.theme != view.theme
                })
            {
                let section_layout = panel
                    .sections
                    .iter()
                    .find(|section| section.id == control.section)
                    .map_or(GameUiSectionLayout::Column, |section| section.layout);
                project_control(
                    world,
                    widget,
                    control,
                    &view.theme,
                    font,
                    section_layout,
                    &view.layout,
                )?;
            }
            if let Some(surface) = world
                .get::<RetainedChart>(widget.node)
                .map(|chart| chart.viewport)
                && let Some(mut viewport) = world.get_mut::<ChartViewport>(surface)
                && viewport.view_revision != view.view_revision
            {
                viewport.view_revision = view.view_revision;
                viewport.measured = None;
            }
        }
    }
    retained.view = Some(Arc::clone(view));
    Ok(())
}

fn structure(view: &GameUiView) -> Vec<String> {
    let mut result = vec![format!(
        "{:?}/{:?}/{:?}",
        view.scene, view.active_overlay, view.layout.shell
    )];
    for panel in &view.panels {
        for section in &panel.sections {
            result.push(format!("{}/{}/{:?}", panel.id, section.id, section.layout));
        }
        for control in &panel.controls {
            let kind = match control.content {
                GameUiControlContent::Button { .. } => "button",
                GameUiControlContent::Toggle { .. } => "toggle",
                GameUiControlContent::Range { .. } => "range",
                GameUiControlContent::Status { .. } => "status",
                GameUiControlContent::Fields { .. } => "fields",
                GameUiControlContent::Chart { .. } => "chart",
            };
            result.push(format!("{}/{}/{kind}", control.id, control.section));
            if let GameUiControlContent::Fields { fields, .. } = &control.content {
                result.extend(
                    fields
                        .iter()
                        .map(|field| format!("{}/{}", control.id, field.id)),
                );
            }
        }
    }
    result
}

fn shell_node(layout: &GameUiLayout) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: px(layout.width_px),
        max_height: px(layout.maximum_height_px),
        padding: UiRect::all(px(layout.padding_px)),
        flex_direction: FlexDirection::Column,
        row_gap: px(layout.gap_px),
        overflow: if layout.scroll_y {
            Overflow::scroll_y()
        } else {
            Overflow::clip()
        },
        ..default()
    };
    match layout.shell {
        GameUiShell::Center => {
            node.left = percent(50);
            node.top = percent(50);
        }
        GameUiShell::FlightBar => {
            node.right = px(0);
            node.bottom = px(0);
        }
        GameUiShell::PauseSettings | GameUiShell::Cinematic => {
            node.top = px(12);
            node.right = px(12);
        }
        GameUiShell::Analysis => {
            node.left = percent(50);
            node.top = percent(50);
            node.height = px(layout.maximum_height_px);
        }
    }
    node
}

pub(super) fn section_node(section: GameUiSectionLayout, layout: &GameUiLayout) -> Node {
    Node {
        width: percent(100),
        min_width: px(0),
        flex_direction: if section == GameUiSectionLayout::Row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        row_gap: px(layout.gap_px),
        column_gap: px(layout.gap_px),
        display: if section == GameUiSectionLayout::ChoiceGrid {
            Display::Grid
        } else {
            Display::Flex
        },
        grid_template_columns: if section == GameUiSectionLayout::ChoiceGrid {
            vec![RepeatedGridTrack::flex(layout.choice_columns as u16, 1.0)]
        } else {
            Vec::new()
        },
        grid_auto_rows: if section == GameUiSectionLayout::ChoiceGrid {
            vec![GridTrack::max_content()]
        } else {
            Vec::new()
        },
        flex_wrap: if section == GameUiSectionLayout::Row {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        justify_content: if section == GameUiSectionLayout::Row
            && layout.shell == GameUiShell::FlightBar
        {
            JustifyContent::FlexEnd
        } else {
            JustifyContent::FlexStart
        },
        ..default()
    }
}

fn project_control(
    world: &mut World,
    widget: &mut Widget,
    control: &GameUiControl,
    theme: &GameUiTheme,
    font: &Handle<Font>,
    section_layout: GameUiSectionLayout,
    layout: &GameUiLayout,
) -> Result<(), String> {
    let mut node = Node {
        width: percent(100),
        min_width: px(0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(px(8)),
        row_gap: px(4),
        ..default()
    };
    if section_layout == GameUiSectionLayout::Row {
        node.width = auto();
        node.max_width = percent(100);
        node.flex_grow = if matches!(
            layout.shell,
            GameUiShell::FlightBar | GameUiShell::PauseSettings
        ) {
            0.0
        } else {
            1.0
        };
    }
    let mut background = Color::NONE;
    let mut label = control.label.clone();
    match &control.content {
        GameUiControlContent::Button {
            action,
            presentation,
        } => {
            node.min_height = px(theme.button_min_height_px);
            background = color(&theme.normal_background, 1.0)?;
            match presentation {
                GameUiButtonPresentation::Action {
                    emphasis: GameUiEmphasis::Primary,
                } => {
                    node.min_height = px(theme.primary_min_height_px);
                    background = color(&theme.primary_background, 1.0)?;
                }
                GameUiButtonPresentation::Action {
                    emphasis: GameUiEmphasis::Secondary,
                }
                | GameUiButtonPresentation::Disclosure { .. } => background = Color::NONE,
                GameUiButtonPresentation::Choice {
                    selected,
                    description,
                    ..
                } => {
                    background = color(
                        if *selected {
                            &theme.primary_background
                        } else {
                            &theme.choice_background
                        },
                        1.0,
                    )?;
                    label = format!("{}{}", if *selected { "● " } else { "" }, control.label);
                    if !description.is_empty() {
                        label.push('\n');
                        label.push_str(description);
                    }
                }
                _ => {}
            }
            if let GameUiButtonPresentation::Disclosure { expanded } = presentation {
                label = format!("{} {}", if *expanded { "▾" } else { "▸" }, control.label);
            }
            world.entity_mut(widget.node).insert((
                Button,
                MenuButton {
                    control_id: control.id.clone(),
                    enabled: control.enabled,
                },
            ));
            if let GameUiAction::Platform {
                command: GameUiPlatformCommand::SetWaterQuality { .. },
            } = action
            {
                world
                    .entity_mut(widget.text)
                    .insert(UiText::WaterQualityChoice);
            }
        }
        GameUiControlContent::Toggle { value, .. } => {
            node.min_height = px(theme.button_min_height_px);
            background = color(&theme.choice_background, 1.0)?;
            label = format!("{} {}", if *value { "☑" } else { "☐" }, control.label);
            world.entity_mut(widget.node).insert((
                Button,
                MenuButton {
                    enabled: control.enabled,
                    control_id: control.id.clone(),
                },
            ));
        }
        GameUiControlContent::Range { display_value, .. } => {
            node.min_height = px(theme.button_min_height_px);
            label = format!("{}: {display_value}", control.label);
            background = color(&theme.choice_background, 1.0)?;
            world.entity_mut(widget.node).insert((
                Button,
                RelativeCursorPosition::default(),
                MenuRange {
                    control_id: control.id.clone(),
                    enabled: control.enabled,
                },
            ));
        }
        GameUiControlContent::Status { value, .. } => {
            label = if control.label.is_empty() {
                value.clone()
            } else {
                format!("{}: {value}", control.label)
            };
            if control.id == "game-briefing-technical-content" {
                world
                    .entity_mut(widget.text)
                    .insert(UiText::TechnicalDetails);
            }
        }
        GameUiControlContent::Fields { fields, .. } => {
            let ink = color(&theme.text, 1.0)?;
            let muted = color(&theme.muted_text, 1.0)?;
            let grid = if let Some(children) = world.get::<Children>(widget.node)
                && let Some(grid) = children.iter().find(|child| *child != widget.text)
            {
                grid
            } else {
                world.spawn((ChildOf(widget.node), Node::default())).id()
            };
            world.entity_mut(grid).insert(Node {
                width: percent(100),
                display: Display::Grid,
                grid_template_columns: vec![RepeatedGridTrack::flex(
                    layout.choice_columns as u16,
                    1.0,
                )],
                row_gap: px(layout.gap_px),
                column_gap: px(layout.gap_px),
                ..default()
            });
            for field in fields {
                let entities = if let Some(entities) = widget.field_text.get(&field.id) {
                    *entities
                } else {
                    let card = world
                        .spawn((
                            ChildOf(grid),
                            Node {
                                min_width: px(0),
                                padding: UiRect::all(px(8)),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(4),
                                ..default()
                            },
                            BackgroundColor(color(&theme.choice_background, 1.0)?),
                        ))
                        .id();
                    let heading = spawn_text(world, card, &field.label, font, 14.0, muted);
                    let row = world
                        .spawn((
                            ChildOf(card),
                            Node {
                                justify_content: JustifyContent::FlexEnd,
                                align_items: AlignItems::Baseline,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: px(5.6),
                                ..default()
                            },
                        ))
                        .id();
                    let value = spawn_text(world, row, &field.value, font, 16.0, ink);
                    world
                        .get_mut::<TextFont>(value)
                        .expect("numeric text font exists")
                        .font_features = FontFeatures::builder()
                        .enable(FontFeatureTag::TABULAR_FIGURES)
                        .build();
                    let unit = spawn_text(world, row, &field.unit, font, 14.0, muted);
                    let entities = [heading, value, unit];
                    widget.field_text.insert(field.id.clone(), entities);
                    entities
                };
                set_text(world, entities[0], field.label.clone());
                set_text(world, entities[1], field.value.clone());
                set_text(world, entities[2], field.unit.clone());
                world.entity_mut(entities[0]).insert(TextColor(muted));
                world.entity_mut(entities[1]).insert(TextColor(ink));
                world.entity_mut(entities[2]).insert(TextColor(muted));
                if let Some(card) = world.get::<ChildOf>(entities[0]).map(ChildOf::parent) {
                    world
                        .entity_mut(card)
                        .insert(BackgroundColor(color(&theme.choice_background, 1.0)?));
                }
            }
        }
        GameUiControlContent::Chart { plot, .. } => {
            label = control.label.clone();
            background = color(&theme.panel_background, theme.panel_opacity)?;
            node.overflow = if plot.presentation.scroll_overflow {
                Overflow::scroll()
            } else {
                Overflow::clip()
            };
            if world.get::<ScrollPosition>(widget.node).is_none() {
                world
                    .entity_mut(widget.node)
                    .insert(ScrollPosition::default());
            }
            let root_font_size = viewport(world).root_font_size;
            let legend_ink = color(&theme.text, 1.0)?;
            project_chart(
                world,
                widget.node,
                widget.text,
                plot,
                root_font_size,
                legend_ink,
                font,
            )?;
            let chart = world
                .get::<RetainedChart>(widget.node)
                .expect("projected chart exists");
            let (viewport, canvas) = (chart.viewport, chart.surface);
            if world.get::<ChartViewport>(viewport).is_none() {
                world.entity_mut(viewport).insert(ChartViewport {
                    canvas,
                    control_id: control.id.clone(),
                    view_revision: 0,
                    measured: None,
                });
            }
        }
    }
    world
        .entity_mut(widget.node)
        .insert((node, BackgroundColor(background)));
    if control.enabled
        && !matches!(
            control.content,
            GameUiControlContent::Status { .. }
                | GameUiControlContent::Fields { .. }
                | GameUiControlContent::Chart { .. }
        )
    {
        world.entity_mut(widget.node).insert(TabIndex(0));
    } else {
        world.entity_mut(widget.node).remove::<TabIndex>();
        if world.resource::<InputFocus>().get() == Some(widget.node) {
            world.resource_mut::<InputFocus>().clear();
        }
    }
    world.entity_mut(widget.text).insert((
        Text::new(label),
        TextColor(color(
            &theme.text,
            if control.enabled
                || matches!(
                    control.content,
                    GameUiControlContent::Status { .. }
                        | GameUiControlContent::Fields { .. }
                        | GameUiControlContent::Chart { .. }
                )
            {
                1.0
            } else {
                0.45
            },
        )?),
    ));
    Ok(())
}
