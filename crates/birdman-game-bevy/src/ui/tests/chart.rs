use super::*;

#[test]
fn chart_projects_shared_ticks_labels_grid_legend_and_map_guides() {
    let mut app = analysis_app();
    for selection in [
        "game-analysis-map",
        "game-analysis-altitude",
        "game-analysis-speed",
    ] {
        press(&mut app, selection);
        let (chart, plot) = projected_chart(&app);
        let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
        let presentation = &plot.presentation;
        assert_eq!(
            app.world().get::<RetainedChart>(chart).unwrap().plot,
            presentation.plot
        );
        for label in [&presentation.x_axis, &presentation.y_axis]
            .into_iter()
            .chain(presentation.x_ticks.iter().map(|tick| &tick.text))
            .chain(presentation.y_ticks.iter().map(|tick| &tick.text))
            .chain(&presentation.marker_labels)
            .chain(&presentation.time_marker_labels)
            .chain(&presentation.reference_labels)
        {
            assert_chart_text(app.world(), surface, label);
        }
        for segment in &presentation.grid_lines {
            assert_chart_segment(
                app.world(),
                surface,
                *segment,
                color(&presentation.grid_color, presentation.grid_opacity).unwrap(),
            );
        }
        if let Some(guides) = &presentation.map_guides {
            for (segments, label) in [
                (&guides.scale_segments, &guides.scale_label),
                (&guides.north_segments, &guides.north_label),
            ] {
                assert_chart_text(app.world(), surface, label);
                for segment in segments {
                    assert_chart_segment(
                        app.world(),
                        surface,
                        *segment,
                        color(&label.color, 1.0).unwrap(),
                    );
                }
            }
        }
        let legend = app
            .world()
            .get::<Children>(chart)
            .unwrap()
            .iter()
            .find(|entity| *entity != surface && app.world().get::<Children>(*entity).is_some())
            .unwrap();
        let items = app.world().get::<Children>(legend).unwrap();
        assert_eq!(items.len(), presentation.legend.len());
        for (item, expected) in items.iter().zip(&presentation.legend) {
            let children = app.world().get::<Children>(item).unwrap();
            let swatch = children[0];
            assert_eq!(
                app.world().get::<Text>(children[1]).unwrap().0,
                expected.label
            );
            assert_eq!(
                app.world().get::<BackgroundColor>(swatch).unwrap().0,
                color(&expected.color, 1.0).unwrap()
            );
            match &expected.segment_gradient {
                Some([start, end]) => assert_eq!(
                    app.world().get::<BackgroundGradient>(swatch).unwrap(),
                    &BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::new(color(start, 1.0).unwrap(), percent(0)),
                        ColorStop::new(color(end, 1.0).unwrap(), percent(100)),
                    ]))
                ),
                None => assert!(app.world().get::<BackgroundGradient>(swatch).is_none()),
            }
        }
    }
}

