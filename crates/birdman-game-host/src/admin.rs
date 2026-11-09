//! Native administration using the same typed host commands as the Bevy frontend.

use crate::{
    server_cli::ServerOptions,
    service::{
        HostListenConfig, HostManagementCommand, HostService, HostServiceHandle, HostTlsConfig,
    },
    web_distribution::WebDistribution,
};
use birdman_game_app::application::ApplicationSession;
use birdman_game_runtime::{
    APPLICATION_CONTRACT_SHA256, ApplicationHostIdentity, ApplicationServer,
    ApplicationServerCommand,
};
use eframe::egui;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Launch the native management UI after validating the selected same-version bundle.
pub fn run(options: ServerOptions, source_sha: &str) -> Result<(), String> {
    let web = Arc::new(
        WebDistribution::open(&options.web_root, source_sha, APPLICATION_CONTRACT_SHA256)
            .map_err(|error| error.to_string())?,
    );
    let server = ApplicationServer::new(
        ApplicationSession::default(),
        ApplicationHostIdentity {
            source_sha: source_sha.to_owned(),
            contract_sha256: APPLICATION_CONTRACT_SHA256.to_owned(),
            web_bundle_sha256: web.manifest_sha256().to_owned(),
        },
    )
    .map_err(|error| error.to_string())?;
    let service = HostService::new(server, Instant::now(), options.records)?;
    let handle = service.handle();
    let tls = options.certificate.zip(options.private_key).map(
        |(certificate_chain_pem, private_key_pem)| HostTlsConfig {
            certificate_chain_pem,
            private_key_pem,
        },
    );
    let initial = HostListenConfig {
        bind: options.bind,
        tls,
        web,
    };
    handle.administer(HostManagementCommand::Start(initial.clone()))?;
    let result = eframe::run_native(
        "鳥人間滑空ゲーム 配信管理",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 540.0]),
            ..Default::default()
        },
        Box::new(move |context| {
            let mut fonts = egui::FontDefinitions::empty();
            fonts.font_data.insert(
                "mplus".into(),
                egui::FontData::from_static(include_bytes!(
                    "../../../assets/fonts/MPLUS1p-Regular.ttf"
                ))
                .into(),
            );
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("mplus".into());
            }
            context.egui_ctx.set_fonts(fonts);
            Ok(Box::new(Administration {
                handle,
                address: initial.bind.to_string(),
                config: initial,
                error: None,
            }))
        }),
    )
    .map_err(|error| error.to_string());
    let shutdown = service.shutdown();
    result.and(shutdown)
}

struct Administration {
    handle: HostServiceHandle,
    address: String,
    config: HostListenConfig,
    error: Option<String>,
}

impl Administration {
    fn command(&mut self, command: HostManagementCommand) {
        self.error = self.handle.administer(command).err();
    }
}

impl eframe::App for Administration {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        let snapshot = self.handle.snapshot();
        egui::CentralPanel::default().show(context, |ui| {
            ui.heading("配信サーバー");
            ui.horizontal(|ui| {
                ui.label("待受アドレス");
                ui.add_enabled(
                    snapshot.listening.is_none() && !snapshot.starting,
                    egui::TextEdit::singleline(&mut self.address),
                );
                if ui
                    .add_enabled(
                        snapshot.listening.is_none() && !snapshot.starting,
                        egui::Button::new("配信を開始"),
                    )
                    .clicked()
                {
                    match self.address.parse() {
                        Ok(bind) => {
                            self.config.bind = bind;
                            self.command(HostManagementCommand::Start(self.config.clone()));
                        }
                        Err(error) => self.error = Some(format!("待受アドレスを確認する: {error}")),
                    }
                }
                if ui
                    .add_enabled(
                        snapshot.listening.is_some(),
                        egui::Button::new("配信を停止"),
                    )
                    .clicked()
                {
                    self.command(HostManagementCommand::Stop);
                }
            });
            if let Some(address) = snapshot.listening {
                if address.ip().is_unspecified() {
                    ui.label(format!("待受中: {address}"));
                    ui.label(format!(
                        "他の端末では、このPCのLANアドレスとポート {} を指定して接続する。",
                        address.port()
                    ));
                } else {
                    ui.label(format!(
                        "接続先: {}://{address}/",
                        if snapshot.https { "https" } else { "http" }
                    ));
                }
            }
            if ui.button("飛行を一時停止").clicked() {
                self.command(HostManagementCommand::Application(
                    ApplicationServerCommand::Pause,
                ));
            }
            ui.separator();
            ui.heading("接続クライアント");
            for peer in &snapshot.application.peers {
                ui.horizontal(|ui| {
                    let operator = snapshot.application.operator == Some(peer.id);
                    ui.label(format!(
                        "接続 {} · {}",
                        peer.id,
                        if operator { "操作担当" } else { "閲覧" }
                    ));
                    if ui
                        .add_enabled(!operator, egui::Button::new("操作を許可"))
                        .clicked()
                    {
                        self.command(HostManagementCommand::Application(
                            ApplicationServerCommand::AssignOperator {
                                peer: Some(peer.id),
                            },
                        ));
                    }
                    if ui.button("切断").clicked() {
                        self.command(HostManagementCommand::Application(
                            ApplicationServerCommand::Disconnect { peer: peer.id },
                        ));
                    }
                });
            }
            if ui
                .add_enabled(
                    snapshot.application.operator.is_some(),
                    egui::Button::new("操作権を解除"),
                )
                .clicked()
            {
                self.command(HostManagementCommand::Application(
                    ApplicationServerCommand::AssignOperator { peer: None },
                ));
            }
            if let Some(error) = self
                .error
                .as_ref()
                .or(snapshot.diagnostic.as_ref())
                .or(snapshot.application.diagnostic.as_ref())
            {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, error);
            }
        });
        context.request_repaint_after(Duration::from_millis(100));
    }
}
