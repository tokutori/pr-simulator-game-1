use super::*;

pub(super) fn project(
    source: MenuViewportSource,
    rectangles: Vec<MenuRect>,
) -> Result<MenuViewportProjection, MenuLayoutFailure> {
    use MenuLayoutFailure::InvalidLayout;
    let size = source.surface_size;
    let padding = source.padding_meters;
    let gap = source.gap_meters;
    if ![
        size.width,
        size.height,
        padding,
        gap,
        source.content_height_meters,
        source.footer_height_meters,
        source.progress,
    ]
    .into_iter()
    .all(f64::is_finite)
        || size.width <= 0.0
        || size.height <= 0.0
        || padding < 0.0
        || gap < 0.0
        || source.content_height_meters < 0.0
        || source.footer_height_meters < 0.0
        || !(0.0..=1.0).contains(&source.progress)
        || rectangles.len() > 16384
        || rectangles.iter().any(|rectangle| !valid_rect(*rectangle))
    {
        return Err(InvalidLayout);
    }
    let fits = source.content_height_meters <= size.height - padding * 2.0;
    let footer_height = if fits {
        0.0
    } else {
        source.footer_height_meters
    };
    let footer_top = size.height - padding - footer_height;
    let clip = MenuRect {
        x: padding,
        y: padding,
        width: size.width - padding * 2.0,
        height: footer_top - if fits { 0.0 } else { gap } - padding,
    };
    let page_width = (clip.width - gap) / 2.0;
    if !finite_rect(clip) || clip.width <= 0.0 || clip.height <= 0.0 || page_width <= 0.0 {
        return Err(InvalidLayout);
    }
    let maximum = (source.content_height_meters - clip.height).max(0.0);
    let offset = source.progress * maximum;
    let translate = |point: MenuPoint| MenuPoint {
        x: point.x + clip.x,
        y: point.y - offset + clip.y,
    };
    let projected_rectangles = rectangles
        .into_iter()
        .map(|rectangle| {
            let origin = translate(MenuPoint {
                x: rectangle.x,
                y: rectangle.y,
            });
            let full = MenuRect {
                x: origin.x,
                y: origin.y,
                ..rectangle
            };
            let left = full.x.max(clip.x);
            let top = full.y.max(clip.y);
            let right = (full.x + full.width).min(clip.x + clip.width);
            let bottom = (full.y + full.height).min(clip.y + clip.height);
            MenuViewportRect {
                full,
                visible: (right > left && bottom > top).then_some(MenuRect {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                }),
            }
        })
        .collect::<Vec<_>>();
    let projection = MenuViewportProjection {
        content_clip: clip,
        previous_bounds: MenuRect {
            x: clip.x,
            y: footer_top,
            width: page_width,
            height: footer_height,
        },
        next_bounds: MenuRect {
            x: clip.x + page_width + gap,
            y: footer_top,
            width: page_width,
            height: footer_height,
        },
        progress: source.progress,
        offset_meters: offset,
        maximum_offset_meters: maximum,
        page_progress: if maximum == 0.0 {
            0.0
        } else {
            (clip.height / maximum).min(1.0)
        },
        rectangles: projected_rectangles,
    };
    if projection
        .rectangles
        .iter()
        .any(|rectangle| !valid_rect(rectangle.full))
    {
        return Err(InvalidLayout);
    }
    Ok(projection)
}

fn valid_rect(rectangle: MenuRect) -> bool {
    [
        rectangle.x,
        rectangle.y,
        rectangle.width,
        rectangle.height,
        rectangle.x + rectangle.width,
        rectangle.y + rectangle.height,
    ]
    .into_iter()
    .all(f64::is_finite)
        && rectangle.width >= 0.0
        && rectangle.height >= 0.0
}
