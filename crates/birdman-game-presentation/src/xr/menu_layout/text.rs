use super::*;

/// Actual platform glyph measurements in physical metres.
#[derive(Clone, Copy, Debug)]
pub struct MenuGlyphMetrics {
    /// Horizontal text advance.
    pub advance: f64,
    /// Signed left ink bearing.
    pub left: f64,
    /// Signed right ink bearing.
    pub right: f64,
    /// Signed ink ascent above the baseline.
    pub ascent: f64,
    /// Signed ink descent below the baseline.
    pub descent: f64,
}

/// Wraps exact contextual text measurements, preserving explicit paragraphs.
pub fn measure_menu_text(
    requests: &[MenuTextRequest],
    mut measure: impl FnMut(usize, &str) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
) -> Result<Vec<MenuTextMeasurement>, MenuLayoutFailure> {
    use MenuLayoutFailure::InvalidTextMetrics;
    if requests.len() > 4096 {
        return Err(InvalidTextMetrics);
    }
    let mut measurements = Vec::with_capacity(requests.len());
    for (index, request) in requests.iter().enumerate() {
        if request.value.len() > 65536
            || !request.width_meters.is_finite()
            || request.width_meters <= 0.0
        {
            return Err(InvalidTextMetrics);
        }
        let mut observe = |value: &str, paragraph_index| {
            let metrics = measure(index, value)?;
            if ![
                metrics.advance,
                metrics.left,
                metrics.right,
                metrics.ascent,
                metrics.descent,
            ]
            .into_iter()
            .all(f64::is_finite)
                || metrics.advance < 0.0
                || metrics.left + metrics.right < 0.0
                || metrics.ascent + metrics.descent < 0.0
            {
                return Err(InvalidTextMetrics);
            }
            Ok(MenuMeasuredLine {
                paragraph_index,
                value: value.into(),
                advance_meters: metrics.advance,
                left_meters: metrics.left,
                right_meters: metrics.right,
                ascent_meters: metrics.ascent,
                descent_meters: metrics.descent,
            })
        };
        let fits = |line: &MenuMeasuredLine| {
            line.advance_meters
                .max(line.left_meters.max(0.0) + line.right_meters)
                <= request.width_meters + 1.0e-10
        };
        let mut lines = Vec::new();
        for (paragraph_index, paragraph) in request.value.split('\n').enumerate() {
            let mut current = observe("", paragraph_index)?;
            if !fits(&current) {
                return Err(InvalidTextMetrics);
            }
            for character in paragraph.chars() {
                let mut value = current.value.clone();
                value.push(character);
                let candidate = observe(&value, paragraph_index)?;
                if fits(&candidate) {
                    current = candidate;
                } else {
                    if current.value.is_empty() {
                        return Err(InvalidTextMetrics);
                    }
                    lines.push(current);
                    current = observe(&character.to_string(), paragraph_index)?;
                    if !fits(&current) {
                        return Err(InvalidTextMetrics);
                    }
                }
                if lines.len() >= 4096 {
                    return Err(InvalidTextMetrics);
                }
            }
            lines.push(current);
            if lines.len() > 4096 {
                return Err(InvalidTextMetrics);
            }
        }
        measurements.push(MenuTextMeasurement {
            identity: request.identity.clone(),
            lines,
        });
    }
    Ok(measurements)
}
