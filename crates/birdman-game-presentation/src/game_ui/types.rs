pub use super::presentation_mode::GameUiPresentationMode;
use birdman_game_app::application::{RecordCommand, SessionCommand};
use birdman_game_core::{FlightRecord, GameSession, SessionScenarioIdentity};
use birdman_game_format::WaterQuality;
use birdman_game_format::{DifficultyPreset, DifficultySettings, HudCue};
use serde::{Deserialize, Serialize};

/// Current shared UI boundary version.
pub const GAME_UI_SCHEMA_VERSION: u32 = 1;

/// Number of newest saved records requested by the Title preview.
pub const GAME_UI_STORED_RECORD_LIMIT: usize = 3;
/// Maximum Unicode scalar count of a displayed platform diagnostic.
pub const GAME_UI_NOTICE_CHARACTERS: usize = 2048;
/// Maximum Unicode scalar count of a saved-record display timestamp.
pub const GAME_UI_SAVED_AT_CHARACTERS: usize = 256;

/// UI-only selections; physical state and domain phase remain in GameSession.
#[derive(Clone, Debug, PartialEq)]
pub struct GameUiState {
    /// Shared credits shown without changing the domain phase.
    pub credits_open: bool,
    /// Expanded preparation diagnostics.
    pub technical_open: bool,
    /// Result presentation selection.
    pub result_tab: GameUiResultTab,
    /// Selected record plot.
    pub analysis_chart: GameUiAnalysisChart,
    /// Read-only analysis query position in seconds.
    pub analysis_time_seconds: f64,
    /// Paused menu selection.
    pub pause_overlay: GameUiPauseOverlay,
    /// Replay presentation selection.
    pub replay_view: GameUiReplayView,
    /// Shared live Screen camera selection; spatial displays keep the pilot mount.
    pub flight_camera: GameUiFlightCamera,
    /// Shared replay camera selection; adapters only apply its geometry.
    pub replay_camera: GameUiReplayCamera,
}

impl Default for GameUiState {
    fn default() -> Self {
        Self {
            credits_open: false,
            technical_open: false,
            result_tab: GameUiResultTab::Summary,
            analysis_chart: GameUiAnalysisChart::Map,
            analysis_time_seconds: 0.0,
            pause_overlay: GameUiPauseOverlay::Menu,
            replay_view: GameUiReplayView::Cinematic,
            replay_camera: GameUiReplayCamera::Auto,
            flight_camera: GameUiFlightCamera::Pilot,
        }
    }
}

/// Result presentation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiResultTab {
    /// Condensed result and navigation.
    Summary,
    /// Recorded analysis charts.
    Analysis,
}

/// Plot generated from the retained record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiAnalysisChart {
    /// Equal-scale horizontal trajectory.
    Map,
    /// Altitude over recorded time.
    Altitude,
    /// Air-relative and ground-relative speed.
    Speed,
}

/// Exclusive pause overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiPauseOverlay {
    /// Pause navigation.
    Menu,
    /// Render settings.
    Settings,
    /// Input instructions.
    Help,
}

/// Replay presentation selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiReplayView {
    /// Scenic replay.
    Cinematic,
    /// Scenic replay with telemetry.
    Telemetry,
    /// Record plot and shared cursor.
    Analysis,
}

/// Screen flight camera selection retained with the shared UI settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiFlightCamera {
    /// Pilot eye view.
    Pilot,
    /// External view following the aircraft.
    Chase,
}

impl GameUiFlightCamera {
    /// The other live Screen camera, used by both the button and C key.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Pilot => Self::Chase,
            Self::Chase => Self::Pilot,
        }
    }
    /// Shared menu label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pilot => "操縦席",
            Self::Chase => "追従",
        }
    }
}

/// Replay camera candidates in their shared selection order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiReplayCamera {
    /// Recorded-event camera director.
    Auto,
    /// Pilot eye view.
    Pilot,
    /// Aircraft-following view.
    Chase,
    /// Orbiting aircraft view.
    Orbit,
    /// Launch-platform fixed view.
    Platform,
    /// Lake-shore fixed view.
    Shore,
    /// Overhead aircraft view.
    Overhead,
    /// Side-on aircraft view.
    Side,
    /// Forward aircraft view.
    Front,
    /// Fixed distant telephoto view.
    Telephoto,
}

impl GameUiReplayCamera {
    /// Shared camera candidate order.
    pub const ALL: [Self; 10] = [
        Self::Auto,
        Self::Pilot,
        Self::Chase,
        Self::Orbit,
        Self::Platform,
        Self::Shore,
        Self::Overhead,
        Self::Side,
        Self::Front,
        Self::Telephoto,
    ];

    /// Shared camera label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "自動",
            Self::Pilot => "操縦席",
            Self::Chase => "追従",
            Self::Orbit => "周回",
            Self::Platform => "発進台",
            Self::Shore => "湖岸",
            Self::Overhead => "上空",
            Self::Side => "側面",
            Self::Front => "正面",
            Self::Telephoto => "望遠",
        }
    }
}

/// Exclusive observation of a platform-owned water resource operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiWaterQualityOperation {
    /// No resource operation is pending and prior resources were released.
    Idle {},
    /// A replacement has been requested and the confirmed resource remains active.
    Applying {
        /// Requested replacement quality.
        requested: WaterQuality,
    },
    /// Replacement failed before commitment; the confirmed resource remains active.
    Failed {
        /// Failed replacement quality.
        requested: WaterQuality,
        /// Captured resource preparation cause.
        cause: String,
    },
    /// Replacement committed, but release of the superseded resource failed.
    CleanupFailed {
        /// Captured superseded-resource cleanup cause.
        cause: String,
    },
}

