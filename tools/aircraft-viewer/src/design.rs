use birdman_game_content::AircraftDefinition;
use birdman_game_core::{AircraftModel, HybridSurfaceRole, InertiaTensor};
use eframe::egui::{self, Grid};

use crate::ui::row;

pub fn show(ui: &mut egui::Ui, definition: &AircraftDefinition) {
    let document = definition.document();
    let aircraft = definition.aircraft();
    ui.heading("質量・パイロット");
    Grid::new("mass").striped(true).show(ui, |ui| {
        row(ui, "機体質量 (kg)", aircraft.airframe_mass_kg());
        row(
            ui,
            "機体重心 FRD / datum O (m)",
            format!("{:?}", aircraft.airframe_center_of_mass_frd_m()),
        );
        inertia_rows(
            ui,
            "機体慣性 / datum O (kg m²)",
            aircraft.airframe_inertia_about_datum(),
        );
        match AircraftModel::airframe_inertia_at_center_of_mass(
            aircraft.airframe_mass_kg(),
            aircraft.airframe_inertia_about_datum(),
            aircraft.airframe_center_of_mass_frd_m(),
        ) {
            Ok(inertia) => inertia_rows(ui, "機体慣性 / 機体CG (kg m²)", inertia),
            Err(error) => row(ui, "機体CG慣性の導出", format!("{error:?}")),
        }
        row(ui, "パイロット質量 (kg)", document.pilot.mass_kg);
        row(ui, "下向きオフセット (m)", document.pilot.vertical_offset_m);
        row(
            ui,
            "前後移動範囲 (m)",
            format!("{:?}", document.pilot.position_bounds_m),
        );
        row(ui, "最大移動速度 (m/s)", document.pilot.maximum_speed_mps);
        row(
            ui,
            "最大移動加速度 (m/s²)",
            document.pilot.maximum_acceleration_mps2,
        );
    });
    ui.heading("翼面と導出諸元");
    match definition.surfaces() {
        Ok(surfaces) => {
            Grid::new("derived").striped(true).show(ui, |ui| {
                for title in [
                    "役割",
                    "投影スパン (m)",
                    "投影面積 (m²)",
                    "表面積 (m²)",
                    "投影MAC (m)",
                    "AR",
                ] {
                    ui.strong(title);
                }
                ui.end_row();
                for surface in surfaces {
                    let geometry = surface.geometry();
                    ui.label(format!("{:?}", geometry.role()));
                    for value in [
                        geometry.projected_span_m(),
                        geometry.projected_area_m2(),
                        geometry.surface_area_m2(),
                        geometry.projected_mac_m(),
                        geometry.aspect_ratio(),
                    ] {
                        ui.monospace(format!("{value:.5}"));
                    }
                    ui.end_row();
                }
            });
        }
        Err(error) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("{error:?}"));
        }
    }
    for surface in &document.surfaces {
        egui::CollapsingHeader::new(format!("{} — 断面・空力", surface.role))
            .default_open(true)
            .show(ui, |ui| {
                Grid::new(("surface", surface.role.to_string()))
                    .striped(true)
                    .show(ui, |ui| {
                        row(ui, "対称配置", surface.mirror_span);
                        row(ui, "strip境界", format!("{:?}", surface.strip_fractions));
                        row(
                            ui,
                            "揚力基準 [係数, rad]",
                            format!("{:?}", surface.lift_anchor),
                        );
                        row(
                            ui,
                            "翼型 [camber, position, thickness]",
                            format!(
                                "[{:.5}, {:.5}, {:.5}]",
                                surface.airfoil.camber_ratio,
                                surface.airfoil.camber_position_ratio,
                                surface.airfoil.thickness_ratio
                            ),
                        );
                        if let Some(tail) = surface.tail_aerodynamics {
                            row(ui, "零揚力抗力係数", tail.zero_lift_drag_coefficient);
                            row(ui, "スパン効率", tail.span_efficiency);
                            row(ui, "ピッチモーメント係数", tail.pitching_moment_coefficient);
                            row(ui, "基準対気速度 (m/s)", tail.reference_airspeed_mps);
                            row(
                                ui,
                                "基準動粘性係数 (m²/s)",
                                tail.reference_kinematic_viscosity_m2ps,
                            );
                        }
                        let actuator = match surface.role.core_role() {
                            HybridSurfaceRole::HorizontalTail => Some(aircraft.tail_actuators()[0]),
                            HybridSurfaceRole::VerticalTail => Some(aircraft.tail_actuators()[1]),
                            HybridSurfaceRole::MainWing => None,
                        };
                        if let Some(actuator) = actuator {
                            let [lower, upper] = actuator.incidence_bounds_rad();
                            row(
                                ui,
                                "機械的な舵角範囲 (rad / °)",
                                format!(
                                    "[{lower:.6}, {upper:.6}] / [{:.3}, {:.3}]",
                                    lower.to_degrees(),
                                    upper.to_degrees()
                                ),
                            );
                            let slew = actuator.maximum_slew_rad_per_second();
                            row(
                                ui,
                                "機械的な最大角速度 (rad/s / °/s)",
                                format!("{slew:.6} / {:.3}", slew.to_degrees()),
                            );
                        }
                    });
                Grid::new(("sections", surface.role.to_string()))
                    .striped(true)
                    .show(ui, |ui| {
                        for title in [
                            "F (m)",
                            "R (m)",
                            "D (m)",
                            "翼弦 (m)",
                            "ねじり (rad)",
                            "ねじり (°)",
                        ] {
                            ui.strong(title);
                        }
                        ui.end_row();
                        for section in &surface.sections {
                            for value in section.quarter_chord_frd_m.into_iter().chain([
                                section.chord_m,
                                section.twist_rad,
                                section.twist_rad.to_degrees(),
                            ]) {
                                ui.monospace(format!("{value:.5}"));
                            }
                            ui.end_row();
                        }
                    });
            });
    }
    ui.heading("構造・トリム入力");
    Grid::new("structure").striped(true).show(ui, |ui| {
        let structure = &document.structure;
        row(
            ui,
            "主梁端点 FRD (m)",
            format!("{:?}", structure.main_beam_endpoints_frd_m),
        );
        row(ui, "主梁半径 (m)", structure.main_beam_radius_m);
        row(
            ui,
            "翼曲げ剛性 EI (N m²)",
            structure.wing_flexural_rigidity_nm2,
        );
        row(
            ui,
            "コックピット丸め指数",
            structure.cockpit_rounding_exponent,
        );
        row(
            ui,
            "コックピット骨組み半径 (m)",
            structure.cockpit_frame_radius_m,
        );
        row(
            ui,
            "視点オフセット FRD (m)",
            format!("{:?}", structure.pilot_eye_offset_frd_m),
        );
        row(ui, "空気密度 (kg/m³)", document.trim.air_density_kg_m3);
        row(ui, "重力 (m/s²)", document.trim.gravity_mps2);
        row(ui, "基準対気速度 (m/s)", document.trim.airspeed_mps);
        row(
            ui,
            "迎角探索範囲 (rad)",
            format!("{:?}", document.trim.alpha_bounds_rad),
        );
    });
    egui::CollapsingHeader::new("構造断面と部材").show(ui, |ui| {
        ui.monospace(format!("{:#?}", document.structure.cockpit_stations));
        ui.monospace(format!("{:#?}", document.structure.members));
    });
}

fn inertia_rows(ui: &mut egui::Ui, label: &str, inertia: InertiaTensor) {
    row(ui, label, "F/R/D軸順");
    for values in inertia.matrix() {
        row(
            ui,
            "",
            format!("{:12.5} {:12.5} {:12.5}", values[0], values[1], values[2]),
        );
    }
}
