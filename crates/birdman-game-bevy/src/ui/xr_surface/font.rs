use super::*;

pub(crate) fn surface(world: &World) -> Option<ApplicationSurfaceObservation> {
    let handle = &world.get_resource::<NativeFont>()?.0;
    let font = world.get_resource::<Assets<Font>>()?.get(handle)?;
    if font.alias.is_empty() {
        return None;
    }
    Some(ApplicationSurfaceObservation {
        font: MenuFont {
            family: font.alias.to_string(),
            weight: 400,
            style: MenuFontStyle::Normal,
            generation: 0,
        },
        locale: "und".to_owned(),
    })
}

pub(super) fn text_font(
    world: &World,
    font: &MenuFont,
    locale: &str,
    size: f64,
) -> Result<TextFont, MenuLayoutFailure> {
    let observed = surface(world).ok_or(MenuLayoutFailure::MeasurementUnavailable)?;
    if font.family != observed.font.family
        || font.generation != observed.font.generation
        || locale != observed.locale
    {
        return Err(MenuLayoutFailure::MeasurementUnavailable);
    }
    if !(size as f32).is_finite() || size <= 0.0 || !(1..=1000).contains(&font.weight) {
        return Err(MenuLayoutFailure::InvalidTextMetrics);
    }
    Ok(TextFont {
        font: world.resource::<NativeFont>().0.clone().into(),
        font_size: FontSize::Px(size as f32),
        weight: FontWeight(font.weight),
        style: match font.style {
            MenuFontStyle::Normal => FontStyle::Normal,
            MenuFontStyle::Italic => FontStyle::Italic,
        },
        ..default()
    })
}

pub(crate) fn with_metrics<T>(
    world: &mut World,
    run: impl FnOnce(
        &mut dyn FnMut(&ApplicationTextProbe) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> T,
) -> T {
    run(&mut |probe| measure(world, probe))
}

fn measure(
    world: &mut World,
    probe: &ApplicationTextProbe,
) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure> {
    let font = text_font(world, &probe.font, &probe.locale, probe.font_size_px)?;
    let mut state = SystemState::<(
        Res<Assets<Font>>,
        ResMut<TextPipeline>,
        ResMut<FontCx>,
        ResMut<LayoutCx>,
        ResMut<ScaleCx>,
        ResMut<FontAtlasSet>,
        ResMut<Assets<Image>>,
    )>::new(world);
    let (fonts, mut pipeline, mut font_cx, mut layout_cx, mut scale_cx, mut atlases, mut images) =
        state
            .get_mut(world)
            .map_err(|_| MenuLayoutFailure::MeasurementUnavailable)?;
    let mut block = ComputedTextBlock::default();
    let mut layout = TextLayoutInfo::default();
    pipeline
        .update_buffer(
            &fonts,
            std::iter::once((
                Entity::PLACEHOLDER,
                0,
                probe.text.as_str(),
                &font,
                Color::WHITE,
                LineHeight::default(),
                LetterSpacing::default(),
            )),
            LineBreak::NoWrap,
            Justify::Left,
            TextBounds::UNBOUNDED,
            1.0,
            &mut block,
            &mut font_cx,
            &mut layout_cx,
            Vec2::ZERO,
            16.0,
        )
        .map_err(|_| MenuLayoutFailure::MeasurementUnavailable)?;
    pipeline
        .update_text_layout_info(
            &mut layout,
            &mut atlases,
            &mut images,
            &mut block,
            &mut scale_cx,
            TextBounds::UNBOUNDED,
            Justify::Left,
            FontHinting::Enabled,
        )
        .map_err(|_| MenuLayoutFailure::MeasurementUnavailable)?;
    let advance = block.buffer().full_width();
    let baseline = block
        .buffer()
        .lines()
        .next()
        .map_or(0.0, |line| line.metrics().baseline);
    let mut ink: Option<Rect> = None;
    for glyph in &layout.glyphs {
        let image = images
            .get(glyph.atlas_info.texture)
            .ok_or(MenuLayoutFailure::MeasurementUnavailable)?;
        let data = image
            .data
            .as_ref()
            .ok_or(MenuLayoutFailure::MeasurementUnavailable)?;
        let atlas = glyph.atlas_info.rect;
        let origin = glyph.position - atlas.size() * 0.5;
        let stride = image.width() as usize;
        for y in atlas.min.y as u32..atlas.max.y as u32 {
            for x in atlas.min.x as u32..atlas.max.x as u32 {
                let alpha = data
                    .get(((y as usize * stride) + x as usize) * 4 + 3)
                    .ok_or(MenuLayoutFailure::MeasurementUnavailable)?;
                if *alpha == 0 {
                    continue;
                }
                let point = origin + Vec2::new(x as f32 - atlas.min.x, y as f32 - atlas.min.y);
                let pixel = Rect::from_corners(point, point + Vec2::ONE);
                ink = Some(ink.map_or(pixel, |old| old.union(pixel)));
            }
        }
    }
    let (left, right, ascent, descent) = ink.map_or((0.0, 0.0, 0.0, 0.0), |ink| {
        (
            -ink.min.x,
            ink.max.x,
            baseline - ink.min.y,
            ink.max.y - baseline,
        )
    });
    Ok(ApplicationGlyphMetrics {
        advance_px: f64::from(advance),
        left_px: f64::from(left),
        right_px: f64::from(right),
        ascent_px: f64::from(ascent),
        descent_px: f64::from(descent),
    })
}