/// Platform-owned operation requested by common UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiPlatformCommand {
    /// Reloads the application after its current resources have been released.
    ReloadApplication {},
    /// Changes only the Screen 3D drawing resolution.
    SetScreenRenderScale {
        /// Requested fraction of each physical rendering dimension.
        scale: crate::screen_render_scale::ScreenRenderScale,
    },
    /// Changes directional shadow quality in the shared rendering owner.
    SetShadowMap {
        /// Requested shadow quality.
        shadow: crate::screen_render_auto::ScreenShadowMap,
    },
    /// Enables Auto or restores its retained manual Screen resolution.
    SetScreenRenderAuto {
        /// Whether GPU-work-based adjustment is selected.
        enabled: bool,
    },
    /// Changes render-only mesh quality.
    SetWaterQuality {
        /// Requested quality.
        quality: WaterQuality,
    },
    /// Requests a supported display mode.
    SetPresentation {
        /// Requested mode.
        mode: GameUiPresentationMode,
    },
    /// Recalibrates phone heading.
    RecenterPhoneTracking {},
    /// Places the retained menu in front of the viewer.
    RecenterMenu {},
    /// Downloads the original JSON record.
    DownloadRecordJson {},
    /// Downloads record-derived numeric CSV.
    DownloadFlightCsv {},
    /// Loads a stored finalized record.
    OpenStoredRecord {
        /// Platform persistence key.
        id: u32,
    },
    /// Retries a failed Title record-list lookup.
    RetryStoredRecords {},
}

/// UI message whose handling is shared by native and browser adapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiAction {
    /// Resolves a widget observation against the current shared view.
    ControlInput {
        /// Button, toggle or range observation from an adapter.
        input: GameUiControlInput,
    },
    /// Opens or closes the shared legal notices.
    SetCredits {
        /// Whether the credits replace the current menu.
        open: bool,
    },
    /// Produces one canonical domain operation.
    Session {
        /// Operation admitted by the current view.
        command: SessionCommand,
    },
    /// Produces one platform operation.
    Platform {
        /// Explicit capability-bound operation.
        command: GameUiPlatformCommand,
    },
    /// Toggles preparation diagnostic disclosure.
    ToggleTechnical {},
    /// Selects the Result presentation.
    SetResultTab {
        /// Requested tab.
        tab: GameUiResultTab,
    },
    /// Selects a record plot.
    SetAnalysisChart {
        /// Requested plot.
        chart: GameUiAnalysisChart,
    },
    /// Selects an explicit pause overlay.
    SetPauseOverlay {
        /// Requested overlay.
        overlay: GameUiPauseOverlay,
    },
    /// Selects a replay presentation mode.
    SetReplayView {
        /// Requested view.
        view: GameUiReplayView,
    },
    /// Selects the live Screen camera.
    SetFlightCamera {
        /// Requested live camera.
        camera: GameUiFlightCamera,
    },
    /// Advances the common ordered camera selection.
    CycleReplayCamera {},
    /// Queries a recorded analysis instant without moving the playback clock.
    SeekAnalysis {
        /// Requested recorded time.
        time_seconds: f64,
    },
    /// Moves the Rust-owned replay clock.
    SeekReplay {
        /// Requested recorded time.
        time_seconds: f64,
    },
    /// Toggles the Rust-owned replay clock.
    ToggleReplayPlaying {},
    /// Changes playback rate by existing stable code.
    SetReplayRate {
        /// Zero is half speed, one normal speed, two double speed.
        code: u32,
    },
}

/// Engine-independent widget observation, resolved by the shared UI owner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GameUiControlInput {
    /// Activates an enabled button.
    Activate {
        /// Published control identifier.
        control_id: String,
    },
    /// Changes an enabled toggle.
    SetToggle {
        /// Published control identifier.
        control_id: String,
        /// Observed selection.
        value: bool,
    },
    /// Changes an enabled range value.
    SetRange {
        /// Published control identifier.
        control_id: String,
        /// Finite selected value within the published bounds.
        value: f64,
    },
    /// Observes a pointer position relative to an enabled range's horizontal track.
    SetRangeRatio {
        /// Published control identifier.
        control_id: String,
        /// Horizontal ratio from the track's left edge to its right edge.
        ratio: f64,
    },
    /// Observes keys newly pressed while an enabled interactive control has focus.
    ControlKeys {
        /// Published control identifier.
        control_id: String,
        /// Platform key codes observed in the same input frame.
        codes: Vec<String>,
    },
}

/// Side-effect request produced after pure action admission.
#[derive(Clone, Debug, PartialEq)]
pub enum GameUiEffect {
    /// Existing session command.
    Session {
        /// Operation to execute through the shared session owner.
        command: SessionCommand,
    },
    /// Platform operation.
    Platform {
        /// Operation to interpret at the platform boundary.
        command: GameUiPlatformCommand,
    },
    /// Queries existing record or clock APIs.
    Record {
        /// Admitted seek, clock toggle or rate action.
        action: RecordCommand,
    },
}

/// Pure update output.
#[derive(Clone, Debug, PartialEq)]
pub struct GameUiUpdate {
    /// Next UI-only selections.
    pub state: GameUiState,
    /// Ordered operations to interpret.
    pub effects: Vec<GameUiEffect>,
}

