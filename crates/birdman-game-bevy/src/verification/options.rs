use super::{Verification, VerificationCompletion, WaterVerificationCase};
use crate::app::{
    offscreen, read_input, screen_render::timing::TimingMode, ui, water_reflection, world,
};
use bevy::{app::ScheduleRunnerPlugin, prelude::*, window::ExitCondition};
use std::{error::Error, path::PathBuf};

#[derive(Default)]
pub(crate) struct Options {
    directory: Option<PathBuf>,
    size: Option<(u32, u32)>,
    water: Option<WaterVerificationCase>,
    water_auto: bool,
    timing: Option<String>,
    replay_cameras: bool,
    offscreen: bool,
}

impl Options {
    pub(crate) fn parse_argument(
        &mut self,
        argument: &str,
        arguments: &mut impl Iterator<Item = String>,
    ) -> Result<bool, Box<dyn Error>> {
        match argument {
            "--offscreen" => self.offscreen = true,
            "--verify-water-auto" => self.water_auto = true,
            "--verify-replay-cameras" => self.replay_cameras = true,
            "--verify" => {
                self.directory = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--verify requires an output directory")?,
                ));
            }
            "--verify-size" => {
                self.size = Some(parse_verification_size(
                    &arguments
                        .next()
                        .ok_or("--verify-size requires WIDTHxHEIGHT")?,
                )?);
            }
            "--verify-water" => {
                self.water = Some(WaterVerificationCase::parse(
                    &arguments
                        .next()
                        .ok_or("--verify-water requires WEATHER-QUALITY")?,
                )?);
            }
            "--verify-timing" => {
                self.timing = Some(
                    arguments
                        .next()
                        .ok_or("--verify-timing requires root or full")?,
                );
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.size.is_some() && self.directory.is_none() {
            return Err("--verify-size requires --verify DIR");
        }
        if self.water.is_some() && self.directory.is_none() {
            return Err("--verify-water requires --verify DIR");
        }
        if self.offscreen && self.directory.is_none() {
            return Err("--offscreen requires --verify DIR");
        }
        if self.replay_cameras && self.directory.is_none() {
            return Err("--verify-replay-cameras requires --verify DIR");
        }
        if self.replay_cameras && self.water.is_some() {
            return Err("--verify-replay-cameras cannot be combined with --verify-water");
        }
        self.timing_mode()?;
        verification_water_auto(self.water, self.water_auto)?;
        Ok(())
    }

