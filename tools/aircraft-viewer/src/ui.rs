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
            ui.label("ファイルのドラッグ&ドロップにも対応");
        });
        ui.label(&app.document.source);
        if let Some(hash) = &app.document.sha256 {
            ui.monospace(format!("SHA-256  {hash}  ({} bytes)", app.document.bytes));
        }
    });

    egui::CentralPanel::default().show(context, |ui| {
        let loaded = match &app.document.result {
            Ok(loaded) => loaded,
            Err(error) => {
                ui.heading("読み込み結果");
                if let LoadError::Aircraft(AircraftDefinitionError::Format(
                    AircraftFormatError::Validation(issues),
                )) = error
                {
                    egui::ScrollArea::both().show(ui, |ui| {
                        egui::Grid::new("aircraft_validation")
                            .striped(true)
                            .show(ui, |ui| {
                                for heading in ["分類", "JSON項目", "入力値", "必要な条件"]
                                {
                                    ui.strong(heading);
                                }
                                ui.end_row();
                                for issue in issues {
                                    ui.monospace(format!("{:?}", issue.code));
                                    ui.monospace(&issue.path);
                                    ui.monospace(&issue.value);
                                    ui.label(&issue.expected);
                                    ui.end_row();
                                }
                            });
                    });
                    return;
                }
                let detail = match error {
                    LoadError::Io(cause) => format!("ファイル読み込み: {cause}"),
                    LoadError::Aircraft(cause) => format!("{cause}\n{cause:?}"),
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
        ui.horizontal(|ui| {
            for (tab, title) in [
                (Tab::Design, "設計・導出諸元"),
                (Tab::Characteristics, "トリム・静特性"),
                (Tab::Planform, "平面形"),
            ] {
                ui.selectable_value(&mut app.tab, tab, title);
            }
        });
        ui.separator();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| match app.tab {
                Tab::Design => crate::design::show(ui, definition),
                Tab::Characteristics => crate::characteristics::show(ui, loaded),
                Tab::Planform => crate::characteristics::planform(ui, definition),
            });
    });
}

pub fn row(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(label);
    ui.label(RichText::new(value.to_string()).monospace());
    ui.end_row();
}