/// Browser or OS observations that never own game physics or phase.
#[derive(Clone, Debug, PartialEq)]
pub struct GameUiPlatformInput {
    /// Current physical display viewport.
    pub viewport: GameUiViewport,
    /// Measured chart content extent; absent before the platform has measured it.
    pub chart_extent: Option<GameUiChartExtent>,
    /// Actual active presentation mode.
    pub presentation_mode: GameUiPresentationMode,
    /// Blocks operations while the owning adapter is transitioning.
    pub operation_pending: bool,
    /// Platform runtime supports head-mounted stereo display.
    pub hmd_available: bool,
    /// Phone sensor headset support.
    pub phone_vr_available: bool,
    /// Platform has observed touch input support.
    pub touch_available: bool,
    /// Active spatial backend can reposition the menu in front of the viewer.
    pub menu_recenter_available: bool,
    /// Platform supports full record plots.
    pub analysis_available: bool,
    /// Platform supports recorded scenic playback.
    pub replay_available: bool,
    /// Platform supports persisted record lookup.
    pub stored_records_available: bool,
    /// The shared record owner admits an explicit retry of the failed Title list.
    pub stored_records_retry_available: bool,
    /// Platform supports record downloads.
    pub flight_log_available: bool,
    /// Confirmed current render quality.
    pub water_quality: WaterQuality,
    /// Actual resource preparation, commitment and cleanup observation.
    pub water_quality_operation: GameUiWaterQualityOperation,
    /// Confirmed Screen selection, applied resources, and stable measurement state.
    pub screen_render_scale: crate::screen_render_auto::ScreenRenderAutoSnapshot,
    /// Platform operation feedback.
    pub notification: String,
    /// Captured renderer failure; the session retains its last valid state.
    pub renderer_failure: Option<String>,
    /// Captured head-HUD resource failure, separate from the domain and physical state.
    pub head_hud_failure: Option<String>,
    /// Bounded newest-record preview; the persistence index and PB search remain separate.
    pub stored_records: GameUiStoredRecordPreview,
}

/// Title's bounded metadata query result, independent of all-record PB selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameUiStoredRecordPreview(Vec<GameUiStoredRecord>);

impl GameUiStoredRecordPreview {
    /// Returns the ordered rows without transferring record ownership.
    pub fn iter(&self) -> core::slice::Iter<'_, GameUiStoredRecord> {
        self.0.iter()
    }
}

impl From<Vec<GameUiStoredRecord>> for GameUiStoredRecordPreview {
    fn from(records: Vec<GameUiStoredRecord>) -> Self {
        Self(
            records
                .into_iter()
                .take(GAME_UI_STORED_RECORD_LIMIT)
                .map(|mut record| {
                    record.saved_at = record
                        .saved_at
                        .chars()
                        .take(GAME_UI_SAVED_AT_CHARACTERS)
                        .collect();
                    record
                })
                .collect(),
        )
    }
}

/// Persistence metadata, not an independent flight record.
#[derive(Clone, Debug, PartialEq)]
pub struct GameUiStoredRecord {
    /// Platform key.
    pub id: u32,
    /// Original RFC 3339 save timestamp; the shared view formats it in UTC.
    pub saved_at: String,
    /// Existing Rust-derived Personal Best membership.
    pub personal_best: bool,
}

/// Logical pixels and root-font size used by the same responsive calculation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiViewport {
    /// Display width in logical pixels.
    pub width: f64,
    /// Display height in logical pixels.
    pub height: f64,
    /// Platform root font size in logical pixels.
    pub root_font_size: f64,
}

/// Borrowed domain and query inputs. This value owns no game state.
pub struct GameUiContext<'view, 'session> {
    /// Save completion feedback belonging to the displayed record.
    pub record_notification: Option<&'view str>,
    /// Existing game session owner.
    pub session: &'view GameSession<'session>,
    /// Resume admission from the application-owned live driver and domain session.
    pub resume_available: bool,
    /// Original live-driver failure retained until the flight is replaced.
    pub flight_failure: Option<birdman_game_app::application::FlightFrameError>,
    /// Admission from retained backend, focus and tracking observations.
    pub presentation_available: bool,
    /// Existing selected or saved settings.
    pub difficulty: DifficultySettings,
    /// Selected keyboard response, editable before flight preparation.
    pub keyboard_controls: birdman_game_app::flight_input::KeyboardControlSettings,
    /// Admitted Up, Down, Left, Right, J, L, C, P, Escape, W, S, A and D keys.
    pub accepted_flight_keys: [bool; 13],
    /// Exact displayed preset retained for archived selections.
    pub preset: DifficultyPreset,
    /// Optional selected identity before preparation or sealed identity thereafter.
    pub identity: Option<SessionScenarioIdentity>,
    /// Existing authoritative record selected for this view.
    pub record: Option<&'view FlightRecord>,
    /// Saved comparison/provenance metadata when an external archive is displayed.
    pub archived_identity: Option<&'view birdman_game_format::FlightRecordIdentityMetadata>,
    /// Registered environment briefing readout derived in Rust.
    pub environment_readout: &'view str,
    /// Actual adapter observations and capabilities.
    pub platform: &'view GameUiPlatformInput,
}

