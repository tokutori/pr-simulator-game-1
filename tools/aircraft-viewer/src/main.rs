mod app;
mod characteristics;
mod design;
mod ui;

fn main() -> eframe::Result {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        "機体JSON viewer",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1200.0, 820.0]),
            ..Default::default()
        },
        Box::new(move |context| {
            let mut fonts = eframe::egui::FontDefinitions::default();
            fonts.font_data.insert(
                "mplus".into(),
                eframe::egui::FontData::from_static(include_bytes!(
                    "../../../assets/fonts/MPLUS1p-Regular.ttf"
                ))
                .into(),
            );
            for family in [
                eframe::egui::FontFamily::Proportional,
                eframe::egui::FontFamily::Monospace,
            ] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("mplus".into());
            }
            context.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app::ViewerApp::new(initial_path)))
        }),
    )
}