    pub(crate) fn timing_mode(&self) -> Result<TimingMode, &'static str> {
        verification_timing_mode(self.water.is_some(), self.timing.as_deref())
    }

    pub(crate) fn offscreen(&self) -> bool {
        self.offscreen
    }

    pub(crate) fn verify_font(&self, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        if self.directory.is_some() {
            super::ui_validation::verify_text_layout(bytes)?;
        }
        Ok(())
    }

    pub(crate) fn window_plugin(&self, mut plugin: WindowPlugin) -> WindowPlugin {
        if let Some(size) = self.size {
            plugin
                .primary_window
                .as_mut()
                .expect("Native primary window")
                .resolution = size.into();
        }
        if self.offscreen {
            plugin.exit_condition = ExitCondition::DontExit;
        }
        plugin
    }

    pub(crate) fn configure_output(
        &self,
        app: &mut App,
        no_vsync: bool,
    ) -> Result<(), Box<dyn Error>> {
        if self.offscreen {
            offscreen::configure(app, self.size.unwrap_or((1280, 720)))?;
            let runner = offscreen_runner(no_vsync);
            println!(
                "Offscreen runner: {:?}; wait=None is unpaced. Logical Window PresentMode does not control Image presentation.",
                runner.run_mode
            );
            app.add_plugins(runner);
        }
        app.add_systems(
            Startup,
            offscreen::redirect_main_camera
                .after(world::setup_world)
                .before(water_reflection::setup_reflection),
        );
        Ok(())
    }

    pub(crate) fn install(
        self,
        app: &mut App,
    ) -> Result<Option<VerificationCompletion>, Box<dyn Error>> {
        let Some(directory) = self.directory else {
            return Ok(None);
        };
        let water = verification_water_auto(self.water, self.water_auto)?;
        if water.is_some() {
            app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
                .add_systems(
                    Update,
                    super::metrics::maintain_timing_history
                        .after(bevy::diagnostic::FrameTimeDiagnosticsPlugin::diagnostic_system)
                        .before(super::project_observer),
                );
        }
        let (verification, completion) = Verification::try_new(
            directory,
            app.world().resource::<AssetServer>(),
            water,
            offscreen::output_target(app.world()),
            self.replay_cameras,
        )?;
        app.insert_resource(verification)
            .add_plugins(super::render_readiness::VerificationRenderPlugin)
            .add_systems(PreUpdate, super::supply_input.after(read_input))
            .add_systems(Update, super::project_observer.before(world::project_world))
            .add_systems(Update, super::advance.after(ui::update_ui))
            .add_systems(
                PostUpdate,
                super::ui_validation::verify_ui_layout.after(bevy::ui::UiSystems::PostLayout),
            );
        Ok(Some(completion))
    }

    pub(crate) fn print_help() {
        println!(
            "検証feature有効: --verify DIR [--offscreen] [--verify-size WIDTHxHEIGHT] [--verify-water WEATHER-QUALITY [--verify-timing root|full] [--verify-water-auto] | --verify-replay-cameras]\n--verify: logical input/core loopとGPU画像保存を検査する。物理キー操作の検査ではない。\n--offscreen: --verifyをOSウィンドウなしで実行し、本番sceneとUIをImageへ描画する。--no-vsync併用時はrunnerの待機を解除する。\n--verify-size: 検査viewportの寸法を指定する。通常起動の寸法とOS表示倍率は変更しない。\n--verify-water: calm/typicalとlow/medium/highの6条件から1条件を検査する。例: typical-low。\n--verify-timing: --verify-waterの診断方式。rootはManualにもroot-only計測を許可し、fullは全pass診断を使用する。省略時はfull。\n--verify-water-auto: 水面の5 motionを通常UIと同じ実Auto policyで観測する。Auto指定のない水面比較はManual100を明示する。\n--verify-replay-cameras: 自然着水した実記録の中間時刻を共有UIの全cameraから撮影する。--verify-waterとの同時指定は不可。"
        );
    }
}

fn verification_timing_mode(
    water_verification: bool,
    value: Option<&str>,
) -> Result<TimingMode, &'static str> {
    match (water_verification, value) {
        (false, None) => Ok(TimingMode::Automatic),
        (false, Some(_)) => Err("--verify-timing requires --verify-water"),
        (true, None | Some("full")) => Ok(TimingMode::Full),
        (true, Some("root")) => Ok(TimingMode::Root),
        (true, Some(_)) => Err("--verify-timing must be root or full"),
    }
}

fn verification_water_auto(
    case: Option<WaterVerificationCase>,
    enabled: bool,
) -> Result<Option<WaterVerificationCase>, &'static str> {
    match (case, enabled) {
        (None, true) => Err("--verify-water-auto requires --verify-water"),
        (Some(case), true) => Ok(Some(case.with_auto())),
        (case, false) => Ok(case),
    }
}

fn offscreen_runner(no_vsync: bool) -> ScheduleRunnerPlugin {
    if no_vsync {
        ScheduleRunnerPlugin::default()
    } else {
        ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0))
    }
}