/// Finite UI boundary, record or action-admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameUiError {
    /// The structural view revision cannot advance without wrapping.
    RevisionExhausted,
    /// A lightweight observation targets a superseded structural view.
    StaleView,
    /// Viewport dimensions are not positive finite logical pixels.
    InvalidViewport,
    /// An action is absent, disabled or incompatible with the current view.
    ActionUnavailable,
    /// Time is nonfinite or outside the retained record.
    InvalidTime,
    /// The authoritative record query failed.
    Record(birdman_game_core::FlightRecordQueryError),
    /// The authoritative session could not produce telemetry or a score.
    Session(birdman_game_core::GameSessionError),
    /// A registered offline environment could not be projected.
    Environment(birdman_game_format::EnvironmentFormatError),
    /// The bundled venue chart asset is malformed.
    InvalidVenueMap,
}
#[cfg(not(verus_only))]
impl core::fmt::Display for GameUiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RevisionExhausted => f.write_str("UI view revision identities are exhausted"),
            Self::StaleView => f.write_str("UI observation targets a superseded view"),
            Self::InvalidViewport => {
                f.write_str("UI viewport must contain positive finite dimensions")
            }
            Self::ActionUnavailable => {
                f.write_str("UI action is not available in the current view")
            }
            Self::InvalidTime => f.write_str("UI time is nonfinite or outside the retained record"),
            Self::Record(_) => f.write_str("UI record query failed"),
            Self::Session(_) => f.write_str("UI session projection failed"),
            Self::Environment(_) => f.write_str("UI environment projection failed"),
            Self::InvalidVenueMap => f.write_str("Venue map asset is invalid"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for GameUiError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Record(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::Environment(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Shared scene name, preserving current Web spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GameUiScene {
    /// Initial platform preparation.
    Boot,
    /// Flight and demo entry.
    Title,
    /// Flight settings.
    FlightSetup,
    /// Prepared flight confirmation.
    Briefing,
    /// Launch countdown.
    Countdown,
    /// Active or paused flight.
    Flight,
    /// Finalized result.
    Result,
    /// Saved snapshot playback.
    Replay,
}

impl GameUiScene {
    /// Classifies all scenes for the shared Screen rendering policy.
    pub const fn screen_render_scope(self) -> crate::screen_render_auto::ScreenRenderScope {
        use crate::screen_render_auto::ScreenRenderScope;

        match self {
            Self::FlightSetup | Self::Briefing | Self::Countdown | Self::Flight => {
                ScreenRenderScope::FlightSettings
            }
            Self::Boot | Self::Title | Self::Result | Self::Replay => ScreenRenderScope::Background,
        }
    }
}

/// Pure shared scene and menu projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiView {
    /// Opaque semantic view identity for focus and scroll ownership.
    pub interaction_scope: String,
    /// Boundary version.
    pub schema_version: u32,
    /// Adapter query-cache revision; pure projections use zero until published.
    pub view_revision: u32,
    /// Internal selections used by Rust action admission and interaction scope.
    #[serde(skip)]
    #[cfg_attr(feature = "schema", schemars(skip))]
    pub selections: GameUiState,
    /// Domain-derived scene.
    pub scene: GameUiScene,
    /// Shared heading.
    pub title: String,
    /// Shared explanatory text.
    pub description: String,
    /// Shared exclusive overlay name.
    pub active_overlay: Option<GameUiOverlay>,
    /// Contextual Back operation selected from enabled shared controls.
    pub back_action: Option<GameUiAction>,
    /// Ordered semantic panels.
    pub panels: Vec<GameUiPanel>,
    /// Shared responsive dimensions and scroll policy.
    pub layout: GameUiLayout,
    /// Shared visual hierarchy.
    pub theme: GameUiTheme,
    /// Shared telemetry and instrument projection, absent outside a flight or record.
    pub hud: Option<GameUiHud>,
}

/// Lightweight per-frame UI observation associated with one published structural view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiFrame {
    /// Structural view revision retained by the query adapter.
    pub view_revision: u32,
    /// Shared HUD projection, never a second physical state.
    pub hud: Option<GameUiHud>,
    /// Existing range values that advance without a structural change.
    pub ranges: Vec<GameUiRangeValue>,
    /// Fixed record-clock and analysis-cursor readouts.
    pub readouts: Vec<GameUiReadoutValue>,
    /// Numeric readouts that advance with the retained record cursor.
    pub fields: Vec<GameUiFieldsValue>,
    /// Replay clock play/pause label.
    pub button_labels: Vec<GameUiButtonLabel>,
    /// Cursor-only chart observation; plotted series and bounds remain in the view.
    pub chart_cursor: Option<GameUiChartCursor>,
    /// Pointer-control indicators associated with the retained flight controls.
    pub flight_touch: Vec<GameUiFlightTouchValue>,
}

/// Completed local indicator geometry for a retained flight pointer control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiFlightTouchValue {
    /// Existing published control identity.
    pub id: String,
    /// Complete local-pixel knob centre.
    pub position: [f64; 2],
    /// Complete local-pixel relative-drag origin, absent for an absolute slider.
    pub origin: Option<[f64; 2]>,
}

/// Value of a retained typed range control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiRangeValue {
    /// Retained control identity.
    pub id: String,
    /// Shared finite range value.
    pub value: f64,
    /// Shared one-decimal display text.
    pub display_value: String,
}

/// Text of a retained record readout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiReadoutValue {
    /// Retained control identity.
    pub id: String,
    /// Shared formatted record or clock observation.
    pub value: String,
}

/// Label of the retained replay play/pause action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiButtonLabel {
    /// Retained control identity.
    pub id: String,
    /// Shared clock-derived label.
    pub label: String,
}

/// Lightweight cursor and selected-sample wind for the retained plot bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartCursor {
    /// Retained chart control identity.
    pub id: String,
    /// Horizontal position in logical display pixels, absent for a map.
    pub cursor_x: Option<f64>,
    /// Shared plot-space sample points.
    pub cursor_points: Vec<GameUiChartCursorPoint>,
    /// Selected-sample wind, replacing the first map vector only.
    pub selected_wind_vector: Option<GameUiChartVector>,
}

/// Exclusive current overlay names retained by both renderer adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GameUiOverlay {
    /// Data and software attribution, from Title or paused flight.
    Credits,
    /// Paused flight navigation.
    Pause,
    /// Paused render settings.
    PauseSettings,
    /// Paused input help.
    PauseHelp,
}

