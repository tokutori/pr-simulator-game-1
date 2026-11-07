use super::super::NativeFont;
use super::{color, retained_node, set_text, spawn_text};
use bevy::{picking::Pickable, prelude::*};
use birdman_game_presentation::game_ui::{GameUiCountdown, GameUiTheme};

pub(super) struct RetainedCountdown {
    pub(super) shell: Entity,
    row: Entity,
    texts: Option<(Entity, Entity)>,
    painted: Option<(GameUiCountdown, GameUiTheme)>,
}

pub(super) fn setup_countdown(commands: &mut Commands) -> RetainedCountdown {
    let shell = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                align_items: AlignItems::Baseline,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(shell).add_child(row);
    RetainedCountdown {
        shell,
        row,
        texts: None,
        painted: None,
    }
}

/// Game-side countdown display: a full-screen overlay with no panel, matching
/// the shared derivation in the retained view and per-second frame values.
pub(super) fn project_countdown(
    world: &mut World,
    retained: &mut RetainedCountdown,
    countdown: Option<&GameUiCountdown>,
    theme: &GameUiTheme,
) -> Result<(), String> {
    let Some(countdown) = countdown else {
        retained.painted = None;
        retained_node(world, retained.shell)
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(Display::None);
        return Ok(());
    };
    if retained
        .painted
        .as_ref()
        .is_some_and(|(previous, previous_theme)| previous == countdown && previous_theme == theme)
    {
        return Ok(());
    }
    retained.painted = Some((countdown.clone(), theme.clone()));
    let font = world.resource::<NativeFont>().0.clone();
    let ink = color(&theme.text, 1.0)?;
    retained_node(world, retained.shell)
        .map_unchanged(|node| &mut node.display)
        .set_if_neq(Display::Flex);
    let (digits, unit) = match retained.texts {
        Some(texts) => texts,
        None => {
            let digits = spawn_text(
                world,
                retained.row,
                "",
                &font,
                countdown.digit_text.font_size_px as f32,
                ink,
            );
            world.entity_mut(digits).insert(Pickable::IGNORE);
            let unit = spawn_text(
                world,
                retained.row,
                "",
                &font,
                countdown.unit_text.font_size_px as f32,
                ink,
            );
            world.entity_mut(unit).insert(Pickable::IGNORE);
            retained.texts = Some((digits, unit));
            (digits, unit)
        }
    };
    set_text(world, digits, countdown.digits.clone());
    set_text(world, unit, countdown.unit.clone());
    retained_node(world, retained.row).column_gap = px(countdown.gap_px as f32);
    for (entity, style) in [(digits, countdown.digit_text), (unit, countdown.unit_text)] {
        if let Some(mut text_font) = world.get_mut::<TextFont>(entity) {
            text_font.font_size = FontSize::Px(style.font_size_px as f32);
            text_font.weight = bevy::text::FontWeight(style.font_weight);
        }
        world
            .entity_mut(entity)
            .insert(bevy::text::LineHeight::Px(style.line_height_px as f32));
        if let Some(mut text_color) = world.get_mut::<TextColor>(entity) {
            *text_color = TextColor(ink);
        }
    }
    super::projection::project_text_shadow(world, retained.shell, theme, true)
}
