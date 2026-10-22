use super::super::native_session::MenuAction;
use super::*;
use birdman_game_app::application::{RecordCommand, SessionCommand};
use birdman_game_presentation::game_ui::{GameUiPauseOverlay, GameUiResultTab};

pub(super) fn app(width: f32, height: f32) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<NativeSession>()
        .init_resource::<WaterQualitySelection>()
        .init_resource::<NativeScreenRender>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .init_resource::<Time<Real>>()
        .init_resource::<super::super::CameraMode>()
        .insert_resource(NativeFont(Handle::default()))
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(UiScale(1.0))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, (button_actions, update_ui).chain());
    app.world_mut().spawn(Window {
        resolution: (width as u32, height as u32).into(),
        ..default()
    });
    app.update();
    app
}

pub(super) fn press(app: &mut App, id: &str) {
    let node = app
        .world()
        .resource::<RetainedUi>()
        .widgets
        .get(id)
        .unwrap()
        .node;
    app.world_mut()
        .entity_mut(node)
        .insert(Interaction::Pressed);
    app.update();
    if let Ok(mut entity) = app.world_mut().get_entity_mut(node) {
        entity.insert(Interaction::None);
    }
    app.update();
}

fn labels(app: &App) -> Vec<String> {
    let retained = app.world().resource::<RetainedUi>();
    let view = retained.view.as_ref().unwrap();
    view.panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .map(|control| {
            app.world()
                .get::<Text>(retained.widgets[&control.id].text)
                .unwrap()
                .0
                .clone()
        })
        .collect()
}

fn analysis_app() -> App {
    analysis_app_size(1280.0, 720.0)
}

fn analysis_app_size(width: f32, height: f32) -> App {
    let mut app = app(width, height);
    press(&mut app, "game-title-start");
    press(&mut app, "game-setup-start");
    press(&mut app, "game-briefing-start");
    {
        let mut session = app.world_mut().resource_mut::<NativeSession>();
        for _ in 0..3 {
            session.presentation_frame(1.0);
        }
        for _ in 0..20 {
            session.tick(super::super::native_session::FlightInput::default());
        }
        session.action(MenuAction::Abort).unwrap();
    }
    app.update();
    press(&mut app, "game-result-open-analysis");
    app
}

pub(super) fn add_ui_layout(app: &mut App, width: u32, height: u32) -> Entity {
    use bevy::{
        asset::{AssetApp, AssetPlugin},
        camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
        image::{ImagePlugin, TextureAtlasLayout},
        input::InputPlugin,
        picking::DefaultPickingPlugins,
        text::TextPlugin,
        ui::UiPlugin,
        window::{ExitCondition, WindowPlugin},
    };
    app.add_plugins((
        AssetPlugin::default(),
        InputPlugin,
        WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        },
        ImagePlugin::default(),
        TextPlugin,
        DefaultPickingPlugins,
        UiPlugin,
    ))
    .init_asset::<TextureAtlasLayout>();
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            bevy::asset::AssetId::default(),
            Font::from_bytes(
                include_bytes!("../../../../assets/test-fonts/FiraMono-subset.ttf").to_vec(),
            ),
        )
        .unwrap();
    app.world_mut()
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(width, height),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size: UVec2::new(width, height),
                    ..default()
                }),
                ..default()
            },
        ))
        .id()
}

fn projected_chart(app: &App) -> (Entity, &GameUiChart) {
    let retained = app.world().resource::<RetainedUi>();
    let chart = retained.widgets["game-analysis-plot"].node;
    let plot = retained
        .view
        .as_ref()
        .unwrap()
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .find_map(|control| match &control.content {
            GameUiControlContent::Chart { plot, .. } => Some(plot),
            _ => None,
        })
        .unwrap();
    (chart, plot)
}

fn assert_chart_text(world: &World, surface: Entity, expected: &GameUiChartText) {
    let anchor = world
        .get::<Children>(surface)
        .unwrap()
        .iter()
        .find(|entity| {
            world.get::<Node>(*entity).is_some_and(|node| {
                node.left == px(expected.position.x) && node.top == px(expected.position.y)
            }) && world.get::<Children>(*entity).is_some_and(|children| {
                children.iter().any(|child| {
                    world
                        .get::<Text>(child)
                        .is_some_and(|text| text.0 == expected.label)
                })
            })
        })
        .unwrap();
    let text = world.get::<Children>(anchor).unwrap()[0];
    assert_eq!(
        world.get::<UiTransform>(anchor).unwrap().rotation,
        Rot2::radians(expected.rotation_degrees.to_radians() as f32)
    );
    assert_eq!(
        world.get::<UiTransform>(text).unwrap().translation,
        Val2::percent(
            match expected.anchor {
                GameUiChartTextAnchor::Start => 0.0,
                GameUiChartTextAnchor::Middle => -50.0,
                GameUiChartTextAnchor::End => -100.0,
            },
            0.0,
        )
    );
    let font = world.get::<TextFont>(text).unwrap();
    assert_eq!(font.font_size, FontSize::Px(expected.font_size as f32));
    assert_eq!(font.weight, FontWeight(expected.font_weight));
    assert_eq!(
        world.get::<TextColor>(text).unwrap().0,
        color(&expected.color, 1.0).unwrap()
    );
    assert!(world.get::<ChartTextBaseline>(text).is_some());
}

fn assert_text_ink_within_node(world: &World, entity: Entity) {
    let node = world.get::<ComputedNode>(entity).unwrap();
    let layout = world.get::<TextLayoutInfo>(entity).unwrap();
    let text = &world.get::<Text>(entity).unwrap().0;
    assert_eq!(
        world.get::<Node>(entity).unwrap().overflow,
        Overflow::clip()
    );
    assert!(node.size.is_finite() && layout.size.is_finite());
    let clip = Rect::from_corners(-Vec2::ONE, node.size + Vec2::ONE);
    for glyph in &layout.glyphs {
        let ink_size = glyph.atlas_info.rect.size();
        if ink_size.min_element() > 0.0 {
            let ink = Rect::from_center_size(glyph.position, ink_size);
            assert!(
                clip.contains(ink.min) && clip.contains(ink.max),
                "Text ink exceeds its clip: {text:?}, node={:?}, ink={ink:?}",
                node.size
            );
        }
    }
}

fn assert_chart_segment(
    world: &World,
    surface: Entity,
    segment: [GameUiChartPoint; 2],
    ink: Color,
) {
    let start = Vec2::new(segment[0].x as f32, segment[0].y as f32);
    let end = Vec2::new(segment[1].x as f32, segment[1].y as f32);
    let midpoint = (start + end) * 0.5;
    let length = (end - start).length();
    assert!(
        world
            .get::<Children>(surface)
            .unwrap()
            .iter()
            .any(|entity| {
                world.get::<Node>(entity).is_some_and(|node| {
                    node.left == px(midpoint.x - length * 0.5)
                        && node.top == px(midpoint.y - 0.75)
                        && node.width == px(length)
                }) && world
                    .get::<BackgroundColor>(entity)
                    .is_some_and(|color| color.0 == ink)
            })
    );
}

#[path = "tests/flow.rs"]
mod flow;

#[path = "tests/settings.rs"]
mod settings;

#[path = "tests/archive.rs"]
mod archive;

#[path = "tests/chart.rs"]
mod chart;

#[path = "tests/chart_input.rs"]
mod chart_input;

#[path = "tests/layout.rs"]
mod layout;
