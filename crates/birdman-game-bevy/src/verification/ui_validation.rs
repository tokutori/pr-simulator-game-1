use super::Verification;
use crate::app::ui::{MenuButton, UiPanel, UiText};
use bevy::{
    ecs as bevy_ecs,
    ecs::system::SystemParam,
    prelude::*,
    ui::{OverrideClip, clip_check_recursive},
};

pub(super) fn verify_text_layout(bytes: &[u8]) -> Result<(), String> {
    use parley::{
        FontContext, LayoutContext, StyleProperty, WordBreak,
        fontique::{Blob, GenericFamily},
    };

    let mut fonts = FontContext::new();
    let registered = fonts
        .collection
        .register_fonts(Blob::new(std::sync::Arc::new(bytes.to_vec())), None);
    if registered.is_empty() {
        return Err("CJK regression could not register the selected native font".into());
    }
    fonts.collection.set_generic_families(
        GenericFamily::SansSerif,
        registered.iter().map(|(family, _)| *family),
    );
    let mut context = LayoutContext::<()>::new();
    for (text, word_break, width) in [
        ("こんにちは世界", WordBreak::KeepAll, None),
        ("飛行Flight確認", WordBreak::Normal, Some(120.0)),
        ("飛行Flight確認", WordBreak::BreakAll, Some(120.0)),
        ("飛行Flight確認", WordBreak::KeepAll, Some(120.0)),
    ] {
        let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(20.0));
        builder.push_default(StyleProperty::WordBreak(word_break));
        let mut layout = builder.build(text);
        layout.break_all_lines(width);
        if !layout.width().is_finite()
            || !layout.height().is_finite()
            || layout.width() <= 0.0
            || layout.height() <= 0.0
        {
            return Err(format!(
                "CJK layout has invalid dimensions: {text} / {word_break:?}"
            ));
        }
        let mut ranges = Vec::new();
        let mut boundaries = Vec::new();
        for line in layout.lines() {
            for run in line.runs() {
                for cluster in run.clusters() {
                    let range = cluster.text_range();
                    if cluster.is_word_boundary() {
                        boundaries.push(range.start);
                    }
                    ranges.push(range);
                }
            }
        }
        ranges.sort_by_key(|range| range.start);
        let mut covered = 0;
        for range in ranges {
            if range.start != covered
                || range.end <= covered
                || range.end > text.len()
                || !text.is_char_boundary(range.start)
                || !text.is_char_boundary(range.end)
            {
                return Err(format!(
                    "CJK layout has inconsistent original text coverage: {text} / {word_break:?}"
                ));
            }
            covered = range.end;
        }
        if covered != text.len() {
            return Err(format!(
                "CJK layout lost original text: {text} / {word_break:?}"
            ));
        }
        if text == "こんにちは世界" {
            boundaries.push(text.len());
            if boundaries != [0, 15, 21] {
                return Err(format!(
                    "CJK word boundaries differ: expected [0, 15, 21], observed {boundaries:?}"
                ));
            }
        }
    }
    println!(
        "Selected native font: CJK word boundaries and mixed-script layout regression passed."
    );
    Ok(())
}

type VerificationUiLabel = (
    Entity,
    &'static Text,
    &'static ComputedNode,
    &'static bevy::text::TextLayoutInfo,
    &'static UiGlobalTransform,
    Option<&'static UiText>,
);

#[derive(SystemParam)]
pub(super) struct VerificationUiHierarchy<'w, 's> {
    pub(super) clipping: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static Node,
        ),
    >,
    pub(super) parents: Query<'w, 's, &'static ChildOf, Without<OverrideClip>>,
    pub(super) names: Query<'w, 's, &'static Name>,
    pub(super) windows: Query<'w, 's, &'static Window>,
}