/// Shared Web-baseline HUD model; no adapter rederives flight values or labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHud {
    /// Shared Web-baseline instrument placement and responsive dimensions.
    pub layout: GameUiHudLayout,
    /// Ordered groups containing every visible readout and instrument.
    pub groups: Vec<GameUiHudGroup>,
}

/// One shared horizontal row; identifiers only track its retained elements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudGroup {
    /// Stable group identity.
    pub id: String,
    /// Shared placement relative to the unobstructed forward view.
    pub placement: GameUiHudPlacement,
    /// Complete visible row in its shared left-to-right order.
    pub nodes: Vec<GameUiHudNode>,
}

/// HUD placement roles consumed identically by all adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiHudPlacement {
    /// Primary numerical readings on the left or first bottom band.
    Readouts,
    /// Instruments on the right or second bottom band.
    Instruments,
}

/// One existing widget instance; adding an instance requires no adapter-specific field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudNode {
    /// Stable identity used only to retain this element.
    pub id: String,
    /// Caption preceding the value in one formatted paragraph, independent of widget type.
    pub label: String,
    /// Shared normalized rectangle within the bottom HUD rail.
    pub bounds: GameUiRect,
    /// Instrument drawing height relative to the shared HUD font size; zero for text.
    pub instrument_height_em: f64,
    /// Exclusive widget geometry and display values.
    pub content: GameUiHudNodeContent,
}

/// Widget implementations must exhaustively handle each display primitive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiHudNodeContent {
    /// Formatted readout, instructions, attribution, or mandatory failure.
    Text {
        /// Fully formatted text; adapters never parse its wording.
        value: String,
        /// Shared visual priority.
        tone: GameUiHudTone,
    },
    /// ADI with an optional flight-path marker, or the marker alone.
    Attitude {
        /// Fully formatted pitch, roll, or flight-path reading.
        value: String,
        /// Shared horizon and marker geometry.
        geometry: Box<GameUiHudAdiGeometry>,
    },
    /// Compass tape or another readout using the same tape primitive.
    HeadingTape {
        /// Formatted central reading.
        value: String,
        /// Logical instrument canvas dimensions.
        canvas_size: [f64; 2],
        /// Ordered completed marks and labels.
        ticks: Vec<GameUiHudHeadingTick>,
        /// Fixed pointer, ordered tip and base corners.
        pointer: [GameUiChartPoint; 3],
    },
    /// Direction arrow with an explicit absence for calm or unavailable direction.
    Direction {
        /// Formatted direction and magnitude reading.
        value: String,
        /// Logical instrument canvas dimensions.
        canvas_size: [f64; 2],
        /// Circle outline bounds and corner radius.
        circle_bounds: GameUiRect,
        /// Circle outline radius in logical pixels.
        circle_radius: f64,
        /// Circle stroke width in logical pixels.
        circle_stroke_width: f64,
        /// Fixed compass reference segments.
        cross_segments: [[GameUiChartPoint; 2]; 2],
        /// Reference segment stroke width in logical pixels.
        cross_stroke_width: f64,
        /// North reference label.
        north_label: String,
        /// North label's middle baseline anchor.
        north_anchor: GameUiChartPoint,
        /// Completed filled arrow polygon, absent when no direction is defined.
        arrow: Option<[GameUiChartPoint; 4]>,
    },
    /// Linear gauge with a shared scale and explicit unavailable value.
    LinearGauge {
        /// Formatted reading including its unit.
        value: String,
        /// Left endpoint label.
        minimum_label: String,
        /// Right endpoint label.
        maximum_label: String,
        /// Logical instrument canvas dimensions.
        canvas_size: [f64; 2],
        /// Scale track endpoints.
        track: [GameUiChartPoint; 2],
        /// Ordered tick endpoints.
        ticks: Box<[[GameUiChartPoint; 2]; 7]>,
        /// Left-start and right-end alphabetic label baselines.
        label_anchors: [GameUiChartPoint; 2],
        /// Reading pointer, ordered tip and base corners; absent for an undefined reading.
        marker: Option<[GameUiChartPoint; 3]>,
    },
}

impl GameUiHudNodeContent {
    /// Returns the shared formatted reading without interpreting the node's identity.
    pub fn value(&self) -> &str {
        match self {
            Self::Text { value, .. }
            | Self::Attitude { value, .. }
            | Self::HeadingTape { value, .. }
            | Self::Direction { value, .. }
            | Self::LinearGauge { value, .. } => value,
        }
    }
}

/// Shared text priority; mandatory resource failures always use Warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiHudTone {
    /// Ordinary operational information.
    Normal,
    /// Secondary instructions or attribution.
    Muted,
    /// A mandatory visible failure or warning.
    Warning,
}

/// Instrument placement in logical pixels with platform safe-area insets applied by adapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudLayout {
    /// Left and bottom viewport insets; the bottom edge is the viewport anchor.
    pub offset_px: [f64; 2],
    /// Responsive rail width without scrolling.
    pub width_px: f64,
    /// Shared rail height enclosing every published node.
    pub height_px: f64,
    /// Horizontal and vertical padding.
    pub padding_px: [f64; 2],
    /// Horizontal and vertical grid gaps.
    pub gap_px: [f64; 2],
    /// ADI width and height.
    pub attitude_size_px: [f64; 2],
    /// Readout font size.
    pub font_size_px: f64,
    /// Individual card background opacity; the surrounding rail is transparent.
    pub opacity: f64,
}

