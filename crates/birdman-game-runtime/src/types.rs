use super::scene::ApplicationSceneProjection;
use super::surface::{ApplicationSpatialProjection, ApplicationSurfaceObservation};
use super::{
    ApplicationGraphicsObservation, ApplicationGraphicsProjection, ApplicationGraphicsTarget,
    ApplicationRendererObservation, ApplicationResourceCommit, DisplayCompletion, DisplayRequest,
    PhoneMotionObservation, PhoneOrientationObservation, PhoneSensorApiObservation,
};
use birdman_game_app::application::{ApplicationError, FlightFrameError, XrVisibility};
use birdman_game_app::environment_snapshot::{EnvironmentIdentity, EnvironmentSnapshotError};
use birdman_game_core::GameSessionError;
use birdman_game_presentation::camera::Pose;
use birdman_game_presentation::game_ui::{
    GameUiChartExtent, GameUiControlInput, GameUiError, GameUiFrame, GameUiView, GameUiViewport,
};
use birdman_game_presentation::xr::XrViewerObservation;
use serde::{Deserialize, Serialize};

/// Capabilities implemented by the platform's I/O and drawing adapters.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationCapabilities {
    /// Runtime can create a tracked head-mounted display.
    pub hmd_available: bool,
    /// Raw orientation API and permission-method availability, absent without the required browser APIs.
    pub phone_sensors: Option<PhoneSensorApiObservation>,
    /// Drawing adapter supports record plots.
    pub analysis_available: bool,
    /// Drawing adapter supports recorded playback.
    pub replay_available: bool,
    /// Platform provides persistent record I/O.
    pub stored_records_available: bool,
    /// Platform can save an exported file.
    pub flight_log_available: bool,
}

/// External metrics and API availability, without derived UI state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationPlatformObservation {
    /// Explicit GPU diagnostic demand, independent of automatic quality selection.
    pub gpu_timing_requested: bool,
    /// Live renderer and its startup resources; None retires that lifetime and replacement uses a new creation identity.
    pub renderer: Option<ApplicationRendererObservation>,
    /// Actual loaded surface font and shaping locale, absent while font I/O is pending.
    pub surface: Option<ApplicationSurfaceObservation>,
    /// Physical logical-pixel viewport and root font size.
    pub viewport: GameUiViewport,
    /// Actual API and drawing support.
    pub capabilities: ApplicationCapabilities,
}

/// Raw stage of a platform chart pointer event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApplicationChartPointerPhase {
    /// Pointer button pressed.
    Down,
    /// Pointer position changed.
    Move,
    /// Pointer button released.
    Up,
    /// Platform cancelled this pointer.
    Cancel,
    /// Platform lost its capture handle.
    LostCapture,
}

/// Completion state of the display and scene resources needed for launch.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationResourceObservation {
    /// Atomic binding failed and the previous binding could not be restored.
    BindingLost {
        /// Candidate belonging to the failed commit request.
        resource_id: u32,
        /// Original drawing and rollback diagnostic.
        cause: String,
    },
    /// GPU resources have been created as an unbound candidate.
    Prepared {
        /// New opaque GPU resource identity.
        resource_id: u32,
    },
    /// A requested atomic binding completed; older bindings remain owned until explicitly released.
    Committed {
        /// Exact candidate identity from the commit request.
        resource_id: u32,
        /// Actual framebuffer and quality after the authorized binding.
        actual: ApplicationResourceCommit,
    },
    /// Cancellation completed and no further asset reads will occur for this request.
    Cancelled,
    /// Creation or atomic binding failed while the previous binding remains owned and unchanged.
    Failed {
        #[doc = "Original preparation failure."]
        cause: String,
    },
}

/// One external observation delivered to the shared application owner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationObservation {
    /// Monotonic receipt time in milliseconds; source clocks are converted to this origin at I/O.
    pub observed_ms: f64,
    /// Raw frame, event or I/O completion received at this time.
    pub input: ApplicationInput,
}

