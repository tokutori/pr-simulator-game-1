use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, AirfoilFace};
use birdman_game_core::HybridSurfaceRole;
use eframe::egui::{self, Grid};
use egui_plot::{Legend, Line, Plot};

use crate::{app::LoadedAircraft, ui::row};

pub fn show(ui: &mut egui::Ui, loaded: &LoadedAircraft) {
    ui.heading("定常滑空トリム");
    Grid::new("trim").striped(true).show(ui, |ui| {
        let trim = loaded.trim;
        row(ui, "対気速度 (m/s)", trim.conditions().airspeed_mps());
        row(
            ui,
            "迎角 (rad / °)",
            format!(
                "{:.7} / {:.4}",
                trim.alpha_rad(),
                trim.alpha_rad().to_degrees()
            ),
        );
        row(ui, "飛行経路角 (rad)", trim.gamma_rad());
        row(ui, "ピッチ姿勢角 (rad)", trim.theta_rad());
        row(ui, "パイロット位置 (m)", trim.pilot_position_m());
    });
    if let Err(error) = polar(ui, &loaded.definition) {
        ui.colored_label(ui.visuals().error_fg_color, format!("{error:?}"));
    }
}

fn polar(
    ui: &mut egui::Ui,
    definition: &AircraftDefinition,
) -> Result<(), AircraftDefinitionError> {
    let polar = definition.polar()?;
    let source = &definition.document().polar;
    let reference = polar.reference();
    ui.heading("全機静特性 polar");
    Grid::new("reference").striped(true).show(ui, |ui| {
        row(ui, "解析method", format!("{:?}", source.method));
        row(ui, "モーメント軸", format!("{:?}", polar.moment_axes()));
        row(
            ui,
            "モーメント基準点 FRD (m)",
            format!("{:?}", polar.moment_point_from_datum().components()),
        );
        row(ui, "参照面積 S (m²)", reference.area_square_meters());
        row(ui, "参照スパン b (m)", reference.span_meters());
        row(ui, "参照翼弦 c (m)", reference.chord_meters());
        row(
            ui,
            "表の迎角範囲 (rad)",
            format!("{:?}", polar.alpha_interval_rad()),
        );
    });
    let [minimum, maximum] = polar.alpha_interval_rad();
    let mut lift = Vec::new();
    let mut drag = Vec::new();
    let mut ratio = Vec::new();
    // Sample the game's interpolation; plotting only converts units and CL/CD.
    for index in 0..=120 {
        let alpha = if index == 120 {
            maximum
        } else {
            minimum + (maximum - minimum) * f64::from(index) / 120.0
        };
        let coefficients = polar
            .coefficients_at(alpha)
            .map_err(AircraftDefinitionError::Aerodynamics)?;
        let cd = coefficients
            .drag()
            .map_err(AircraftDefinitionError::Aerodynamics)?;
        lift.push([alpha.to_degrees(), coefficients.lift()]);
        drag.push([alpha.to_degrees(), cd]);
        if cd > 0.0 && (coefficients.lift() / cd).is_finite() {
            ratio.push([alpha.to_degrees(), coefficients.lift() / cd]);
        }
    }
    for (name, y_axis, points) in [
        ("揚力係数", "CL", lift),
        ("抗力係数", "CD", drag),
        ("揚抗比", "CL/CD", ratio),
    ] {
        Plot::new(name)
            .height(220.0)
            .x_axis_label("迎角 (°)")
            .y_axis_label(y_axis)
            .show(ui, |plot| plot.line(Line::new(name, points)));
    }
    ui.label("揚抗比は CD > 0 の点を表示する。");
    egui::CollapsingHeader::new("polar原表（角度rad）")
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("polar_table").striped(true).show(ui, |ui| {
                for heading in ["迎角", "CL", "CDi", "CDv", "CY", "Cl", "Cm", "Cn"] {
                    ui.strong(heading);
                }
                ui.end_row();
                for row in polar.rows() {
                    let c = row.coefficients();
                    for value in [
                        row.alpha_rad(),
                        c.lift(),
                        c.induced_drag(),
                        c.profile_drag(),
                        c.side_force(),
                        c.roll_moment(),
                        c.pitch_moment(),
                        c.yaw_moment(),
                    ] {
                        ui.monospace(format!("{value:.6}"));
                    }
                    ui.end_row();
                }
            });
        });
    Ok(())
}

pub fn planform(ui: &mut egui::Ui, definition: &AircraftDefinition) {
    ui.heading("検証済み外形の平面投影");
    ui.label("横軸は翼幅方向、縦軸は機首方向。垂直尾翼は横から見た高さ方向へ投影する。単位 m。");
    for source in &definition.document().surfaces {
        let shape = definition.shape().surface(source.role.core_role());
        let span_axis = if shape.role() == HybridSurfaceRole::VerticalTail {
            2
        } else {
            1
        };
        let stations: Vec<_> = shape.span_stations_m().collect();
        let outline = stations
            .iter()
            .copied()
            .map(|span| (span, 0.0))
            .chain(stations.iter().rev().copied().map(|span| (span, 1.0)))
            .chain(stations.first().copied().map(|span| (span, 0.0)))
            .map(|(span, chord)| {
                shape.point(span, chord, AirfoilFace::Upper).map(|point| {
                    let frd = point.components();
                    [frd[span_axis], frd[0]]
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let quarter_chord = stations
            .iter()
            .map(|&span| {
                shape.section(span).map(|section| {
                    [
                        section.quarter_chord_frd_m[span_axis],
                        section.quarter_chord_frd_m[0],
                    ]
                })
            })
            .collect::<Result<Vec<_>, _>>();
        match (outline, quarter_chord) {
            (Ok(outline), Ok(quarter_chord)) => {
                let title = source.role.to_string();
                ui.heading(&title);
                Plot::new(("planform", &title))
                    .height(310.0)
                    .data_aspect(1.0)
                    .legend(Legend::default())
                    .x_axis_label(if span_axis == 2 {
                        "下向き D (m)"
                    } else {
                        "右向き R (m)"
                    })
                    .y_axis_label("前向き F (m)")
                    .show(ui, |plot| {
                        plot.line(Line::new("外形", outline));
                        plot.line(Line::new("四分の一翼弦線", quarter_chord));
                    });
            }
            (Err(error), _) | (_, Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("{error:?}"));
            }
        }
    }
}
