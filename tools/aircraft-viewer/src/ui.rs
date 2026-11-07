use birdman_game_content::AircraftDefinitionError;
use birdman_game_format::aircraft::AircraftFormatError;
use eframe::egui::{self, RichText};

use crate::app::{LoadError, Tab, ViewerApp};

pub fn show(context: &egui::Context, app: &mut ViewerApp) {
    egui::TopBottomPanel::top("file").show(context, |ui| {
        ui.horizontal(|ui| {
            if ui.button("機体JSONを開く…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("機体JSON", &["json"])
                    .pick_file()
            {
                app.document = crate::app::DocumentState::open(path);
            }
            if ui.button("登録機体").clicked() {
                app.document = ViewerApp::registered();
            }
            if ui.button("制御設定JSONを開く…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("制御設定JSON", &["json"])
                    .pick_file()
            {
                app.controller = crate::app::ControllerState::open(path);
                app.tab = Tab::Controller;
            }
            ui.label("選択中のタブへJSONをドラッグ&ドロップできる");
        });
        ui.label(&app.document.source);
        if let Some(hash) = &app.document.sha256 {
            ui.monospace(format!("SHA-256  {hash}  ({} bytes)", app.document.bytes));
        }
    });

    egui::CentralPanel::default().show(context, |ui| {
        ui.horizontal(|ui| {
            for (tab, title) in [
                (Tab::Design, "設計・導出諸元"),
                (Tab::Characteristics, "トリム・静特性"),
                (Tab::Planform, "平面形"),
                (Tab::Controller, "制御設定"),
            ] {
                ui.selectable_value(&mut app.tab, tab, title);
            }
        });
        ui.separator();
        if app.tab == Tab::Controller {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| show_controller(ui, app));
            return;
        }
        let loaded = match &app.document.result {
            Ok(loaded) => loaded,
            Err(error) => {
                ui.heading("読み込み結果");
                if let LoadError::Aircraft(AircraftDefinitionError::Format(
                    AircraftFormatError::Validation(issues),
                )) = error
                {
                    egui::ScrollArea::both().show(ui, |ui| {
                        for issue in issues {
                            validation_issue(
                                ui,
                                issue.code,
                                &issue.path,
                                &issue.value,
                                &issue.expected,
                                None,
                            );
                        }
                    });
                    return;
                }
                let detail = match error {
                    LoadError::Io(cause) => format!("ファイル読み込み: {cause}"),
                    LoadError::Aircraft(cause) => format!("{cause}\n{cause:?}"),
                    LoadError::Controller(cause) => cause.to_string(),
                };
                ui.colored_label(ui.visuals().error_fg_color, detail);
                return;
            }
        };
        let definition = &loaded.definition;
        let document = definition.document();
        ui.heading(&document.configuration_id);
        ui.label(format!(
            "model {}  ·  schema {}  ·  総質量 {:.3} kg",
            document.model_version,
            document.schema_version,
            definition.aircraft().airframe_mass_kg() + definition.aircraft().pilot_mass_kg()
        ));
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| match app.tab {
                Tab::Design => crate::design::show(ui, definition),
                Tab::Characteristics => crate::characteristics::show(ui, loaded),
                Tab::Planform => crate::characteristics::planform(ui, definition),
                Tab::Controller => {}
            });
    });
}

fn show_controller(ui: &mut egui::Ui, app: &ViewerApp) {
    ui.heading("制御設定");
    ui.label(&app.controller.source);
    let controller = match &app.controller.result {
        Ok(controller) => controller,
        Err(error) => {
            if let LoadError::Controller(
                birdman_game_format::controller::ControllerFormatError::Validation(issues),
            ) = error
            {
                for issue in issues {
                    controller_issue(ui, issue);
                }
                return;
            }
            let message = match error {
                LoadError::Io(cause) => cause.to_string(),
                LoadError::Controller(cause) => cause.to_string(),
                LoadError::Aircraft(cause) => cause.to_string(),
            };
            ui.colored_label(ui.visuals().error_fg_color, message);
            return;
        }
    };
    ui.label(format!(
        "{} · schema {} · 混合時FBW {:.0}%",
        controller.controller_id,
        controller.schema_version,
        controller.shared_authority * 100.0
    ));
    ui.label("文書: 有効");
    if let Ok(aircraft) = &app.document.result {
        let issues = controller.pair_issues(
            aircraft.definition.document(),
            aircraft.trim.theta_rad(),
            aircraft.trim.alpha_rad(),
        );
        if issues.is_empty() {
            ui.label("機体との整合: 有効");
        } else {
            for issue in issues {
                controller_issue(ui, &issue);
            }
        }
    } else {
        ui.label("機体JSONを読み込むと組合せを検査できる。");
    }
    ui.monospace(format!(
        "pitch: {:#?}\nyaw: {:#?}\n使用角範囲: {:?} rad\nbody rate範囲: {:?} rad/s\n迎角保護: {:?}",
        controller.pitch,
        controller.yaw,
        controller.incidence_bounds_rad,
        controller.body_rate_bounds_rad_per_second,
        controller.angle_of_attack_guard
    ));
}

pub fn row(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(label);
    ui.label(RichText::new(value.to_string()).monospace());
    ui.end_row();
}

fn controller_issue(
    ui: &mut egui::Ui,
    issue: &birdman_game_format::controller::ControllerValidationIssue,
) {
    validation_issue(
        ui,
        issue.code,
        &issue.path,
        &issue.value,
        &issue.expected,
        issue
            .aircraft
            .as_ref()
            .map(|peer| (peer.path.as_str(), peer.value.as_str())),
    );
}

fn validation_issue(
    ui: &mut egui::Ui,
    code: impl std::fmt::Debug,
    path: &str,
    value: &str,
    expected: &str,
    aircraft: Option<(&str, &str)>,
) {
    ui.group(|ui| {
        ui.monospace(format!("{code:?} · {path}: {value}"));
        ui.label(format!("必要な条件: {expected}"));
        if let Some((path, value)) = aircraft {
            ui.monospace(format!("機体側 {path}: {value}"));
        }
    });
}