/// Payload of one externally observed event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationXrFrameObservation {
    /// Unmodified same-reference head and eye observations.
    pub observation: XrViewerObservation,
    /// Runtime visibility observed with this frame.
    pub visibility: XrVisibility,
    /// Runtime input-focus observation.
    pub input_focused: bool,
}

/// Unmodified tracked controller pose in the active runtime reference space.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationPointerObservation {
    /// Platform input-source identity, retired when that source disconnects.
    pub source_id: u32,
    /// Runtime target-ray-space pose in tracking coordinates.
    #[serde(deserialize_with = "birdman_game_presentation::xr::deserialize_xr_pose")]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "birdman_game_presentation::xr::XrObservedPose")
    )]
    pub tracking_from_ray: Pose,
}

/// Same-frame spatial input sampled by the platform I/O adapter.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationSpatialObservation {
    /// Identity of the spatial projection actually submitted by the drawing adapter.
    pub presented_id: Option<u32>,
    /// Same-frame raw XR sources, including temporarily untracked controllers.
    pub sources: Vec<birdman_game_presentation::xr::XrInputSourceObservation>,
    /// Connected standard gamepads, before device selection or dead-zone filtering.
    pub gamepads: Vec<super::ApplicationGamepadObservation>,
}

/// Payload of one externally observed event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationInput {
    /// Terminal result of attaching the exact requested sensor subscription.
    PhoneSensorsStarted {
        #[doc = "Owner-issued sensor subscription identity."]
        resource_id: u32,
        #[doc = "Original subscription failure, absent after successful attachment."]
        failure: Option<String>,
    },
    /// Terminal result of detaching the exact requested sensor subscription.
    PhoneSensorsStopped {
        #[doc = "Owner-issued sensor subscription identity."]
        resource_id: u32,
        #[doc = "Original cleanup failure, absent after successful detachment."]
        failure: Option<String>,
    },
    /// A drawing API operation failed independently of a graphics-context loss event.
    RendererFailed {
        #[doc = "Actual renderer lifetime reporting the failure."]
        renderer_resource_id: u32,
        #[doc = "Original platform diagnostic."]
        cause: String,
    },
    /// Raw document or native-window input focus, independent of page visibility.
    DocumentFocus {
        #[doc = "Whether the platform currently grants keyboard/pointer focus."]
        focused: bool,
    },
    /// The actual platform display ended independently of a requested stop.
    DisplayEnded {
        #[doc = "Ended session or sensor subscription identity."]
        resource_id: u32,
        #[doc = "Original runtime reason for an unsolicited end, when supplied."]
        #[serde(default)]
        cause: Option<String>,
    },
    /// Stops new application work and drains already issued I/O.
    Dispose,
    /// Raw browser pagehide observation; persisted documents suspend rather than dispose.
    PageHide {
        #[doc = "Whether the browser retained the document in its history cache."]
        persisted: bool,
    },
    /// A requested graphics context release reached its terminal result.
    RendererReleased {
        #[doc = "Owner-issued renderer identity."]
        renderer_resource_id: u32,
        #[doc = "Original API diagnostic on failure."]
        failure: Option<String>,
    },
    /// A requested display-resource release reached its terminal result.
    DisplayReleased {
        #[doc = "Actual session or sensor handle identity."]
        resource_id: u32,
        #[doc = "Original API diagnostic on failure."]
        failure: Option<String>,
    },
    /// Requested cancellation completed after its actual storage operation became terminal.
    RecordIoCancelled {
        #[doc = "Original storage operation identity."]
        request_id: u32,
        #[doc = "Original API diagnostic on failure."]
        failure: Option<String>,
    },
    /// Completion of the current owner-issued graphics context creation.
    RendererCreated {
        /// Initialization request being completed.
        request_id: u32,
        /// Raw graphics API result.
        result: super::ApplicationRendererCreation,
    },
    /// Raw results from a bounded atomic record preview.
    RecordPreviewObserved {
        /// Owner-issued read transaction identity.
        request_id: u32,
        /// Unmodified transaction or request result.
        observation: birdman_game_app::record_persistence::RecordPreviewObservation,
    },
    /// Observable file creation or submission result, rather than user save completion.
    DownloadCompleted {
        /// Owner-issued file operation identity.
        request_id: u32,
        /// Original platform failure, or None after successful submission.
        failure: Option<String>,
    },
    /// Raw completion of a specific record read.
    RecordLoaded {
        /// Owner-issued read identity.
        request_id: u32,
        /// Raw storage result.
        completion: super::ApplicationRecordLoadCompletion,
    },
    /// An atomic record transaction is active and can accept immediate requests.
    RecordSaveOpened {
        /// Owner-issued transaction identity.
        request_id: u32,
        /// Raw platform wall clock at transaction creation.
        unix_milliseconds: i64,
    },
    /// One raw storage result delivered in the transaction's active callback.
    RecordSaveObserved {
        /// Owner-issued transaction identity.
        request_id: u32,
        /// Unmodified operation or transaction result.
        observation: birdman_game_app::record_persistence::RecordWriteObservation,
    },
    /// Raw GPU or graphics-context observation for one renderer lifetime.
    Graphics {
        /// Renderer that produced the observation.
        renderer_resource_id: u32,
        /// Original hardware or API result.
        observation: ApplicationGraphicsObservation,
    },
    /// Result of releasing an explicitly retired GPU resource.
    ResourcesReleased {
        /// Identity from the release request.
        resource_id: u32,
        /// Original API diagnostic when release failed.
        failure: Option<String>,
    },
    /// Explicit trigger observation targeting the last submitted spatial painting.
    XrSelect {
        /// Active display resource that emitted the trigger.
        display_resource_id: Option<u32>,
        /// Opaque identity returned with the submitted spatial projection.
        presentation_id: u32,
        /// Raw controller pose sampled for the trigger.
        pointer: ApplicationPointerObservation,
        /// Source event time on the common monotonic clock.
        timestamp_ms: f64,
    },
    /// Raw passive reference-space reset supplied by the active HMD runtime.
    XrReferenceReset {
        /// Runtime resource that emitted the reset.
        resource_id: u32,
        /// New reference coordinates expressed in the previous tracking frame.
        #[serde(
            default,
            deserialize_with = "birdman_game_presentation::xr::deserialize_xr_optional_pose"
        )]
        #[cfg_attr(
            feature = "schema",
            schemars(with = "Option<birdman_game_presentation::xr::XrObservedPose>")
        )]
        previous_from_new: Option<Pose>,
    },
    /// Raw pointer movement and the currently observed standard buttons.
    PointerMotion {
        /// Display resource receiving the pointer event.
        display_resource_id: Option<u32>,
        /// Movement along the two screen axes as reported by the platform.
        delta_px: [f64; 2],
        /// Standard button bits: primary=1, secondary=2, auxiliary=4, back=8, forward=16.
        buttons: u16,
    },
    /// Runtime visibility or focus changed between frames.
    XrSessionState {
        /// Runtime resource that emitted the event.
        resource_id: u32,
        /// Raw runtime visibility.
        visibility: XrVisibility,
        /// Raw runtime input-focus state.
        input_focused: bool,
    },
    /// Document or native window visibility changed.
    PageVisibility {
        /// Whether the platform currently presents the application window.
        visible: bool,
    },
    /// Raw orientation callback for the identified sensor subscription.
    PhoneOrientation {
        /// Platform resource owning the callback.
        resource_id: u32,
        /// Unmodified angles and source metadata.
        observation: PhoneOrientationObservation,
    },
    /// Raw motion callback for the identified sensor subscription.
    PhoneMotion {
        /// Platform resource owning the callback.
        resource_id: u32,
        /// Unmodified acceleration components.
        observation: PhoneMotionObservation,
    },
    /// Browser screen rotation changed for the active sensor subscription.
    PhoneScreenRotation {
        /// Platform resource owning the callback.
        resource_id: u32,
        /// Observed screen angle in degrees.
        angle_degrees: Option<f64>,
    },
    /// A timer requested by this owner reached its deadline.
    Wake {
        /// Identity of the scheduled wake request.
        request_id: u32,
    },
    /// Same-frame clock and raw flight devices.
    Frame {
        /// Active display that supplied the callback; absent for the initial Screen resource.
        display_resource_id: Option<u32>,
        /// Monotonic presentation timestamp in milliseconds.
        timestamp_ms: f64,
        /// Unfiltered same-frame flight device samples.
        gamepads: Vec<super::ApplicationGamepadObservation>,
        /// Same-frame HMD poses and runtime observations, when supplied by the active runtime.
        xr: Option<ApplicationXrFrameObservation>,
        /// Unfiltered spatial pointer and standard gamepad samples.
        spatial: ApplicationSpatialObservation,
    },
    /// A widget event from the last displayed structural view.
    Control {
        /// Revision of the structure that received the event.
        view_revision: u32,
        /// Observed widget interaction.
        input: GameUiControlInput,
    },
    /// Actual measured viewport belonging to a displayed chart.
    ChartMeasured {
        /// Revision displayed by the measured element.
        view_revision: u32,
        /// Measured chart identity.
        control_id: String,
        /// Actual logical-pixel extent.
        extent: GameUiChartExtent,
    },
    /// Raw pointer event and actual chart rectangle in the same coordinate system.
    ChartPointer {
        /// Revision displayed by the event target.
        view_revision: u32,
        /// Chart element identity.
        control_id: String,
        /// Platform pointer identity.
        pointer_id: i32,
        /// Raw event stage.
        phase: ApplicationChartPointerPhase,
        /// Changed button, using the platform pointer-event numbering.
        button: i16,
        /// Currently held button bitset.
        buttons: u16,
        /// Platform primary-pointer observation.
        is_primary: bool,
        /// Pointer coordinates in logical pixels.
        position: [f64; 2],
        /// Actual viewport rectangle: left, top, width, height.
        rect: [f64; 4],
    },
    /// A raw keyboard binding transition.
    Keyboard {
        /// Physical key identifier in the DOM KeyboardEvent.code vocabulary.
        code: String,
        /// Whether the key is pressed.
        pressed: bool,
        /// Whether this press repeats an existing hold.
        repeat: bool,
    },
    /// Confirmed viewport, capabilities and renderer resource observations.
    Platform {
        #[doc = "Viewport and API capabilities."]
        observation: ApplicationPlatformObservation,
    },
    /// Completion of the resources required by the current display.
    Resources {
        /// Identity of the resource preparation request being completed.
        request_id: u32,
        #[doc = "Completion state of the required drawing resources."]
        observation: ApplicationResourceObservation,
    },
    /// Completion of a specific display I/O request.
    DisplayCompleted {
        /// Identity issued by this owner for the completed I/O operation.
        request_id: u32,
        /// Actual outcome and acquired resource identity.
        completion: DisplayCompletion,
    },
}

