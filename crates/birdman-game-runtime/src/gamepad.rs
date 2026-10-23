use birdman_game_app::flight_input::{FlightDeviceIdentity, FlightGamepadObservation};
use birdman_game_presentation::xr::XrGamepadObservation;
use serde::{Deserialize, Serialize};

/// Unfiltered platform gamepad data in the standard browser/OpenXR mapping vocabulary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationGamepadObservation {
    /// Physical slot/entity and observed connection generation.
    pub identity: FlightDeviceIdentity,
    /// Raw connection status.
    pub connected: bool,
    /// Platform mapping declaration, such as standard or xr-standard.
    pub mapping: String,
    /// Raw axis order; missing and non-finite samples are None.
    pub axes: Vec<Option<f64>>,
    /// Raw button pressed states in platform order.
    pub buttons: Vec<bool>,
}

impl ApplicationGamepadObservation {
    fn standard(&self) -> bool {
        self.connected && matches!(self.mapping.as_str(), "standard" | "standard-y-up")
    }

    fn axis(&self, index: usize) -> Option<f64> {
        self.axes
            .get(index)
            .copied()
            .flatten()
            .filter(|value| value.is_finite())
    }

    pub(super) fn flight(&self) -> Option<FlightGamepadObservation> {
        self.standard().then(|| FlightGamepadObservation {
            identity: self.identity,
            connected: true,
            axes: [
                self.axis(1).map(|value| {
                    if self.mapping == "standard-y-up" {
                        value
                    } else {
                        -value
                    }
                }),
                self.axis(0),
                self.axis(3).map(|value| {
                    if self.mapping == "standard-y-up" {
                        value
                    } else {
                        -value
                    }
                }),
            ],
        })
    }

    pub(super) fn menu(&self) -> Option<XrGamepadObservation> {
        self.standard().then(|| XrGamepadObservation {
            index: self.identity.slot_low,
            generation: self.identity.generation,
            axes: std::array::from_fn(|index| {
                let value = self.axis(index).unwrap_or(0.0);
                if self.mapping == "standard-y-up" && index % 2 == 1 {
                    -value
                } else {
                    value
                }
            }),
            buttons: [
                self.buttons.first().copied().unwrap_or(false),
                self.buttons.get(1).copied().unwrap_or(false),
            ],
        })
    }
}
