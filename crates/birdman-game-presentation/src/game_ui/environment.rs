use birdman_game_core::{NedVector, SessionScenarioIdentity, WindField};
use birdman_game_format::{EnvironmentBasisDocument, EnvironmentFormatError};

use birdman_game_app::{
    HybridSessionPreparation, bundled_environment, preset_environment_for_version,
};

pub(super) fn registered(identity: SessionScenarioIdentity) -> bool {
    HybridSessionPreparation::weather_for_environment_identity(identity.into())
        .is_ok_and(|weather| weather.is_some())
}

pub(super) fn registered_wind(
    identity: SessionScenarioIdentity,
) -> Result<Option<WindField<'static>>, EnvironmentFormatError> {
    if !registered(identity) {
        return Ok(None);
    }
    if identity.environment_version == 6 {
        return bundled_environment()?.wind_field().map(Some);
    }
    let Some(preset) = preset_environment_for_version(identity.environment_version) else {
        return Ok(None);
    };
    let [north, east, down] = preset.wind_velocity_ned_mps;
    let velocity = NedVector::try_new(north, east, down)
        .map_err(|_| EnvironmentFormatError::InvalidWindSample(0))?;
    Ok(Some(WindField::uniform(velocity)))
}

/// Formats registered weather from the same immutable provider used by flight preparation.
pub fn game_ui_environment_readout(
    identity: Option<SessionScenarioIdentity>,
    position: [f64; 3],
) -> Result<String, EnvironmentFormatError> {
    format_environment_readout(identity, false, position)
}

pub(super) fn game_ui_environment_diagnostics(
    identity: Option<SessionScenarioIdentity>,
    position: [f64; 3],
) -> Result<String, EnvironmentFormatError> {
    format_environment_readout(identity, true, position)
}

fn format_environment_readout(
    identity: Option<SessionScenarioIdentity>,
    technical: bool,
    selected_position: [f64; 3],
) -> Result<String, EnvironmentFormatError> {
    let Some(identity) = identity.filter(|identity| registered(*identity)) else {
        return Ok(if technical {
            "代表風情報を取得できない。"
        } else {
            "気象情報は準備完了後に表示する。"
        }
        .to_owned());
    };
    let (name, position, velocity, basis, variation) = if identity.environment_version == 6 {
        let environment = bundled_environment()?;
        let document = environment.document();
        let basis = match document.provenance.wind_grid {
            EnvironmentBasisDocument::Observed { .. } => "観測値",
            EnvironmentBasisDocument::Derived { .. } => "資料から導出した環境",
            EnvironmentBasisDocument::Assumed { .. } => "仮定した合成環境",
            EnvironmentBasisDocument::GameTuned { .. } => "ゲーム用に調整した環境",
        };
        (
            document.name.as_str(),
            selected_position,
            environment
                .wind_field()?
                .velocity_at(
                    birdman_game_core::NedPoint::try_new(
                        selected_position[0],
                        selected_position[1],
                        selected_position[2],
                    )
                    .map_err(|_| EnvironmentFormatError::InvalidWindPosition)?,
                )
                .map_err(EnvironmentFormatError::Wind)?
                .components(),
            basis,
            "場所により風向・風速が変わる",
        )
    } else {
        let preset = preset_environment_for_version(identity.environment_version)
            .ok_or(EnvironmentFormatError::InvalidIdentity)?;
        (
            preset.name,
            selected_position,
            preset.wind_velocity_ned_mps,
            "仮定した合成環境",
            "位置によらず一定の風",
        )
    };
    let [north, east, down] = velocity;
    let speed = north.hypot(east);
    let direction = if speed == 0.0 {
        "方向なし（水平風は無風）".to_owned()
    } else {
        let degrees = (-east).atan2(-north).to_degrees().rem_euclid(360.0);
        let compass = ["北", "北東", "東", "南東", "南", "南西", "西", "北西"]
            [(degrees / 45.0).round() as usize % 8];
        format!("{compass}から（{degrees:.1}°）")
    };
    let vertical = if down == 0.0 {
        "鉛直流なし".to_owned()
    } else {
        format!(
            "{} {:.2} m/s",
            if down < 0.0 { "上昇" } else { "下降" },
            down.abs()
        )
    };
    let mut readout = format!(
        "代表風: {speed:.2} m/s · {direction}\n水面から {:.1} mでの代表値 · {vertical}\n{variation}",
        -position[2]
    );
    if technical {
        readout.push_str(&format!(
            "\n環境名: {name} · {basis}\n速度成分 N/E/D: {north:.2} / {east:.2} / {down:.2} m/s\n代表地点 N/E/D: {:.1} / {:.1} / {:.1} m",
            position[0], position[1], position[2]
        ));
    }
    Ok(readout)
}