/// Completed ADI drawing in its fixed logical canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudAdiGeometry {
    /// Complete logical drawing extent.
    pub canvas_size: [f64; 2],
    /// Rounded horizon clipping and outline rectangle.
    pub clip: GameUiRect,
    /// Radius of the rounded clipping corners.
    pub clip_radius: f64,
    /// Final horizon marks, absent for a flight-path-only instrument.
    pub horizon: Option<GameUiHudAttitudeGeometry>,
    /// Fixed aircraft reference segments.
    pub aircraft_segments: [[GameUiChartPoint; 2]; 6],
    /// Flight-path circle centre, absent when its direction is undefined.
    pub flight_path_center: Option<GameUiChartPoint>,
    /// Flight-path circle radius.
    pub flight_path_radius: f64,
}

/// Completed horizon marks and label orientation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudAttitudeGeometry {
    /// Clockwise rotation of each label about its final baseline anchor.
    pub label_rotation_degrees: f64,
    /// Completed pitch ladder, clipped to the rounded horizon region.
    pub pitch_ticks: [GameUiHudPitchTick; 7],
}

/// One final pitch ladder segment and its optional numeric label.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudPitchTick {
    /// Final segment endpoints, absent when the mark is outside the clip.
    pub segment: Option<[GameUiChartPoint; 2]>,
    /// Final baseline anchor with horizontal centre alignment, absent for an unlabelled mark.
    pub label_anchor: Option<GameUiChartPoint>,
    /// Absolute pitch in degrees; the horizon has no numeric label.
    pub label_degrees: Option<u32>,
}

/// One completed heading tape mark in logical canvas pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHudHeadingTick {
    /// Tick endpoints.
    pub segment: [GameUiChartPoint; 2],
    /// Tick stroke width in logical pixels.
    pub stroke_width: f64,
    /// Compass label, empty for a minor tick.
    pub label: String,
    /// Centred alphabetic label baseline, present for visible labels.
    pub label_anchor: Option<GameUiChartPoint>,
}

/// Normalized control position for spatial and chart rendering.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Shared ordered menu panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiPanel {
    /// Stable current Web identifier.
    pub id: String,
    /// Shared heading.
    pub title: String,
    /// Explicit placement anchor.
    pub anchor: GameUiAnchor,
    /// Local offset in metres, with rendering forward on negative Z.
    pub local_position: [f64; 3],
    /// Spatial width and height in metres.
    pub size: [f64; 2],
    /// Shared ordered presentation sections.
    pub sections: Vec<GameUiSection>,
    /// Ordered controls and readouts.
    pub controls: Vec<GameUiControl>,
}

pub use birdman_game_app::flight_input::FlightTouchControl;

/// Engine-independent anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiAnchor {
    /// Retained, recenterable menu frame.
    Menu,
    /// Camera-relative smart-glasses display.
    Head,
    /// Aircraft-relative physical instrumentation.
    Cockpit,
    /// Fixed world frame.
    World,
}

/// Shared control group and placement mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiSection {
    /// Stable group identifier.
    pub id: String,
    /// Optional visible group heading.
    pub label: String,
    /// Shared layout mode.
    pub layout: GameUiSectionLayout,
}

/// Group placement strategy, selected in shared Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiSectionLayout {
    /// Vertical flow in declared control order.
    Column,
    /// Responsive choice grid using shared column count.
    ChoiceGrid,
    /// Horizontal action row.
    Row,
}

/// Shared control, including data and admitted semantic action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiControl {
    /// Stable current Web control identifier.
    pub id: String,
    /// Shared visible label.
    pub label: String,
    /// Shared admission flag.
    pub enabled: bool,
    /// Section identifier.
    pub section: String,
    /// Exclusive control data.
    pub content: GameUiControlContent,
}

/// Exclusive shared control representation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiControlContent {
    /// Action or selection button.
    Button {
        /// Typed semantic action.
        action: GameUiAction,
        /// Shared visual hierarchy.
        presentation: GameUiButtonPresentation,
    },
    /// Individual Custom information cue.
    Toggle {
        /// Cue identifier from the existing format model.
        cue: HudCue,
        /// Current Rust-owned visibility.
        value: bool,
    },
    /// Record query cursor.
    Range {
        /// Analysis or playback target.
        target: GameUiRangeTarget,
        /// Current time.
        value: f64,
        /// Shared one-decimal display text.
        display_value: String,
        /// First permitted time.
        minimum: f64,
        /// Last permitted time.
        maximum: f64,
        /// Logical range step.
        step: f64,
    },
    /// Derived readout.
    Status {
        /// Typed dynamic-value provider; static values require no frame patch.
        source: GameUiReadoutSource,
        /// Shared formatted content.
        value: String,
    },
    /// Stable labelled numeric values for the selected record sample.
    Fields {
        /// Shared dynamic-value provider.
        source: GameUiReadoutSource,
        /// Ordered values and units.
        fields: Vec<GameUiNumericField>,
    },
    /// Screen flight pointer region placed in viewport logical pixels.
    FlightTouch {
        /// Stick or absolute pilot-position interaction.
        control: FlightTouchControl,
        /// Completed viewport placement.
        bounds: GameUiRect,
        /// Completed local indicator centre in logical pixels.
        position: [f64; 2],
        /// Acquired local stick origin, absent before touch acquisition.
        origin: Option<[f64; 2]>,
        /// Shared indicator radius in logical pixels.
        radius: f64,
    },
    /// Record-derived chart data, not a separately simulated trajectory.
    Chart {
        /// Plot selection.
        chart: GameUiAnalysisChart,
        /// Fully derived axes, geometry and cursor; adapters render this value.
        plot: Box<GameUiChart>,
    },
}