/// Ordered platform operations produced by the application owner.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationRequest {
    /// Subscribe to raw phone observations without changing the active display.
    StartPhoneSensors {
        #[doc = "Owner-issued sensor subscription identity."]
        resource_id: u32,
    },
    /// Detach the exact raw sensor subscription and report its terminal result.
    StopPhoneSensors {
        #[doc = "Owner-issued sensor subscription identity."]
        resource_id: u32,
    },
    /// Aborts the exact record transaction or pending open and reports its terminal result.
    CancelRecordIo {
        #[doc = "Original storage operation identity."]
        request_id: u32,
    },
    /// Creates the platform graphics context and reports its actual startup resources.
    CreateRenderer {
        #[doc = "Owner-issued initialization identity."]
        request_id: u32,
    },
    /// Releases a graphics context acquired by an obsolete initialization request.
    ReleaseRenderer {
        #[doc = "Acquired graphics context identity."]
        renderer_resource_id: u32,
    },
    /// Reloads the application document or restarts the native process.
    ReloadApplication,
    /// Cancels an obsolete preparation; its CPU buffers remain retained until completion.
    CancelResources {
        /// Preparation identity to cancel.
        request_id: u32,
    },
    /// Atomically binds a prepared candidate while retaining the previous GPU resources.
    CommitResources {
        /// Owner-authorized preparation identity.
        request_id: u32,
        /// Exact candidate GPU identity.
        resource_id: u32,
    },
    /// Disposes a retired or rejected candidate, independently of the current binding.
    ReleaseResources {
        /// Opaque GPU resource identity to release.
        resource_id: u32,
    },
    /// Opens one atomic read transaction with a descending metadata cursor and same-key records.
    BeginRecordPreview {
        /// Owner-issued read transaction identity.
        request_id: u32,
        /// Exact maximum number of cursor rows to retrieve.
        limit: usize,
    },
    /// Bounded index reads to enqueue synchronously in the active read callback.
    RecordPreview {
        /// Owner-issued read transaction identity.
        request_id: u32,
        /// Ordered primitive reads selected by shared Rust.
        requests: Vec<birdman_game_app::record_persistence::RecordPreviewRequest>,
    },
    /// Opens one atomic record transaction; the adapter immediately reports its creation.
    BeginRecordSave {
        /// Owner-issued transaction identity.
        request_id: u32,
    },
    /// Primitive storage operations to enqueue synchronously in the active callback.
    RecordWrite {
        /// Owner-issued transaction identity.
        request_id: u32,
        /// Ordered operations selected by the shared application policy.
        requests: Vec<birdman_game_app::record_persistence::RecordWriteRequest>,
    },
    /// Notify this owner when its deadline is reached.
    ScheduleWake {
        /// Owner-issued timer identity.
        request_id: u32,
        /// Deadline on the shared receipt clock, in milliseconds.
        at_ms: f64,
    },
    /// Cancel a timer whose observation is no longer needed.
    CancelWake {
        /// Identity of the previously scheduled timer.
        request_id: u32,
    },
    /// Prepare scene resources for one confirmed display and environment.
    PrepareResources {
        /// Revision-scoped CPU drawing data; image bytes use the common buffer channel.
        assets: Box<super::ApplicationEnvironmentAssets>,
        /// Complete desired renderer configuration to prepare and bind atomically.
        graphics: ApplicationGraphicsTarget,
        /// Owner-issued identity to echo in the completion.
        request_id: u32,
        /// Canonical environment to render.
        environment: EnvironmentIdentity,
        /// Active platform display resource receiving the scene.
        display_resource_id: Option<u32>,
    },
    /// A single permission, resource stop or resource start operation.
    Display {
        #[doc = "One primitive platform display operation."]
        request: DisplayRequest,
    },
    /// Reads one raw record envelope by key.
    ReadRecord {
        /// Owner-issued read identity.
        request_id: u32,
        /// Selected persistence key.
        record_id: u32,
    },
    /// Saves the exact bytes prepared by the shared owner.
    DownloadFile {
        /// Owner-issued file operation identity.
        request_id: u32,
        /// Suggested file name.
        file_name: String,
        /// Media type for the file bytes.
        mime_type: String,
        /// Exact output bytes, including preserved original archive JSON.
        bytes: Vec<u8>,
    },
}