fn parse_verification_size(value: &str) -> Result<(u32, u32), &'static str> {
    let (width, height) = value
        .split_once('x')
        .ok_or("--verify-size requires WIDTHxHEIGHT")?;
    match (
        width.parse::<std::num::NonZeroU32>(),
        height.parse::<std::num::NonZeroU32>(),
    ) {
        (Ok(width), Ok(height)) => Ok((width.get(), height.get())),
        _ => Err("--verify-size dimensions must be positive u32 integers"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<Options, Box<dyn Error>> {
        let mut options = Options::default();
        let mut arguments = values.iter().map(|value| (*value).to_owned());
        while let Some(argument) = arguments.next() {
            assert!(options.parse_argument(&argument, &mut arguments)?);
        }
        Ok(options)
    }

    #[test]
    fn verification_options_validate_before_app_initialization() {
        for arguments in [
            vec!["--offscreen"],
            vec!["--verify-size", "800x600"],
            vec!["--verify-water", "calm-low"],
            vec!["--verify-replay-cameras"],
            vec!["--verify", "output", "--verify-water-auto"],
            vec!["--verify", "output", "--verify-timing", "root"],
            vec![
                "--verify",
                "output",
                "--verify-water",
                "calm-low",
                "--verify-replay-cameras",
            ],
        ] {
            assert!(
                parse(&arguments).unwrap().validate().is_err(),
                "{arguments:?}"
            );
        }
        let options = parse(&[
            "--verify",
            "output",
            "--offscreen",
            "--verify-size",
            "800x600",
        ])
        .unwrap();
        options.validate().unwrap();
        assert!(options.offscreen());
        let plugin = options.window_plugin(WindowPlugin {
            primary_window: Some(Window::default()),
            ..Default::default()
        });
        let window = plugin.primary_window.unwrap();
        assert_eq!((window.width(), window.height()), (800.0, 600.0));
        assert!(matches!(plugin.exit_condition, ExitCondition::DontExit));
    }

    #[test]
    fn verification_arguments_reject_missing_values_and_preserve_unknown_options() {
        for argument in [
            "--verify",
            "--verify-size",
            "--verify-water",
            "--verify-timing",
        ] {
            assert!(
                Options::default()
                    .parse_argument(argument, &mut std::iter::empty())
                    .is_err()
            );
        }
        assert!(
            !Options::default()
                .parse_argument("--font", &mut std::iter::empty())
                .unwrap()
        );
    }

    #[test]
    fn water_auto_is_explicit_and_requires_a_water_case() {
        let case = WaterVerificationCase::parse("typical-high").unwrap();
        assert_eq!(verification_water_auto(None, false), Ok(None));
        assert!(verification_water_auto(None, true).is_err());
        assert_eq!(verification_water_auto(Some(case), false), Ok(Some(case)));
        assert_eq!(
            verification_water_auto(Some(case), true),
            Ok(Some(case.with_auto()))
        );
        assert_ne!(case, case.with_auto());
    }

    #[test]
    fn verification_timing_only_overrides_explicit_water_diagnostics() {
        for (water, value, expected) in [
            (false, None, TimingMode::Automatic),
            (true, None, TimingMode::Full),
            (true, Some("full"), TimingMode::Full),
            (true, Some("root"), TimingMode::Root),
        ] {
            assert_eq!(verification_timing_mode(water, value), Ok(expected));
        }
        for value in ["root", "full", "", "automatic", "ROOT"] {
            assert!(verification_timing_mode(false, Some(value)).is_err());
        }
        for value in ["", "automatic", "ROOT"] {
            assert!(verification_timing_mode(true, Some(value)).is_err());
        }
    }

    #[test]
    fn offscreen_runner_only_removes_wait_when_explicitly_selected() {
        for (no_vsync, expected_wait) in [
            (false, Some(std::time::Duration::from_secs_f64(1.0 / 60.0))),
            (true, None),
        ] {
            let bevy::app::RunMode::Loop { wait } = offscreen_runner(no_vsync).run_mode else {
                panic!("Offscreen verification must retain the continuous runner");
            };
            assert_eq!(wait, expected_wait);
        }
    }

    #[test]
    fn verification_size_accepts_explicit_positive_dimensions() {
        assert_eq!(parse_verification_size("800x600"), Ok((800, 600)));
        assert_eq!(parse_verification_size("1920x1080"), Ok((1920, 1080)));
    }

    #[test]
    fn verification_size_rejects_missing_zero_and_non_integer_dimensions() {
        for value in [
            "800",
            "800x",
            "x600",
            "0x600",
            "800x0",
            "-800x600",
            "800.5x600",
            "800x600x1",
            "4294967296x600",
        ] {
            assert!(parse_verification_size(value).is_err(), "{value}");
        }
    }
}