/// One shared formatted numeric measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiNumericField {
    /// Stable measurement identity.
    pub id: String,
    /// Measurement label.
    pub label: String,
    /// Shared formatted numeric value.
    pub value: String,
    /// Unit displayed next to the value.
    pub unit: String,
}

/// Numeric values for a retained field group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiFieldsValue {
    /// Retained group identity.
    pub id: String,
    /// Shared ordered measurements.
    pub fields: Vec<GameUiNumericField>,
}

/// Shared source for frame-varying readouts, independent of element identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiReadoutSource {
    /// Value retained in the structural view.
    Static,
    /// Remaining countdown duration.
    Countdown,
    /// Current playback time and record duration.
    PlaybackTime,
    /// Terminal distance and current playback time.
    PlaybackSummary,
    /// Record sample at the common analysis cursor.
    AnalysisCursor,
}

/// Complete engine-independent recorded chart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChart {
    /// Internal range relationship used by shared pointer admission.
    #[serde(skip)]
    #[cfg_attr(feature = "schema", schemars(skip))]
    pub seek_target: Option<GameUiRangeTarget>,
    /// Shared chart layout and labels in logical display pixels.
    pub presentation: GameUiChartPresentation,
    /// Horizontal axis label and unit.
    pub x_axis_label: String,
    /// Vertical axis label and unit.
    pub y_axis_label: String,
    /// Horizontal lower bound.
    pub x_minimum: f64,
    /// Horizontal upper bound.
    pub x_maximum: f64,
    /// Vertical lower bound.
    pub y_minimum: f64,
    /// Vertical upper bound.
    pub y_maximum: f64,
    /// Preserves metre-for-metre map scale.
    pub equal_axis_scale: bool,
    /// Ordered curves in logical display pixels, including venue geometry.
    pub series: Vec<GameUiChartSeries>,
    /// Clipped ambient-wind arrows.
    pub vectors: Vec<GameUiChartVector>,
    /// Launch, terminal and venue markers.
    pub markers: Vec<GameUiChartMarker>,
    /// Recorded-time map markers.
    pub time_markers: Vec<GameUiChartMarker>,
    /// Horizontal reference lines.
    pub reference_lines: Vec<GameUiChartReferenceLine>,
    /// Horizontal cursor position in logical pixels; absent for maps.
    pub cursor_x: Option<f64>,
    /// Record-interpolated cursor positions.
    pub cursor_points: Vec<GameUiChartCursorPoint>,
}

/// Measured or initially assumed chart content dimensions in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartExtent {
    /// Content width.
    pub width: f64,
    /// Content height.
    pub height: f64,
}

/// Shared static plot geometry and Web-baseline typography.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartPresentation {
    /// Coordinate extent of this descriptor.
    pub viewport: GameUiChartExtent,
    /// Minimum usable measured dimensions in logical pixels.
    pub measurement_minimum: GameUiChartExtent,
    /// Keeps caption, viewport and legend inside one scrollable chart host.
    pub scroll_overflow: bool,
    /// Plot rectangle ordered left, top, right, bottom.
    pub plot: [f64; 4],
    /// Tick font size in logical pixels.
    pub tick_font_size: f64,
    /// Axis font size in logical pixels.
    pub axis_font_size: f64,
    /// Ordered horizontal ticks.
    pub x_ticks: Vec<GameUiChartTick>,
    /// Ordered vertical ticks.
    pub y_ticks: Vec<GameUiChartTick>,
    /// Static grid segments in descriptor coordinates.
    pub grid_lines: Vec<[GameUiChartPoint; 2]>,
    /// Shared axis stroke colour.
    pub axes_color: String,
    /// Shared grid stroke colour.
    pub grid_color: String,
    /// Grid stroke opacity.
    pub grid_opacity: f64,
    /// Horizontal axis title.
    pub x_axis: GameUiChartText,
    /// Vertical axis title.
    pub y_axis: GameUiChartText,
    /// Marker labels in the same order as the chart markers.
    pub marker_labels: Vec<GameUiChartText>,
    /// Recorded-time labels in the same order as the time markers.
    pub time_marker_labels: Vec<GameUiChartText>,
    /// Reference labels in the same order as the reference lines.
    pub reference_labels: Vec<GameUiChartText>,
    /// Ordered shared legend entries.
    pub legend: Vec<GameUiChartLegend>,
    /// Metre scale and north reference for equal-scale maps.
    pub map_guides: Option<GameUiChartMapGuides>,
    /// Minimum viewport width and height in root-font units.
    pub viewport_minimum_rem: [f64; 2],
    /// Allocated chart height in logical pixels, independent of its measured extent.
    pub target_height_px: f64,
    /// Legend font size in root-font units.
    pub legend_font_rem: f64,
    /// Legend item gap in root-font units.
    pub legend_gap_rem: f64,
    /// Solid legend swatch dimensions in root-font units.
    pub legend_swatch_rem: [f64; 2],
    /// Gap after each legend swatch in root-font units.
    pub legend_swatch_gap_rem: f64,
    /// Gradient legend swatch dimensions in root-font units.
    pub legend_gradient_rem: [f64; 2],
    /// Gradient swatch corner radius in root-font units.
    pub legend_gradient_radius_rem: f64,
}

/// Tick value, placement and already formatted label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartTick {
    /// Axis coordinate.
    pub value: f64,
    /// Increasing-axis fraction between zero and one.
    pub ratio: f64,
    /// Shared text and placement.
    pub text: GameUiChartText,
}

