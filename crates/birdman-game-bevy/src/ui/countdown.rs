use super::super::NativeFont;
use super::{color, retained_node, set_text, spawn_text};
use bevy::{picking::Pickable, prelude::*};
use birdman_game_presentation::game_ui::{GameUiCountdown, GameUiTheme};

pub(super) struct RetainedCountdown {
    pub(super) shell: Entity,
    texts: Option<(Entity, Entity)>,
    previous: Option<GameUiCountdown>,
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
    RetainedCountdown {
        shell,
        texts: None,
        previous: None,
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
        retained_node(world, retained.shell)
            .map_unchanged(|node| &mut node.display)
            .set_if_neq(Display::None);
        retained.previous = None;
        return Ok(());
    };
    if retained.previous.as_ref() == Some(countdown) {
        return Ok(());
    }
    retained.previous = Some(countdown.clone());
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
                retained.shell,
                "",
                &font,
                countdown.font_size_px as f32,
                ink,
            );
            world.entity_mut(digits).insert(Pickable::IGNORE);
            let unit = spawn_text(
                world,
                retained.shell,
                "",
                &font,
                (countdown.font_size_px * 0.35) as f32,
                ink,
            );
            world.entity_mut(unit).insert(Pickable::IGNORE);
            retained.texts = Some((digits, unit));
            (digits, unit)
        }
    };
    set_text(world, digits, countdown.digits.clone());
    set_text(world, unit, countdown.unit.clone());
    for (entity, size) in [
        (digits, countdown.font_size_px),
        (unit, countdown.font_size_px * 0.35),
    ] {
        if let Some(mut text_font) = world.get_mut::<TextFont>(entity) {
            text_font.font_size = FontSize::Px(size as f32);
        }
        if let Some(mut text_color) = world.get_mut::<TextColor>(entity) {
            *text_color = TextColor(ink);
        }
    }
    Ok(())
}
