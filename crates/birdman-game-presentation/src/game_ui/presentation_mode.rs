/// Presentation mode supported by a platform adapter.
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_only, derive(vstd::prelude::Structural))]
#[cfg_attr(not(verus_only), derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(all(feature = "schema", not(verus_only)), derive(schemars::JsonSchema))]
#[cfg_attr(not(verus_only), serde(rename_all = "snake_case"))]
pub enum GameUiPresentationMode {
    /// Flat native or browser display.
    Screen,
    /// Head-mounted stereo display provided by the platform runtime.
    Hmd,
    /// Browser phone headset display.
    PhoneVr,
}