#[test]
fn chart_legend_layout_keeps_shared_names_with_rasterized_glyphs_and_disjoint_items() {
    let bounds = |world: &World, entity: Entity| {
        Rect::from_center_size(
            world.get::<UiGlobalTransform>(entity).unwrap().translation,
            world.get::<ComputedNode>(entity).unwrap().size(),
        )
    };
    for width in [360, 800] {
        for replay in [false, true] {
            let mut app = analysis_app_size(width as f32, 600.0);
            if replay {
                press(&mut app, "game-result-replay");
                press(&mut app, "game-replay-view-mode");
                press(&mut app, "game-replay-view-mode");
            }
            add_ui_layout(&mut app, width, 600);
            for selection in [
                "game-analysis-map",
                "game-analysis-altitude",
                "game-analysis-speed",
            ] {
                press(&mut app, selection);
                for _ in 0..4 {
                    app.update();
                }
                let (chart, plot) = projected_chart(&app);
                let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
                let legend = app
                    .world()
                    .get::<Children>(chart)
                    .unwrap()
                    .iter()
                    .find(|entity| {
                        *entity != surface && app.world().get::<Children>(*entity).is_some()
                    })
                    .unwrap();
                let items = app.world().get::<Children>(legend).unwrap();
                assert_eq!(items.len(), plot.presentation.legend.len());
                for (index, (item, entry)) in
                    items.iter().zip(&plot.presentation.legend).enumerate()
                {
                    let children = app.world().get::<Children>(item).unwrap();
                    let swatch_bounds = bounds(app.world(), children[0]);
                    let text = children[1];
                    let text_bounds = bounds(app.world(), text);
                    let item_bounds = bounds(app.world(), item);
                    assert_text_ink_within_node(app.world(), text);
                    let layout = app.world().get::<TextLayoutInfo>(text).unwrap();
                    assert_eq!(app.world().get::<Text>(text).unwrap().0, entry.label);
                    assert!(
                        !layout.glyphs.is_empty()
                            && layout.size.is_finite()
                            && layout.size.min_element() > 0.0,
                        "Legend glyphs collapsed: {} node={:?} layout={:?}",
                        entry.label,
                        text_bounds,
                        layout.size
                    );
                    let block = app.world().get::<ComputedTextBlock>(text).unwrap();
                    let mut glyph_sources = Vec::new();
                    for line in block.buffer().lines() {
                        for run in line.runs() {
                            for cluster in run.visual_clusters() {
                                let source = &entry.label[cluster.text_range()];
                                for glyph in cluster.glyphs() {
                                    if u16::try_from(glyph.id).is_ok() {
                                        glyph_sources.push(source);
                                    }
                                }
                            }
                        }
                    }
                    assert_eq!(
                        glyph_sources.len(),
                        layout.glyphs.len(),
                        "Legend source clusters differ from rendered glyphs: {}",
                        entry.label
                    );
                    assert_eq!(
                        app.world().get::<TextLayout>(text).unwrap().linebreak,
                        LineBreak::NoWrap
                    );
                    let mut visible_ink = 0;
                    for (glyph, source) in layout.glyphs.iter().zip(glyph_sources) {
                        let ink_size = glyph.atlas_info.rect.size();
                        assert!(
                            glyph.position.is_finite()
                                && glyph.atlas_info.offset.is_finite()
                                && glyph.atlas_info.rect.min.is_finite()
                                && glyph.atlas_info.rect.max.is_finite()
                                && ink_size.min_element() >= 0.0,
                            "Legend glyph has invalid geometry: {} source={source:?} glyph={glyph:?}",
                            entry.label
                        );
                        assert_eq!(
                            glyph.line_index, 0,
                            "Legend glyph wrapped: {} source={source:?} glyph={glyph:?}",
                            entry.label
                        );
                        if ink_size.min_element() > 0.0 {
                            visible_ink += 1;
                        } else {
                            assert!(
                                source.chars().all(char::is_whitespace),
                                "Non-whitespace legend glyph has no visible ink: {} source={source:?} glyph={glyph:?}",
                                entry.label
                            );
                        }
                    }
                    assert!(
                        visible_ink > 0,
                        "Legend has no visible ink: {}",
                        entry.label
                    );
                    assert!(text_bounds.size().x + 1.0 >= layout.size.x);
                    assert!(text_bounds.size().y + 1.0 >= layout.size.y);
                    assert!(item_bounds.min.cmple(text_bounds.min + Vec2::ONE).all());
                    assert!(item_bounds.max.cmpge(text_bounds.max - Vec2::ONE).all());
                    assert!(swatch_bounds.max.x < text_bounds.min.x);
                    for other in items.iter().skip(index + 1) {
                        let other_bounds = bounds(app.world(), other);
                        assert!(
                            item_bounds.max.x <= other_bounds.min.x
                                || other_bounds.max.x <= item_bounds.min.x
                                || item_bounds.max.y <= other_bounds.min.y
                                || other_bounds.max.y <= item_bounds.min.y
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn chart_observes_logical_viewport_and_retains_surface_across_reprojection() {
    let mut app = analysis_app();
    let (chart, _) = projected_chart(&app);
    let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
    app.world_mut().entity_mut(surface).insert((
        ComputedNode {
            size: Vec2::new(800.0, 480.0),
            inverse_scale_factor: 0.5,
            ..default()
        },
        InheritedVisibility::VISIBLE,
    ));
    let extent = GameUiChartExtent {
        width: 400.0,
        height: 240.0,
    };
    assert_eq!(platform_input(app.world_mut()).chart_extent, Some(extent));
    app.update();
    let (updated, plot) = projected_chart(&app);
    assert_eq!(updated, chart);
    assert_eq!(plot.presentation.viewport, extent);
    assert_eq!(
        app.world().get::<RetainedChart>(chart).unwrap().surface,
        surface
    );
    app.world_mut()
        .entity_mut(surface)
        .insert(InheritedVisibility::HIDDEN);
    assert!(platform_input(app.world_mut()).chart_extent.is_none());
    app.world_mut().entity_mut(surface).insert((
        InheritedVisibility::VISIBLE,
        ComputedNode {
            size: Vec2::ZERO,
            ..default()
        },
    ));
    assert!(platform_input(app.world_mut()).chart_extent.is_none());
    app.world_mut().entity_mut(surface).insert(ComputedNode {
        size: Vec2::new(139.0, 100.0),
        inverse_scale_factor: 1.0,
        ..default()
    });
    assert!(platform_input(app.world_mut()).chart_extent.is_none());
}

#[test]
fn speed_cursor_projects_series_colors_and_retains_points_during_seek() {
    let mut app = analysis_app();
    press(&mut app, "game-analysis-speed");
    let (chart, plot) = projected_chart(&app);
    let expected = plot.cursor_points.clone();
    let retained = app.world().get::<RetainedChart>(chart).unwrap().clone();
    assert_eq!(expected.len(), 2);
    assert_ne!(expected[0].color, expected[1].color);
    let revision = app
        .world()
        .resource::<RetainedUi>()
        .view
        .as_ref()
        .unwrap()
        .view_revision;
    for (entity, expected) in retained.points.iter().zip(&expected) {
        assert_eq!(
            app.world().get::<BackgroundColor>(*entity).unwrap().0,
            color(&expected.color, 1.0).unwrap()
        );
    }
    let initial = app.world().get::<Node>(retained.points[0]).unwrap().left;
    let platform = platform_input(app.world_mut());
    app.world_mut()
        .resource_mut::<NativeSession>()
        .update_ui_action(GameUiAction::SeekAnalysis { time_seconds: 0.1 }, &platform)
        .unwrap();
    app.update();
    assert_eq!(
        app.world()
            .resource::<RetainedUi>()
            .view
            .as_ref()
            .unwrap()
            .view_revision,
        revision
    );
    assert_eq!(
        app.world().get::<RetainedChart>(chart).unwrap().points,
        retained.points
    );
    assert_ne!(
        app.world().get::<Node>(retained.points[0]).unwrap().left,
        initial
    );
    for (entity, expected) in retained.points.iter().zip(&expected) {
        assert_eq!(
            app.world().get::<BackgroundColor>(*entity).unwrap().0,
            color(&expected.color, 1.0).unwrap()
        );
    }
}

#[test]
fn frame_projection_preserves_static_chart_entities_and_rejects_stale_revision() {
    let mut app = analysis_app();
    let (chart, revision, children, surface, static_children, cursor) = {
        let retained = app.world().resource::<RetainedUi>();
        let chart = retained.widgets["game-analysis-plot"].node;
        let surface = app.world().get::<RetainedChart>(chart).unwrap().surface;
        (
            chart,
            retained.view.as_ref().unwrap().view_revision,
            app.world()
                .get::<Children>(chart)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            surface,
            app.world()
                .get::<Children>(surface)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            app.world().get::<RetainedChart>(chart).unwrap().cursor,
        )
    };
    let projected = app.world().get::<RetainedChart>(chart).unwrap();
    let cursor_entities = std::iter::once(projected.cursor)
        .chain(projected.points.iter().copied())
        .chain(projected.wind)
        .collect::<Vec<_>>();
    let geometry = |world: &World| {
        cursor_entities
            .iter()
            .map(|entity| {
                let entity = world.entity(*entity);
                let node = entity.get_ref::<Node>().unwrap();
                let transform = entity.get_ref::<UiTransform>().unwrap();
                (
                    (*node).clone(),
                    node.last_changed(),
                    *transform,
                    transform.last_changed(),
                )
            })
            .collect::<Vec<_>>()
    };
    let previous_geometry = geometry(app.world());
    for _ in 0..3 {
        app.update();
        assert_eq!(geometry(app.world()), previous_geometry);
    }
    let platform = platform_input(app.world_mut());
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        session
            .update_ui_action(GameUiAction::SeekAnalysis { time_seconds: 0.1 }, &platform)
            .unwrap();
    }
    app.update();
    let current_geometry = geometry(app.world());
    assert!(
        previous_geometry
            .iter()
            .zip(&current_geometry)
            .any(|(previous, current)| previous.0 != current.0 || previous.2 != current.2)
    );
    assert!(
        previous_geometry
            .iter()
            .zip(&current_geometry)
            .any(|(previous, current)| previous.1 != current.1 || previous.3 != current.3)
    );
    assert_eq!(
        app.world()
            .resource::<RetainedUi>()
            .view
            .as_ref()
            .unwrap()
            .view_revision,
        revision
    );
    assert_eq!(
        app.world()
            .get::<Children>(chart)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        children
    );
    assert_eq!(
        app.world()
            .get::<Children>(surface)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        static_children
    );
    assert_eq!(
        app.world().get::<RetainedChart>(chart).unwrap().cursor,
        cursor
    );
    let view = app.world().resource::<RetainedUi>().view.as_ref().unwrap();
    let mut frame = app
        .world()
        .resource::<NativeSession>()
        .ui_frame(&platform, view, view.view_revision)
        .unwrap();
    frame.view_revision += 1;
    let mut retained = app.world_mut().remove_resource::<RetainedUi>().unwrap();
    assert!(project_frame(app.world_mut(), &mut retained, &frame).is_err());
    app.world_mut().insert_resource(retained);
    assert_eq!(geometry(app.world()), current_geometry);
}