pub(super) fn verify_ui_layout(
    mut verification: ResMut<Verification>,
    buttons: Query<(Entity, &ComputedNode, &UiGlobalTransform, &Children), With<MenuButton>>,
    labels: Query<VerificationUiLabel>,
    panels: Query<(
        Entity,
        &UiPanel,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &ScrollPosition,
    )>,
    hierarchy: VerificationUiHierarchy,
    mut inputs: ResMut<crate::app::native_runtime::NativeInputs>,
) {
    let identity = verification.capture_identity();
    if verification.failure.is_some() || !verification.capture.accepts(identity, identity) {
        return;
    }
    let result = (|| {
        let window = hierarchy
            .windows
            .iter()
            .next()
            .ok_or("UI viewport is unavailable")?;
        let viewport = Rect::from_corners(Vec2::ZERO, window.physical_size().as_vec2());
        let mut visible_buttons = 0;
        for (entity, button, transform, children) in &buttons {
            if button.is_empty() {
                continue;
            }
            let bounds = Rect::from_center_size(transform.translation, button.size());
            let fully_visible = contains_layout(viewport, bounds)
                && within_ancestor_clip(entity, bounds, &hierarchy);
            if fully_visible {
                visible_buttons += 1;
            } else if !scroll_reachable(entity, bounds, &panels, &hierarchy) {
                return Err(format!(
                    "Button is neither visible nor scroll-reachable: {bounds:?}"
                ));
            }
            let mut visible_labels = 0;
            for child in children.iter() {
                let Ok((_, text, node, layout, transform, role)) = labels.get(child) else {
                    continue;
                };
                if text.0.trim().is_empty()
                    || node.is_empty()
                    || layout.glyphs.is_empty()
                    || !layout.size.is_finite()
                    || layout.size.min_element() <= 0.0
                {
                    return Err(format!(
                        "Button label collapsed or has no glyphs: {:?}",
                        text.0
                    ));
                }
                let label_bounds = Rect::from_center_size(transform.translation, node.size());
                if !contains_layout(bounds, label_bounds)
                    || layout.size.x > node.size().x + 2.0
                    || layout.size.y > node.size().y + 2.0
                {
                    return Err(format!(
                        "Button label exceeds its allocated bounds: {:?}",
                        text.0
                    ));
                }
                if matches!(role, Some(UiText::WaterQualityChoice))
                    && !fully_visible
                    && !scroll_reachable(entity, bounds, &panels, &hierarchy)
                {
                    return Err(format!(
                        "Visible quality control is clipped by an ancestor: {:?}",
                        text.0
                    ));
                }
                visible_labels += 1;
            }
            if visible_labels == 0 {
                return Err("Visible button has no text label".into());
            }
        }
        if visible_buttons == 0 {
            return Err("Captured scene has no visible navigation buttons".into());
        }
        let mut content = None;
        for (_, panel, _, node, transform, _) in &panels {
            if node.is_empty() {
                continue;
            }
            let bounds = Rect::from_center_size(transform.translation, node.size());
            match panel {
                UiPanel::SessionContent => content = Some(bounds),
                UiPanel::Navigation => {}
            }
        }
        let content = content.ok_or("Captured scene has no session content panel")?;
        if !contains_layout(viewport, content) {
            return Err("Shared menu scroll viewport exceeds the native viewport".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!("Native UI verification: {error}; capture={identity:?}, phase={phase:?}");
        verification.complete(Err(error.clone()));
        verification.failure = Some(error);
        inputs
            .0
            .push(birdman_game_runtime::ApplicationInput::Dispose);
    }
}

fn scroll_reachable(
    entity: Entity,
    bounds: Rect,
    panels: &Query<(
        Entity,
        &UiPanel,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &ScrollPosition,
    )>,
    hierarchy: &VerificationUiHierarchy,
) -> bool {
    let mut current = entity;
    while let Ok(parent) = hierarchy.parents.get(current) {
        current = parent.parent();
        if let Ok((_, UiPanel::SessionContent, style, node, transform, scroll)) =
            panels.get(current)
        {
            if style.overflow.y != OverflowAxis::Scroll {
                return false;
            }
            let viewport = Rect::from_center_size(transform.translation, node.size());
            let scroll_px = scroll.y / node.inverse_scale_factor;
            let virtual_bounds = Rect::from_corners(
                Vec2::new(viewport.min.x, viewport.min.y - scroll_px),
                Vec2::new(
                    viewport.max.x,
                    viewport.min.y - scroll_px + node.content_size.y.max(node.size.y),
                ),
            );
            return contains_layout(virtual_bounds, bounds);
        }
    }
    false
}

pub(super) fn contains_layout(outer: Rect, inner: Rect) -> bool {
    inner.min.x >= outer.min.x - 2.0
        && inner.min.y >= outer.min.y - 2.0
        && inner.max.x <= outer.max.x + 2.0
        && inner.max.y <= outer.max.y + 2.0
}

pub(super) fn within_ancestor_clip(
    entity: Entity,
    bounds: Rect,
    hierarchy: &VerificationUiHierarchy,
) -> bool {
    let inset = Vec2::splat(2.0).min(bounds.size() * 0.5);
    let minimum = bounds.min + inset;
    let maximum = bounds.max - inset;
    [
        minimum,
        Vec2::new(maximum.x, minimum.y),
        maximum,
        Vec2::new(minimum.x, maximum.y),
    ]
    .into_iter()
    .all(|point| clip_check_recursive(point, entity, &hierarchy.clipping, &hierarchy.parents))
}