/// Structure is transferred only when its shared revision changes.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationUiProjection {
    /// Complete replacement for the retained visible structure.
    View {
        #[doc = "New structure carrying its shared revision."]
        view: Box<GameUiView>,
    },
    /// Lightweight values for the retained structure.
    Frame {
        #[doc = "Dynamic values for the retained structural revision."]
        frame: Box<GameUiFrame>,
    },
}

/// One shared update's rendering and I/O work.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationDrawingPresentation {
    /// Shared frame-rate text used by both Screen and Head HUD drawing.
    pub frame_rate_label: String,
    /// Confirmed active display, absent during an unbound transition.
    pub mode: Option<birdman_game_presentation::game_ui::GameUiPresentationMode>,
    /// Submit the shared DOM/native screen menu.
    pub screen_ui_visible: bool,
    /// Submit the shared screen-anchored HUD.
    pub screen_hud_visible: bool,
    /// Submit the prepared 3D scene for this display.
    pub scene_visible: bool,
}

/// One shared update's rendering and I/O work.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationDrawingOutput {
    /// Final visibility decisions for drawing adapters.
    pub presentation: ApplicationDrawingPresentation,
    /// Confirmed graphics resources and optional query for this frame.
    pub graphics: ApplicationGraphicsProjection,
    /// Aircraft and camera poses ready for direct drawing-engine transfer.
    pub scene: ApplicationSceneProjection,
    /// Spatial painting for an admitted frame; other notifications retain submitted painting.
    pub spatial: Option<ApplicationSpatialProjection>,
    /// Structural replacement or dynamic UI values.
    pub ui: ApplicationUiProjection,
}

