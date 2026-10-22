use super::*;
use crate::camera::{Pose, Vector3, compose_pose};
use crate::xr::{
    XrFitReason, XrInkFit, XrInkSegment, XrMenuViewFit, XrViewerFrame, fit_xr_menu_view,
    fit_xr_text_ink,
};

pub(super) fn prepare(
    mut source: MenuLayoutSource,
    viewer: &XrViewerFrame,
    menu_from_panel: Pose,
    observation: Option<&MenuPreparationObservation>,
) -> MenuLayoutReply {
    let result = select(&mut source, viewer, menu_from_panel, observation);
    result.unwrap_or_else(|reason| MenuLayoutReply::Unavailable { reason })
}

fn select(
    source: &mut MenuLayoutSource,
    viewer: &XrViewerFrame,
    menu_from_panel: Pose,
    observation: Option<&MenuPreparationObservation>,
) -> Result<MenuLayoutReply, MenuLayoutFailure> {
    let attempt = observation.map_or(0, |observed| observed.attempt);
    if attempt >= 12 {
        return Err(MenuLayoutFailure::StaleTextMetrics);
    }
    let fit = |size: MenuSize| {
        fit_xr_menu_view(
            viewer,
            [size.width, size.height],
            menu_from_panel,
            2.4,
            0.035,
        )
        .map_err(|_| MenuLayoutFailure::InvalidLayout)
    };
    let mut fitted = fit(source.surface_size)?;
    for _ in 0..12 {
        match fitted {
            XrMenuViewFit::Unavailable { reason } => return Err(fit_reason(reason)),
            XrMenuViewFit::Ready {
                distance_meters, ..
            } if distance_meters > 2.6 && source.surface_size.width > 0.35 => {
                source.surface_size.width = (source.surface_size.width * 0.85).max(0.35);
                fitted = fit(source.surface_size)?;
            }
            _ => break,
        }
    }
    let head_from_menu = match fitted {
        XrMenuViewFit::Ready { head_from_menu, .. } => head_from_menu,
        XrMenuViewFit::Unavailable { reason } => return Err(fit_reason(reason)),
    };
    let head_from_surface = compose_pose(head_from_menu, menu_from_panel)
        .map_err(|_| MenuLayoutFailure::InvalidViewGeometry)?;
    if source.surface_size.height < 0.6 {
        source.title.clear();
        source.caption.clear();
    }
    for _ in 0..attempt {
        source.font_meters *= 1.15;
    }
    let planned = plan(source.clone())?;
    let Some(observed) = observation else {
        return Ok(MenuLayoutReply::Measure {
            plan: planned,
            attempt,
        });
    };
    let geometry = arrange(&planned, &observed.measurements)?;
    match readability(
        viewer,
        planned.source.surface_size,
        &observed.measurements,
        head_from_surface,
    ) {
        XrInkFit::Ready { .. } => Ok(MenuLayoutReply::Ready { geometry }),
        XrInkFit::Unavailable { reason } => Err(fit_reason(reason)),
        XrInkFit::Empty | XrInkFit::TooSmall if attempt + 1 < 12 => {
            source.font_meters *= 1.15;
            Ok(MenuLayoutReply::Measure {
                plan: plan(source.clone())?,
                attempt: attempt + 1,
            })
        }
        _ => Err(MenuLayoutFailure::InsufficientInkAngle),
    }
}

fn readability(
    viewer: &XrViewerFrame,
    size: MenuSize,
    measurements: &[MenuTextMeasurement],
    head_from_surface: Pose,
) -> XrInkFit {
    let mut segments = Vec::new();
    for line in measurements
        .iter()
        .flat_map(|measurement| &measurement.lines)
        .filter(|line| !line.value.trim().is_empty())
    {
        let height = line.ascent_meters + line.descent_meters;
        if height <= 0.0 || height > size.height {
            return XrInkFit::TooSmall;
        }
        for horizontal in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                segments.push(XrInkSegment {
                    start: Vector3 {
                        x: horizontal * size.width / 2.0,
                        y: vertical * size.height / 2.0,
                        z: 0.0,
                    },
                    end: Vector3 {
                        x: horizontal * size.width / 2.0,
                        y: vertical * (size.height / 2.0 - height),
                        z: 0.0,
                    },
                });
            }
        }
    }
    fit_xr_text_ink(viewer, &segments, head_from_surface, 0.35)
}

fn fit_reason(reason: XrFitReason) -> MenuLayoutFailure {
    match reason {
        XrFitReason::NotStereo => MenuLayoutFailure::NotStereo,
        XrFitReason::ViewerUnavailable => MenuLayoutFailure::ViewerUnavailable,
        XrFitReason::UnsupportedViewConfiguration => {
            MenuLayoutFailure::UnsupportedViewConfiguration
        }
        XrFitReason::InvalidViewGeometry => MenuLayoutFailure::InvalidViewGeometry,
        XrFitReason::InsufficientViewArea => MenuLayoutFailure::InsufficientViewArea,
    }
}