/// Static label placement independent of the rendering engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartText {
    /// Already formatted content.
    pub label: String,
    /// Alphabetic-baseline position in logical pixels; adapters apply actual font metrics.
    pub position: GameUiChartPoint,
    /// Font size in logical pixels.
    pub font_size: f64,
    /// CSS-compatible font weight.
    pub font_weight: u16,
    /// Rotation around the baseline anchor in degrees.
    pub rotation_degrees: f64,
    /// Horizontal text anchor.
    pub anchor: GameUiChartTextAnchor,
    /// Shared text colour.
    pub color: String,
}

/// Horizontal alignment relative to a chart label's baseline anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiChartTextAnchor {
    /// Left edge.
    Start,
    /// Horizontal centre.
    Middle,
    /// Right edge.
    End,
}

/// One ordered legend item with an optional progress gradient.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartLegend {
    /// Shared provider for labels that follow the selected record cursor.
    pub source: GameUiReadoutSource,
    /// Shared label.
    pub label: String,
    /// Solid swatch colour.
    pub color: String,
    /// First and last progress colours when the series uses a gradient.
    pub segment_gradient: Option<[String; 2]>,
}

/// Static map references in descriptor coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartMapGuides {
    /// Represented distance in metres.
    pub scale_distance_m: f64,
    /// Scale bar and its two end caps.
    pub scale_segments: [[GameUiChartPoint; 2]; 3],
    /// Metre scale label.
    pub scale_label: GameUiChartText,
    /// North arrow shaft and its two head segments.
    pub north_segments: [[GameUiChartPoint; 2]; 3],
    /// North label.
    pub north_label: GameUiChartText,
}

/// Two-dimensional display point in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartPoint {
    /// Horizontal component.
    pub x: f64,
    /// Vertical component.
    pub y: f64,
}

/// Record-interpolated position with its shared series colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartCursorPoint {
    /// Position in logical display pixels.
    pub point: GameUiChartPoint,
    /// Colour of the corresponding shared series.
    pub color: String,
}

/// One curve and its shared visual identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartSeries {
    /// Legend label.
    pub label: String,
    /// Shared base colour.
    pub color: String,
    /// Ordered positions in logical display pixels.
    pub points: Vec<GameUiChartPoint>,
    /// Per-segment colours; empty means the base colour.
    pub segment_colors: Vec<String>,
}

/// Clipped vector with simultaneously present or absent endpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartVector {
    /// Legend label.
    pub label: String,
    /// Shared colour.
    pub color: String,
    /// Clipped body and arrowhead lines in logical pixels; empty for zero wind.
    pub segments: Vec<[GameUiChartPoint; 2]>,
}

/// One labelled chart location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartMarker {
    /// Shared label.
    pub label: String,
    /// Shared colour.
    pub color: String,
    /// Position in logical display pixels.
    pub point: GameUiChartPoint,
}

/// One horizontal plot reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiChartReferenceLine {
    /// Vertical position in logical display pixels.
    pub position_y_px: f64,
    /// Shared label.
    pub label: String,
    /// Shared colour.
    pub color: String,
}

/// Range action target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiRangeTarget {
    /// Observational analysis query.
    Analysis,
    /// Rust-owned playback clock.
    Replay,
}

/// Shared button role and selection state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiButtonPresentation {
    /// Navigation or command.
    Action {
        /// Primary or secondary emphasis.
        emphasis: GameUiEmphasis,
    },
    /// Explicit option in a named choice group.
    Choice {
        /// Stable group identifier.
        group: String,
        /// Group heading.
        group_label: String,
        /// Current selected option.
        selected: bool,
        /// Shared option explanation.
        description: String,
    },
    /// Technical-information disclosure.
    Disclosure {
        /// Current disclosure state.
        expanded: bool,
    },
}

/// Shared action emphasis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiEmphasis {
    /// Principal forward operation.
    Primary,
    /// Back or auxiliary operation.
    Secondary,
    /// Existing ordinary action styling.
    Normal,
}

/// Responsive shell selected using the Web baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiLayout {
    /// Shared shell placement.
    pub shell: GameUiShell,
    /// Shell width in logical pixels.
    pub width_px: f64,
    /// Maximum reachable scroll viewport height.
    pub maximum_height_px: f64,
    /// Shared shell padding.
    pub padding_px: f64,
    /// Shared control gap.
    pub gap_px: f64,
    /// Responsive choice grid count.
    pub choice_columns: u32,
    /// Small-screen branch selected below the Web 600px breakpoint.
    pub compact: bool,
    /// Vertical scrolling is admitted for the shell.
    pub scroll_y: bool,
    /// Horizontal overflow is clipped.
    pub clip_x: bool,
    /// Input-transparent forward view outside explicit controls.
    pub passthrough: bool,
}

/// Shared viewport-relative placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GameUiShell {
    /// Centered scrollable menu.
    Center,
    /// Bottom-right flight action strip.
    FlightBar,
    /// Top-right pause settings.
    PauseSettings,
    /// Centered analysis card with shared chart-height allocation.
    Analysis,
    /// Top-right compact scenic replay menu.
    Cinematic,
}

/// Engine-independent colors, opacity and minimum hit dimensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiTheme {
    /// Main text color.
    pub text: String,
    /// Secondary text color.
    pub muted_text: String,
    /// Panel background color.
    pub panel_background: String,
    /// Centered menu opacity.
    pub panel_opacity: f64,
    /// Head-relative HUD background opacity, preserving forward visibility.
    pub hud_opacity: f64,
    /// Ordinary action fill.
    pub normal_background: String,
    /// Primary or selected action fill.
    pub primary_background: String,
    /// Unselected choice fill.
    pub choice_background: String,
    /// Keyboard focus outline.
    pub focus_outline: String,
    /// Ordinary button minimum height.
    pub button_min_height_px: f64,
    /// Principal button minimum height.
    pub primary_min_height_px: f64,
}