/// One update's I/O work and optional drawing submission.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationOutput {
    /// Absent for synchronous I/O continuations that leave the visible state unchanged.
    pub drawing: Option<ApplicationDrawingOutput>,
    /// The executable owner can be released only after this becomes Drained.
    pub lifecycle: super::ApplicationLifecycle,
    /// Technical failure details for the platform diagnostic sink, separate from UI wording.
    pub diagnostic: Option<String>,
    /// Ordered I/O requests to execute at the platform boundary.
    pub requests: Vec<ApplicationRequest>,
    /// Whether the observed key belongs to the active flight input.
    pub keyboard_consumed: bool,
}

/// Finite failures from the same domain and presentation implementations.
#[derive(Debug)]
pub enum ApplicationRuntimeError {
    /// Shared immutable drawing-resource generation failed.
    Assets(super::ApplicationAssetError),
    /// The canonical finalized result could not be exported.
    RecordExport(birdman_game_app::record_archive::RecordExportError),
    /// A record document failed the shared external-data contract.
    RecordFormat(birdman_game_format::FlightRecordFormatError),
    /// Shared rendering policy rejected a raw observation or resource completion.
    Graphics(birdman_game_presentation::screen_render_auto::ScreenRenderAutoError),
    /// Spatial input or prepared geometry could not be admitted.
    SpatialInput(&'static str),
    /// The submitted spatial presentation identity space has been consumed.
    PresentationExhausted,
    /// The live or prepared display projection failed.
    RenderPose(birdman_game_presentation::render_pose::RenderPoseProjectionError),
    /// The recorded display camera could not be projected.
    ReplayCamera(birdman_game_presentation::replay_camera::ReplayCameraError),
    /// The registered exhibition venue is invalid.
    Venue(birdman_game_app::LaunchVenueError),
    /// Head or tracking-space composition failed.
    Camera(birdman_game_presentation::camera::CameraError),
    /// Canonical display environment could not be resolved.
    Environment(EnvironmentSnapshotError),
    /// Clock is negative, non-finite or moves backwards.
    InvalidTimestamp,
    /// Canonical application operation failed.
    Application(ApplicationError),
    /// Canonical session progression failed.
    Session(GameSessionError),
    /// Physical frame driver failed.
    Flight(FlightFrameError),
    /// Shared UI observation or action admission failed.
    Ui(GameUiError),
    /// The request identity space has been consumed.
    RequestExhausted,
}
