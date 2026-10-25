/* Generated from birdman-game-runtime. */
/**
 * Font slant selected for measurement.
 */
export type MenuFontStyle = "normal" | "italic";
/**
 * Payload of one externally observed event.
 */
export type ApplicationInput = {
    /**
     * Original subscription failure, absent after successful attachment.
     */
    readonly failure?: string | null;
    readonly kind: "phone_sensors_started";
    /**
     * Owner-issued sensor subscription identity.
     */
    readonly resource_id: number;
} | {
    /**
     * Original cleanup failure, absent after successful detachment.
     */
    readonly failure?: string | null;
    readonly kind: "phone_sensors_stopped";
    /**
     * Owner-issued sensor subscription identity.
     */
    readonly resource_id: number;
} | {
    /**
     * Original platform diagnostic.
     */
    readonly cause: string;
    readonly kind: "renderer_failed";
    /**
     * Actual renderer lifetime reporting the failure.
     */
    readonly renderer_resource_id: number;
} | {
    /**
     * Whether the platform currently grants keyboard/pointer focus.
     */
    readonly focused: boolean;
    readonly kind: "document_focus";
} | {
    readonly kind: "display_ended";
    /**
     * Ended session or sensor subscription identity.
     */
    readonly resource_id: number;
} | {
    readonly kind: "dispose";
} | {
    readonly kind: "page_hide";
    /**
     * Whether the browser retained the document in its history cache.
     */
    readonly persisted: boolean;
} | {
    /**
     * Original API diagnostic on failure.
     */
    readonly failure?: string | null;
    readonly kind: "renderer_released";
    /**
     * Owner-issued renderer identity.
     */
    readonly renderer_resource_id: number;
} | {
    /**
     * Original API diagnostic on failure.
     */
    readonly failure?: string | null;
    readonly kind: "display_released";
    /**
     * Actual session or sensor handle identity.
     */
    readonly resource_id: number;
} | {
    /**
     * Original API diagnostic on failure.
     */
    readonly failure?: string | null;
    readonly kind: "record_io_cancelled";
    /**
     * Original storage operation identity.
     */
    readonly request_id: number;
} | {
    readonly kind: "renderer_created";
    /**
     * Initialization request being completed.
     */
    readonly request_id: number;
    /**
     * Raw graphics API result.
     */
    readonly result: ApplicationRendererCreation;
} | {
    readonly kind: "record_preview_observed";
    /**
     * Unmodified transaction or request result.
     */
    readonly observation: RecordPreviewObservation;
    /**
     * Owner-issued read transaction identity.
     */
    readonly request_id: number;
} | {
    /**
     * Original platform failure, or None after successful submission.
     */
    readonly failure?: string | null;
    readonly kind: "download_completed";
    /**
     * Owner-issued file operation identity.
     */
    readonly request_id: number;
} | {
    /**
     * Raw storage result.
     */
    readonly completion: ApplicationRecordLoadCompletion;
    readonly kind: "record_loaded";
    /**
     * Owner-issued read identity.
     */
    readonly request_id: number;
} | {
    readonly kind: "record_save_opened";
    /**
     * Owner-issued transaction identity.
     */
    readonly request_id: number;
    /**
     * Raw platform wall clock at transaction creation.
     */
    readonly unix_milliseconds: number;
} | {
    readonly kind: "record_save_observed";
    /**
     * Unmodified operation or transaction result.
     */
    readonly observation: RecordWriteObservation;
    /**
     * Owner-issued transaction identity.
     */
    readonly request_id: number;
} | {
    readonly kind: "graphics";
    /**
     * Original hardware or API result.
     */
    readonly observation: ApplicationGraphicsObservation;
    /**
     * Renderer that produced the observation.
     */
    readonly renderer_resource_id: number;
} | {
    /**
     * Original API diagnostic when release failed.
     */
    readonly failure?: string | null;
    readonly kind: "resources_released";
    /**
     * Identity from the release request.
     */
    readonly resource_id: number;
} | {
    /**
     * Active display resource that emitted the trigger.
     */
    readonly display_resource_id?: number | null;
    readonly kind: "xr_select";
    /**
     * Raw controller pose sampled for the trigger.
     */
    readonly pointer: ApplicationPointerObservation;
    /**
     * Opaque identity returned with the submitted spatial projection.
     */
    readonly presentation_id: number;
    /**
     * Source event time on the common monotonic clock.
     */
    readonly timestamp_ms: number;
} | {
    readonly kind: "xr_reference_reset";
    /**
     * New reference coordinates expressed in the previous tracking frame.
     */
    readonly previous_from_new?: XrObservedPose | null;
    /**
     * Runtime resource that emitted the reset.
     */
    readonly resource_id: number;
} | {
    /**
     * Standard button bits: primary=1, secondary=2, auxiliary=4, back=8, forward=16.
     */
    readonly buttons: number;
    /**
     * Movement along the two screen axes as reported by the platform.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly delta_px: readonly [
        number,
        number
    ];
    /**
     * Display resource receiving the pointer event.
     */
    readonly display_resource_id?: number | null;
    readonly kind: "pointer_motion";
} | {
    /**
     * Raw runtime input-focus state.
     */
    readonly input_focused: boolean;
    readonly kind: "xr_session_state";
    /**
     * Runtime resource that emitted the event.
     */
    readonly resource_id: number;
    /**
     * Raw runtime visibility.
     */
    readonly visibility: XrVisibility;
} | {
    readonly kind: "page_visibility";
    /**
     * Whether the platform currently presents the application window.
     */
    readonly visible: boolean;
} | {
    readonly kind: "phone_orientation";
    /**
     * Unmodified angles and source metadata.
     */
    readonly observation: PhoneOrientationObservation;
    /**
     * Platform resource owning the callback.
     */
    readonly resource_id: number;
} | {
    readonly kind: "phone_motion";
    /**
     * Unmodified acceleration components.
     */
    readonly observation: PhoneMotionObservation;
    /**
     * Platform resource owning the callback.
     */
    readonly resource_id: number;
} | {
    /**
     * Observed screen angle in degrees.
     */
    readonly angle_degrees?: number | null;
    readonly kind: "phone_screen_rotation";
    /**
     * Platform resource owning the callback.
     */
    readonly resource_id: number;
} | {
    readonly kind: "wake";
    /**
     * Identity of the scheduled wake request.
     */
    readonly request_id: number;
} | {
    /**
     * Active display that supplied the callback; absent for the initial Screen resource.
     */
    readonly display_resource_id?: number | null;
    /**
     * Unfiltered same-frame flight device samples.
     */
    readonly gamepads: readonly ApplicationGamepadObservation[];
    readonly kind: "frame";
    /**
     * Unfiltered spatial pointer and standard gamepad samples.
     */
    readonly spatial: ApplicationSpatialObservation;
    /**
     * Monotonic presentation timestamp in milliseconds.
     */
    readonly timestamp_ms: number;
    /**
     * Same-frame HMD poses and runtime observations, when supplied by the active runtime.
     */
    readonly xr?: ApplicationXrFrameObservation | null;
} | {
    /**
     * Observed widget interaction.
     */
    readonly input: GameUiControlInput;
    readonly kind: "control";
    /**
     * Revision of the structure that received the event.
     */
    readonly view_revision: number;
} | {
    /**
     * Measured chart identity.
     */
    readonly control_id: string;
    /**
     * Actual logical-pixel extent.
     */
    readonly extent: GameUiChartExtent;
    readonly kind: "chart_measured";
    /**
     * Revision displayed by the measured element.
     */
    readonly view_revision: number;
} | {
    /**
     * Changed button, using the platform pointer-event numbering.
     */
    readonly button: number;
    /**
     * Currently held button bitset.
     */
    readonly buttons: number;
    /**
     * Chart element identity.
     */
    readonly control_id: string;
    /**
     * Platform primary-pointer observation.
     */
    readonly is_primary: boolean;
    readonly kind: "chart_pointer";
    /**
     * Raw event stage.
     */
    readonly phase: ApplicationChartPointerPhase;
    /**
     * Platform pointer identity.
     */
    readonly pointer_id: number;
    /**
     * Pointer coordinates in logical pixels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly position: readonly [
        number,
        number
    ];
    /**
     * Actual viewport rectangle: left, top, width, height.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly rect: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Revision displayed by the event target.
     */
    readonly view_revision: number;
} | {
    /**
     * Physical key identifier in the DOM KeyboardEvent.code vocabulary.
     */
    readonly code: string;
    readonly kind: "keyboard";
    /**
     * Whether the key is pressed.
     */
    readonly pressed: boolean;
    /**
     * Whether this press repeats an existing hold.
     */
    readonly repeat: boolean;
} | {
    readonly kind: "platform";
    /**
     * Viewport and API capabilities.
     */
    readonly observation: ApplicationPlatformObservation;
} | {
    readonly kind: "resources";
    /**
     * Completion state of the required drawing resources.
     */
    readonly observation: ApplicationResourceObservation;
    /**
     * Identity of the resource preparation request being completed.
     */
    readonly request_id: number;
} | {
    /**
     * Actual outcome and acquired resource identity.
     */
    readonly completion: DisplayCompletion;
    readonly kind: "display_completed";
    /**
     * Identity issued by this owner for the completed I/O operation.
     */
    readonly request_id: number;
};
/**
 * Completion of an owner-requested graphics context creation.
 */
export type ApplicationRendererCreation = {
    readonly kind: "ready";
    /**
     * Actual acquired renderer.
     */
    readonly renderer: ApplicationRendererObservation;
} | {
    /**
     * Original API diagnostic.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Actual renderer creation and drawing-buffer observations.
 */
export type ApplicationGpuTimingCapability = {
    readonly kind: "available";
} | {
    /**
     * Original capability diagnostic.
     */
    readonly cause: string;
    readonly kind: "unsupported";
};
/**
 * Supported manual Screen rendering scales, with full resolution as the default.
 */
export type ScreenRenderScale = "100" | "75" | "50";
/**
 * Directional shadow-map resolutions supported by both Screen adapters.
 */
export type ScreenShadowMap = "full" | "half" | "quarter";
/**
 * Shared non-physical water-detail selection.
 */
export type WaterQuality = "low" | "medium" | "high";
/**
 * Raw results from one bounded record preview transaction.
 */
export type RecordPreviewObservation = {
    readonly kind: "rows";
    /**
     * Rows in descending primary-key order.
     */
    readonly rows: readonly RecordPreviewRow[];
} | {
    readonly kind: "bests";
    /**
     * Exactly one raw value per requested key, in request order.
     */
    readonly values: readonly unknown[];
} | {
    readonly kind: "committed";
} | {
    /**
     * Original platform failure.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Raw result of a platform record read.
 */
export type ApplicationRecordLoadCompletion = {
    readonly kind: "found";
    /**
     * Raw record value from storage.
     */
    readonly value: unknown;
} | {
    readonly kind: "missing";
} | {
    /**
     * Platform I/O error.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Raw request results delivered synchronously within an active transaction callback.
 */
export type RecordWriteObservation = {
    readonly kind: "best";
    /**
     * Unmodified stored index.
     */
    readonly value: unknown;
} | {
    readonly kind: "record";
    /**
     * Unmodified stored record.
     */
    readonly value: unknown;
} | {
    /**
     * Raw cursor primary key; null at the end.
     */
    readonly key: unknown;
    readonly kind: "scanned";
    /**
     * Raw cursor value; null at the end.
     */
    readonly value: unknown;
} | {
    /**
     * Raw generated key.
     */
    readonly key: unknown;
    readonly kind: "inserted";
} | {
    readonly kind: "metadata_written";
} | {
    readonly kind: "best_written";
} | {
    readonly kind: "committed";
} | {
    /**
     * Original I/O failure.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Raw graphics API observations.
 */
export type ApplicationGraphicsObservation = {
    /**
     * Measured command-span duration in milliseconds.
     */
    readonly gpu_work_ms: number | null;
    readonly kind: "gpu_completed";
    /**
     * Unmodified query tag.
     */
    readonly query: ApplicationGpuQuery;
} | {
    /**
     * Original capability diagnostic.
     */
    readonly cause: string;
    readonly kind: "unsupported";
} | {
    readonly kind: "disjoint";
} | {
    readonly kind: "context_lost";
};
/**
 * Runtime visibility observed independently of input focus and tracking.
 */
export type XrVisibility = "visible" | "blurred" | "hidden";
/**
 * Browser event source, before selection of a gravity reference.
 */
export type PhoneOrientationSource = "orientation" | "absolute_orientation";
/**
 * Origin of the same-frame platform view observations.
 */
export type XrViewerSource = "configured" | "runtime-derived";
/**
 * Homogeneous depth-clip convention of the rendering API's projection matrix.
 */
export type XrDepthRange = "negative-one-to-one" | "zero-to-one";
/**
 * Eye identity within the supported primary stereo view configuration.
 */
export type XrEye = "left" | "right";
/**
 * Engine-independent widget observation, resolved by the shared UI owner.
 */
export type GameUiControlInput = {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "activate";
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "set-toggle";
    /**
     * Observed selection.
     */
    readonly value: boolean;
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "set-range";
    /**
     * Finite selected value within the published bounds.
     */
    readonly value: number;
} | {
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    /**
     * Horizontal ratio from the track's left edge to its right edge.
     */
    readonly ratio: number;
    readonly type: "set-range-ratio";
} | {
    /**
     * Platform key codes observed in the same input frame.
     */
    readonly codes: readonly string[];
    /**
     * Published control identifier.
     */
    readonly controlId: string;
    readonly type: "control-keys";
};
/**
 * Raw stage of a platform chart pointer event.
 */
export type ApplicationChartPointerPhase = "down" | "move" | "up" | "cancel" | "lost_capture";
/**
 * Completion state of the display and scene resources needed for launch.
 */
export type ApplicationResourceObservation = {
    /**
     * Original drawing and rollback diagnostic.
     */
    readonly cause: string;
    readonly kind: "binding_lost";
    /**
     * Candidate belonging to the failed commit request.
     */
    readonly resource_id: number;
} | {
    readonly kind: "prepared";
    /**
     * New opaque GPU resource identity.
     */
    readonly resource_id: number;
} | {
    /**
     * Actual framebuffer and quality after the authorized binding.
     */
    readonly actual: ApplicationResourceCommit;
    readonly kind: "committed";
    /**
     * Exact candidate identity from the commit request.
     */
    readonly resource_id: number;
} | {
    readonly kind: "cancelled";
} | {
    /**
     * Original preparation failure.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Identity and outcome of an actual requested I/O operation.
 */
export type DisplayCompletion = {
    readonly kind: "permission_granted";
    /**
     * Concrete resource created while acquiring permission.
     */
    readonly resource_id?: number | null;
} | {
    readonly kind: "stopped";
} | {
    readonly kind: "started";
    /**
     * Concrete platform resource that is now active.
     */
    readonly resource_id: number;
} | {
    /**
     * Original I/O failure.
     */
    readonly cause: string;
    readonly kind: "failed";
};
/**
 * Presentation mode supported by a platform adapter.
 */
export type GameUiPresentationMode = "screen" | "hmd" | "phone_vr";
/**
 * Parent transform selected by shared physical or world placement.
 */
export type ApplicationMeshAttachment = "world" | "aircraft" | "horizontal_tail" | "vertical_tail";
/**
 * Scalar element format of a shared upload buffer.
 */
export type ApplicationBufferType = "u8" | "f32" | "u32";
/**
 * Head-fixed painting with its final world anchor.
 */
export type ApplicationHeadHudProjection = {
    readonly kind: "absent";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "pending";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "unavailable";
    /**
     * Precise shared failure.
     */
    readonly reason: MenuLayoutFailure;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "ready";
    /**
     * Head-relative surface and texture drawing commands.
     */
    readonly paint: HeadHudPaintProjection;
    /**
     * Final GPU depth, alpha and ordering settings.
     */
    readonly surface_style: XrSurfaceStyle;
    /**
     * Final render-world surface pose.
     */
    readonly world_from_surface: Pose;
    readonly [k: string]: unknown | undefined;
};
/**
 * Rejection class of a measured menu document.
 */
export type MenuLayoutFailure = "stale-view" | "measurement-unavailable" | "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area" | "insufficient-ink-angle" | "missing-text-metrics" | "stale-text-metrics" | "invalid-text-metrics" | "invalid-layout";
/**
 * Engine-independent paint operation in surface coordinates.
 */
export type SurfacePaintItem = {
    /**
     * Surface bounds in metres.
     */
    readonly bounds: MenuRect;
    /**
     * Fill colour.
     */
    readonly color: string;
    /**
     * Optional left-to-right gradient.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly gradient?: readonly [
        string,
        string
    ] | null;
    readonly kind: "rectangle";
    /**
     * Fill opacity.
     */
    readonly opacity: number;
} | {
    /**
     * Outline centre-line bounds in surface metres.
     */
    readonly bounds: MenuRect;
    /**
     * Stroke colour.
     */
    readonly color: string;
    readonly kind: "rounded_outline";
    /**
     * Stroke opacity.
     */
    readonly opacity: number;
    /**
     * Corner radius in surface metres.
     */
    readonly radius: number;
    /**
     * Stroke width in texture pixels.
     */
    readonly widthPx: number;
} | {
    /**
     * Fill colour.
     */
    readonly color: string;
    readonly kind: "triangle";
    /**
     * Fill opacity.
     */
    readonly opacity: number;
    /**
     * Ordered triangle vertices in metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly points: readonly [
        MenuPoint,
        MenuPoint,
        MenuPoint
    ];
} | {
    /**
     * Alphabetic baseline in surface metres.
     */
    readonly anchor: MenuPoint;
    /**
     * Text colour.
     */
    readonly color: string;
    /**
     * Observed font resource.
     */
    readonly font: MenuFont;
    /**
     * Font size in metres.
     */
    readonly fontMeters: number;
    readonly kind: "text";
    /**
     * Text-shaping locale.
     */
    readonly locale: string;
    /**
     * Text opacity.
     */
    readonly opacity: number;
    /**
     * Rotation about the baseline.
     */
    readonly rotationDegrees: number;
    /**
     * Visible line content.
     */
    readonly value: string;
} | {
    /**
     * Stroke colour.
     */
    readonly color: string;
    /**
     * Segment end in metres.
     */
    readonly end: MenuPoint;
    readonly kind: "line";
    /**
     * Stroke opacity.
     */
    readonly opacity: number;
    /**
     * Segment start in metres.
     */
    readonly start: MenuPoint;
    /**
     * Stroke width in texture pixels.
     */
    readonly widthPx: number;
} | {
    /**
     * Fill colour.
     */
    readonly color: string;
    readonly kind: "marker";
    /**
     * Marker centre in surface metres.
     */
    readonly point: MenuPoint;
    /**
     * Square side in texture pixels.
     */
    readonly sizePx: number;
} | {
    /**
     * Plot clipping rectangle in surface metres.
     */
    readonly clip: MenuRect;
    /**
     * Ordered plot operations.
     */
    readonly items: readonly SurfacePaintItem[];
    readonly kind: "plot";
};
/**
 * Spatial menu painting and final mounted poses.
 */
export type ApplicationMenuProjection = {
    readonly kind: "absent";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "pending";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "unavailable";
    /**
     * Precise shared failure.
     */
    readonly reason: MenuLayoutFailure;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Texture allocation in pixels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvas_size: readonly [
        number,
        number
    ];
    /**
     * Visible cursor points in panel-local right/up metres.
     */
    readonly cursors: readonly XrCursorProjection[];
    readonly kind: "ready";
    /**
     * Drawing commands, without interaction policy or hit geometry.
     */
    readonly paint: ApplicationMenuPaint;
    /**
     * Final GPU depth, alpha and ordering settings.
     */
    readonly surface_style: XrSurfaceStyle;
    /**
     * Final render-world panel pose.
     */
    readonly world_from_panel: Pose;
    readonly [k: string]: unknown | undefined;
};
/**
 * Structure is transferred only when its shared revision changes.
 */
export type ApplicationUiProjection = {
    readonly kind: "view";
    /**
     * New structure carrying its shared revision.
     */
    readonly view: GameUiView;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Dynamic values for the retained structural revision.
     */
    readonly frame: GameUiFrame;
    readonly kind: "frame";
    readonly [k: string]: unknown | undefined;
};
/**
 * Exclusive current overlay names retained by both renderer adapters.
 */
export type GameUiOverlay = "Credits" | "Pause" | "PauseSettings" | "PauseHelp";
/**
 * UI message whose handling is shared by native and browser adapters.
 */
export type GameUiAction = {
    /**
     * Button, toggle or range observation from an adapter.
     */
    readonly input: GameUiControlInput;
    readonly kind: "control_input";
} | {
    readonly kind: "set_credits";
    /**
     * Whether the credits replace the current menu.
     */
    readonly open: boolean;
} | {
    /**
     * Operation admitted by the current view.
     */
    readonly command: SessionCommand;
    readonly kind: "session";
} | {
    /**
     * Explicit capability-bound operation.
     */
    readonly command: GameUiPlatformCommand;
    readonly kind: "platform";
} | {
    readonly kind: "toggle_technical";
} | {
    readonly kind: "set_result_tab";
    /**
     * Requested tab.
     */
    readonly tab: GameUiResultTab;
} | {
    /**
     * Requested plot.
     */
    readonly chart: GameUiAnalysisChart;
    readonly kind: "set_analysis_chart";
} | {
    readonly kind: "set_pause_overlay";
    /**
     * Requested overlay.
     */
    readonly overlay: GameUiPauseOverlay;
} | {
    readonly kind: "set_replay_view";
    /**
     * Requested view.
     */
    readonly view: GameUiReplayView;
} | {
    readonly kind: "cycle_replay_camera";
} | {
    readonly kind: "seek_analysis";
    /**
     * Requested recorded time.
     */
    readonly time_seconds: number;
} | {
    readonly kind: "seek_replay";
    /**
     * Requested recorded time.
     */
    readonly time_seconds: number;
} | {
    readonly kind: "toggle_replay_playing";
} | {
    /**
     * Zero is half speed, one normal speed, two double speed.
     */
    readonly code: number;
    readonly kind: "set_replay_rate";
};
/**
 * Canonical application operations generated by every game UI.
 */
export type SessionCommand = {
    readonly kind: "open_setup";
} | {
    readonly kind: "return_to_title";
} | {
    readonly kind: "prepare";
} | {
    readonly kind: "cancel_briefing";
} | {
    readonly kind: "start_countdown";
} | {
    readonly kind: "cancel_countdown";
} | {
    readonly kind: "pause";
} | {
    readonly kind: "resume";
} | {
    readonly kind: "abort";
} | {
    readonly kind: "retry";
} | {
    readonly kind: "retry_briefing";
} | {
    readonly kind: "enter_replay";
} | {
    readonly kind: "leave_replay";
} | {
    readonly kind: "enter_attract";
} | {
    readonly kind: "leave_attract";
} | {
    readonly kind: "select_preset";
    /**
     * Named preset; Custom is not a selectable shortcut.
     */
    readonly preset: DifficultyPreset;
} | {
    /**
     * Existing information level.
     */
    readonly information: InformationLevel;
    readonly kind: "select_information";
} | {
    /**
     * Existing assistance level.
     */
    readonly assistance: AssistanceLevel;
    readonly kind: "select_assistance";
} | {
    readonly kind: "select_weather";
    /**
     * Existing weather class.
     */
    readonly weather: WeatherClass;
} | {
    /**
     * Existing cue.
     */
    readonly cue: HudCue;
    readonly kind: "set_cue";
    /**
     * Requested visibility.
     */
    readonly visible: boolean;
};
/**
 * Named preset whose axes are resolved independently.
 */
export type DifficultyPreset = "beginner" | "standard" | "expert" | "realistic" | "custom";
/**
 * Information presentation selected for a session.
 */
export type InformationLevel = "full" | "standard" | "minimal" | "realistic" | "custom";
/**
 * Controller category resolved from the Assistance axis.
 */
export type AssistanceLevel = "strong" | "assisted" | "light" | "manual";
/**
 * Weather classification attached to a versioned scenario catalog.
 */
export type WeatherClass = "calm" | "mild" | "typical" | "challenging" | "near_limit";
/**
 * Stable identity for one independently configurable HUD cue.
 */
export type HudCue = "telemetry" | "attitude" | "wind" | "flight_path" | "angle_of_attack" | "warnings";
/**
 * Platform-owned operation requested by common UI.
 */
export type GameUiPlatformCommand = {
    readonly kind: "set_screen_render_scale";
    /**
     * Requested fraction of each physical rendering dimension.
     */
    readonly scale: ScreenRenderScale;
} | {
    /**
     * Whether GPU-work-based adjustment is selected.
     */
    readonly enabled: boolean;
    readonly kind: "set_screen_render_auto";
} | {
    readonly kind: "set_water_quality";
    /**
     * Requested quality.
     */
    readonly quality: WaterQuality;
} | {
    readonly kind: "set_presentation";
    /**
     * Requested mode.
     */
    readonly mode: GameUiPresentationMode;
} | {
    readonly kind: "recenter_phone_tracking";
} | {
    readonly kind: "recenter_menu";
} | {
    readonly kind: "download_record_json";
} | {
    readonly kind: "download_flight_csv";
} | {
    /**
     * Platform persistence key.
     */
    readonly id: number;
    readonly kind: "open_stored_record";
} | {
    /**
     * Shared UI selection to apply.
     */
    readonly camera: GameUiReplayCamera;
    readonly kind: "set_replay_camera";
};
/**
 * Replay camera candidates in their shared selection order.
 */
export type GameUiReplayCamera = "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
/**
 * Result presentation mode.
 */
export type GameUiResultTab = "summary" | "analysis";
/**
 * Plot generated from the retained record.
 */
export type GameUiAnalysisChart = "map" | "altitude" | "speed";
/**
 * Exclusive pause overlay.
 */
export type GameUiPauseOverlay = "menu" | "settings" | "help";
/**
 * Replay presentation selection.
 */
export type GameUiReplayView = "cinematic" | "telemetry" | "analysis";
/**
 * Widget implementations must exhaustively handle each display primitive.
 */
export type GameUiHudNodeContent = {
    readonly kind: "text";
    /**
     * Shared visual priority.
     */
    readonly tone: GameUiHudTone;
    /**
     * Fully formatted text; adapters never parse its wording.
     */
    readonly value: string;
} | {
    /**
     * Shared horizon and marker geometry.
     */
    readonly geometry: GameUiHudAdiGeometry;
    readonly kind: "attitude";
    /**
     * Fully formatted pitch, roll, or flight-path reading.
     */
    readonly value: string;
} | {
    /**
     * Logical instrument canvas dimensions.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvas_size: readonly [
        number,
        number
    ];
    readonly kind: "heading_tape";
    /**
     * Fixed pointer, ordered tip and base corners.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly pointer: readonly [
        GameUiChartPoint,
        GameUiChartPoint,
        GameUiChartPoint
    ];
    /**
     * Ordered completed marks and labels.
     */
    readonly ticks: readonly GameUiHudHeadingTick[];
    /**
     * Formatted central reading.
     */
    readonly value: string;
} | {
    /**
     * Completed filled arrow polygon, absent when no direction is defined.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly arrow?: readonly [
        GameUiChartPoint,
        GameUiChartPoint,
        GameUiChartPoint,
        GameUiChartPoint
    ] | null;
    /**
     * Logical instrument canvas dimensions.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvas_size: readonly [
        number,
        number
    ];
    /**
     * Circle outline bounds and corner radius.
     */
    readonly circle_bounds: GameUiRect;
    /**
     * Circle outline radius in logical pixels.
     */
    readonly circle_radius: number;
    /**
     * Circle stroke width in logical pixels.
     */
    readonly circle_stroke_width: number;
    /**
     * Fixed compass reference segments.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly cross_segments: readonly [
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ]
    ];
    /**
     * Reference segment stroke width in logical pixels.
     */
    readonly cross_stroke_width: number;
    readonly kind: "direction";
    /**
     * North label's middle baseline anchor.
     */
    readonly north_anchor: GameUiChartPoint;
    /**
     * North reference label.
     */
    readonly north_label: string;
    /**
     * Formatted direction and magnitude reading.
     */
    readonly value: string;
} | {
    /**
     * Logical instrument canvas dimensions.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvas_size: readonly [
        number,
        number
    ];
    readonly kind: "linear_gauge";
    /**
     * Left-start and right-end alphabetic label baselines.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly label_anchors: readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ];
    /**
     * Reading pointer, ordered tip and base corners; absent for an undefined reading.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly marker?: readonly [
        GameUiChartPoint,
        GameUiChartPoint,
        GameUiChartPoint
    ] | null;
    /**
     * Right endpoint label.
     */
    readonly maximum_label: string;
    /**
     * Left endpoint label.
     */
    readonly minimum_label: string;
    /**
     * Ordered tick endpoints.
     *
     * @minItems 7
     * @maxItems 7
     */
    readonly ticks: readonly [
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ]
    ];
    /**
     * Scale track endpoints.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly track: readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ];
    /**
     * Formatted reading including its unit.
     */
    readonly value: string;
};
/**
 * Shared text priority; mandatory resource failures always use Warning.
 */
export type GameUiHudTone = "normal" | "muted" | "warning";
/**
 * HUD placement roles consumed identically by all adapters.
 */
export type GameUiHudPlacement = "readouts" | "instruments";
/**
 * Shared viewport-relative placement.
 */
export type GameUiShell = "center" | "flight_bar" | "pause_settings" | "analysis" | "cinematic";
/**
 * Engine-independent anchor.
 */
export type GameUiAnchor = "menu" | "head" | "cockpit" | "world";
/**
 * Exclusive shared control representation.
 */
export type GameUiControlContent = {
    /**
     * Typed semantic action.
     */
    readonly action: GameUiAction;
    readonly kind: "button";
    /**
     * Shared visual hierarchy.
     */
    readonly presentation: GameUiButtonPresentation;
} | {
    /**
     * Cue identifier from the existing format model.
     */
    readonly cue: HudCue;
    readonly kind: "toggle";
    /**
     * Current Rust-owned visibility.
     */
    readonly value: boolean;
} | {
    /**
     * Shared one-decimal display text.
     */
    readonly display_value: string;
    readonly kind: "range";
    /**
     * Last permitted time.
     */
    readonly maximum: number;
    /**
     * First permitted time.
     */
    readonly minimum: number;
    /**
     * Logical range step.
     */
    readonly step: number;
    /**
     * Analysis or playback target.
     */
    readonly target: GameUiRangeTarget;
    /**
     * Current time.
     */
    readonly value: number;
} | {
    readonly kind: "status";
    /**
     * Typed dynamic-value provider; static values require no frame patch.
     */
    readonly source: GameUiReadoutSource;
    /**
     * Shared formatted content.
     */
    readonly value: string;
} | {
    /**
     * Ordered values and units.
     */
    readonly fields: readonly GameUiNumericField[];
    readonly kind: "fields";
    /**
     * Shared dynamic-value provider.
     */
    readonly source: GameUiReadoutSource;
} | {
    /**
     * Plot selection.
     */
    readonly chart: GameUiAnalysisChart;
    readonly kind: "chart";
    /**
     * Fully derived axes, geometry and cursor; adapters render this value.
     */
    readonly plot: GameUiChart;
};
/**
 * Shared button role and selection state.
 */
export type GameUiButtonPresentation = {
    /**
     * Primary or secondary emphasis.
     */
    readonly emphasis: GameUiEmphasis;
    readonly kind: "action";
} | {
    /**
     * Shared option explanation.
     */
    readonly description: string;
    /**
     * Stable group identifier.
     */
    readonly group: string;
    /**
     * Group heading.
     */
    readonly group_label: string;
    readonly kind: "choice";
    /**
     * Current selected option.
     */
    readonly selected: boolean;
} | {
    /**
     * Current disclosure state.
     */
    readonly expanded: boolean;
    readonly kind: "disclosure";
};
/**
 * Shared action emphasis.
 */
export type GameUiEmphasis = "primary" | "secondary" | "normal";
/**
 * Range action target.
 */
export type GameUiRangeTarget = "analysis" | "replay";
/**
 * Shared source for frame-varying readouts, independent of element identity.
 */
export type GameUiReadoutSource = "static" | "countdown" | "playback_time" | "playback_summary" | "analysis_cursor";
/**
 * Horizontal alignment relative to a chart label's baseline anchor.
 */
export type GameUiChartTextAnchor = "start" | "middle" | "end";
/**
 * Group placement strategy, selected in shared Rust.
 */
export type GameUiSectionLayout = "column" | "choice_grid" | "row";
/**
 * Shared scene name, preserving current Web spelling.
 */
export type GameUiScene = "Boot" | "Title" | "FlightSetup" | "Briefing" | "Countdown" | "Flight" | "Result" | "Replay";
/**
 * Ownership stage controlling input admission and the executable owner's final release.
 */
export type ApplicationLifecycle = "running" | "closing" | "drained";
/**
 * Ordered platform operations produced by the application owner.
 */
export type ApplicationRequest = {
    readonly kind: "start_phone_sensors";
    /**
     * Owner-issued sensor subscription identity.
     */
    readonly resource_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "stop_phone_sensors";
    /**
     * Owner-issued sensor subscription identity.
     */
    readonly resource_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "cancel_record_io";
    /**
     * Original storage operation identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "create_renderer";
    /**
     * Owner-issued initialization identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "release_renderer";
    /**
     * Acquired graphics context identity.
     */
    readonly renderer_resource_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "reload_application";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "cancel_resources";
    /**
     * Preparation identity to cancel.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "commit_resources";
    /**
     * Owner-authorized preparation identity.
     */
    readonly request_id: number;
    /**
     * Exact candidate GPU identity.
     */
    readonly resource_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "release_resources";
    /**
     * Opaque GPU resource identity to release.
     */
    readonly resource_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "begin_record_preview";
    /**
     * Exact maximum number of cursor rows to retrieve.
     */
    readonly limit: number;
    /**
     * Owner-issued read transaction identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "record_preview";
    /**
     * Owner-issued read transaction identity.
     */
    readonly request_id: number;
    /**
     * Ordered primitive reads selected by shared Rust.
     */
    readonly requests: readonly RecordPreviewRequest[];
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "begin_record_save";
    /**
     * Owner-issued transaction identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "record_write";
    /**
     * Owner-issued transaction identity.
     */
    readonly request_id: number;
    /**
     * Ordered operations selected by the shared application policy.
     */
    readonly requests: readonly RecordWriteRequest[];
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Deadline on the shared receipt clock, in milliseconds.
     */
    readonly at_ms: number;
    readonly kind: "schedule_wake";
    /**
     * Owner-issued timer identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "cancel_wake";
    /**
     * Identity of the previously scheduled timer.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Revision-scoped CPU drawing data; image bytes use the common buffer channel.
     */
    readonly assets: ApplicationEnvironmentAssets;
    /**
     * Active platform display resource receiving the scene.
     */
    readonly display_resource_id?: number | null;
    /**
     * Canonical environment to render.
     */
    readonly environment: EnvironmentIdentity;
    /**
     * Complete desired renderer configuration to prepare and bind atomically.
     */
    readonly graphics: ApplicationGraphicsTarget;
    readonly kind: "prepare_resources";
    /**
     * Owner-issued identity to echo in the completion.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "display";
    /**
     * One primitive platform display operation.
     */
    readonly request: DisplayRequest;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "read_record";
    /**
     * Selected persistence key.
     */
    readonly record_id: number;
    /**
     * Owner-issued read identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Exact output bytes, including preserved original archive JSON.
     */
    readonly bytes: readonly number[];
    /**
     * Suggested file name.
     */
    readonly file_name: string;
    readonly kind: "download_file";
    /**
     * Media type for the file bytes.
     */
    readonly mime_type: string;
    /**
     * Owner-issued file operation identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
};
/**
 * Primitive reads following the initial bounded cursor.
 */
export type RecordPreviewRequest = {
    /**
     * Keys in the order expected by the next observation.
     */
    readonly keys: readonly string[];
    readonly kind: "read_best";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "finish";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "abort";
    readonly [k: string]: unknown | undefined;
};
/**
 * Record-store operations within one already opened atomic save transaction.
 */
export type RecordWriteRequest = {
    /**
     * Canonical comparison key.
     */
    readonly key: string;
    readonly kind: "read_best";
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Persistence key.
     */
    readonly id: number;
    readonly kind: "read_record";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "scan_records";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "continue_records";
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Original finalized JSON.
     */
    readonly json: string;
    readonly kind: "insert_record";
    /**
     * Canonical RFC 3339 save timestamp.
     */
    readonly saved_at: string;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Generated persistence key.
     */
    readonly id: number;
    readonly kind: "write_metadata";
    /**
     * Same timestamp as the inserted record.
     */
    readonly saved_at: string;
    readonly [k: string]: unknown | undefined;
} | {
    /**
     * Exact index replacement.
     */
    readonly entry: PersistedBest;
    readonly kind: "write_best";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "finish";
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "abort";
    readonly [k: string]: unknown | undefined;
};
/**
 * Pixel encoding submitted directly to a graphics API.
 */
export type ApplicationImageEncoding = "srgb_rgba8" | "linear_rgba8";
/**
 * Shared admission of the launch venue, independent of display resources.
 */
export type EnvironmentVenueProjection = {
    readonly kind: "shared_launch";
} | {
    readonly kind: "registered_origin";
} | {
    readonly kind: "unregistered_origin";
};
/**
 * Primitive I/O operation; Rust chooses the next stage from its completion.
 */
export type DisplayRequest = {
    readonly kind: "permission";
    /**
     * Mode requiring permission.
     */
    readonly mode: GameUiPresentationMode;
    /**
     * Owner-issued operation identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "stop";
    /**
     * Mode being stopped.
     */
    readonly mode: GameUiPresentationMode;
    /**
     * Owner-issued operation identity.
     */
    readonly request_id: number;
    /**
     * Actual active resource, if separately allocated.
     */
    readonly resource_id?: number | null;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "start";
    /**
     * Mode being started.
     */
    readonly mode: GameUiPresentationMode;
    /**
     * Resource returned by permission acquisition.
     */
    readonly permission_resource_id?: number | null;
    /**
     * Owner-issued operation identity.
     */
    readonly request_id: number;
    readonly [k: string]: unknown | undefined;
} | {
    readonly kind: "release";
    /**
     * Concrete platform resource to release.
     */
    readonly resource_id: number;
    readonly [k: string]: unknown | undefined;
};
export interface ApplicationContract {
    readonly font_metrics: ApplicationGlyphMetrics;
    readonly font_probe: ApplicationTextProbe;
    readonly observation: ApplicationObservation;
    readonly output: ApplicationOutput;
    readonly platform: ApplicationPlatformObservation;
}
/**
 * Unmodified platform text metrics in logical pixels.
 */
export interface ApplicationGlyphMetrics {
    /**
     * Horizontal advance.
     */
    readonly advance_px: number;
    /**
     * Signed ink ascent above the baseline.
     */
    readonly ascent_px: number;
    /**
     * Signed ink descent below the baseline.
     */
    readonly descent_px: number;
    /**
     * Signed left ink bearing.
     */
    readonly left_px: number;
    /**
     * Signed right ink bearing.
     */
    readonly right_px: number;
}
/**
 * A direct platform text-measurement request in logical pixels.
 */
export interface ApplicationTextProbe {
    /**
     * Font resource to select for this measurement.
     */
    readonly font: MenuFont;
    /**
     * Font height in logical pixels.
     */
    readonly font_size_px: number;
    /**
     * Platform text-shaping locale.
     */
    readonly locale: string;
    /**
     * Exact contextual string to measure.
     */
    readonly text: string;
    readonly [k: string]: unknown | undefined;
}
/**
 * Font resource observation supplied by the drawing adapter.
 */
export interface MenuFont {
    /**
     * Adapter font family identifier.
     */
    readonly family: string;
    /**
     * Adapter font resource generation.
     */
    readonly generation: number;
    /**
     * Observed font slant.
     */
    readonly style: MenuFontStyle;
    /**
     * Font weight in the range 1 through 1000.
     */
    readonly weight: number;
}
/**
 * One external observation delivered to the shared application owner.
 */
export interface ApplicationObservation {
    /**
     * Raw frame, event or I/O completion received at this time.
     */
    readonly input: ApplicationInput;
    /**
     * Monotonic receipt time in milliseconds; source clocks are converted to this origin at I/O.
     */
    readonly observed_ms: number;
}
/**
 * Actual renderer creation and drawing-buffer observations.
 */
export interface ApplicationRendererObservation {
    /**
     * Actual drawable extent; XR extents come directly from the runtime.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly extent_px: readonly [
        number,
        number
    ];
    /**
     * Actual timestamp-query capability of this context, observed before initial resource preparation.
     */
    readonly gpu_timing: ApplicationGpuTimingCapability;
    /**
     * Confirmed resources at this renderer's creation, before owner-issued changes.
     */
    readonly initial_quality: ApplicationRenderQuality;
    /**
     * Creation/restoration identity, replaced after context loss or device replacement.
     */
    readonly resource_id: number;
    /**
     * Unscaled physical Screen surface dimensions.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly screen_extent_px: readonly [
        number,
        number
    ];
    /**
     * Actual HMD runtime framebuffer dimensions when available.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly xr_extent_px?: readonly [
        number,
        number
    ] | null;
}
/**
 * Renderer resource configuration; Screen scale is independent of runtime XR framebuffer size.
 */
export interface ApplicationRenderQuality {
    /**
     * Retained Screen drawing resolution.
     */
    readonly scale: ScreenRenderScale;
    /**
     * Directional shadow-map resolution.
     */
    readonly shadow: ScreenShadowMap;
    /**
     * Water geometry and wave budget.
     */
    readonly water_quality: WaterQuality;
}
/**
 * Raw rows retrieved by a bounded descending metadata cursor in one read transaction.
 */
export interface RecordPreviewRow {
    /**
     * Unmodified metadata primary key.
     */
    readonly key: unknown;
    /**
     * Unmodified metadata row.
     */
    readonly metadata: unknown;
    /**
     * Unmodified record retrieved under the same primary key.
     */
    readonly record: unknown;
}
/**
 * Tag to attach unchanged to a raw GPU query result.
 */
export interface ApplicationGpuQuery {
    /**
     * Policy's measurement generation.
     */
    readonly epoch: number;
    /**
     * Owner-issued query source frame.
     */
    readonly frame: number;
    /**
     * Graphics context or device lifetime.
     */
    readonly renderer_resource_id: number;
    /**
     * Source time on the common monotonic clock.
     */
    readonly source_ms: number;
}
/**
 * Unmodified tracked controller pose in the active runtime reference space.
 */
export interface ApplicationPointerObservation {
    /**
     * Platform input-source identity, retired when that source disconnects.
     */
    readonly source_id: number;
    /**
     * Runtime target-ray-space pose in tracking coordinates.
     */
    readonly tracking_from_ray: XrObservedPose;
}
/**
 * Nullable components received from an unvalidated tracking API observation.
 */
export interface XrObservedPose {
    readonly orientation: ObservedQuaternion;
    readonly position: ObservedVector;
}
export interface ObservedQuaternion {
    readonly w?: number | null;
    readonly x?: number | null;
    readonly y?: number | null;
    readonly z?: number | null;
}
export interface ObservedVector {
    readonly x?: number | null;
    readonly y?: number | null;
    readonly z?: number | null;
}
/**
 * Unmodified orientation angles and screen rotation from one sensor callback.
 */
export interface PhoneOrientationObservation {
    /**
     * Whether the browser supplied an Earth-referenced orientation.
     */
    readonly absolute: boolean;
    /**
     * Rotation around device Z, in degrees.
     */
    readonly alpha?: number | null;
    /**
     * Intrinsic rotation around device X, in degrees.
     */
    readonly beta?: number | null;
    /**
     * Intrinsic rotation around device Y, in degrees.
     */
    readonly gamma?: number | null;
    /**
     * Provider's coherent acquisition ID; absent for independent DOM events.
     */
    readonly sample_id?: number | null;
    /**
     * Current screen rotation, in degrees, absent when unavailable.
     */
    readonly screen_angle_degrees?: number | null;
    /**
     * Browser event that supplied this sample.
     */
    readonly source: PhoneOrientationSource;
    /**
     * Sensor event timestamp on the presentation clock.
     */
    readonly timestamp_ms: number;
}
/**
 * Raw motion components from one platform sensor observation.
 */
export interface PhoneMotionObservation {
    /**
     * Device-axis acceleration in metres per second squared.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly acceleration: readonly [
        number | null,
        number | null,
        number | null
    ];
    /**
     * Device-axis acceleration including gravity, in metres per second squared.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly acceleration_including_gravity: readonly [
        number | null,
        number | null,
        number | null
    ];
    /**
     * Provider's coherent acquisition ID; absent for independent DOM events.
     */
    readonly sample_id?: number | null;
    /**
     * Sensor event timestamp on the presentation clock.
     */
    readonly timestamp_ms: number;
}
/**
 * Unfiltered platform gamepad data in the standard browser/OpenXR mapping vocabulary.
 */
export interface ApplicationGamepadObservation {
    /**
     * Raw axis order; missing and non-finite samples are None.
     */
    readonly axes: readonly (number | null)[];
    /**
     * Raw button pressed states in platform order.
     */
    readonly buttons: readonly boolean[];
    /**
     * Raw connection status.
     */
    readonly connected: boolean;
    /**
     * Physical slot/entity and observed connection generation.
     */
    readonly identity: FlightDeviceIdentity;
    /**
     * Platform mapping declaration, such as standard or xr-standard.
     */
    readonly mapping: string;
}
/**
 * Exact platform device identity, including entity and reconnection generations.
 */
export interface FlightDeviceIdentity {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Same-frame spatial input sampled by the platform I/O adapter.
 */
export interface ApplicationSpatialObservation {
    /**
     * Connected standard gamepads, before device selection or dead-zone filtering.
     */
    readonly gamepads: readonly ApplicationGamepadObservation[];
    /**
     * Identity of the spatial projection actually submitted by the drawing adapter.
     */
    readonly presented_id?: number | null;
    /**
     * Same-frame raw XR sources, including temporarily untracked controllers.
     */
    readonly sources: readonly XrInputSourceObservation[];
}
/**
 * One source's same-frame tracking and physical inputs.
 */
export interface XrInputSourceObservation {
    /**
     * Actual gamepad, absent when the source has no gamepad API.
     */
    readonly gamepad?: XrInputGamepadObservation | null;
    /**
     * Raw handedness, independent of the source handle.
     */
    readonly handedness: string;
    /**
     * Platform handle for this source's connection lifetime.
     */
    readonly source_id: number;
    /**
     * Raw target-ray mode.
     */
    readonly target_ray_mode: string;
    /**
     * Tracking-space target-ray pose; absent tracking retains button observations.
     */
    readonly tracking_from_ray?: XrObservedPose | null;
}
/**
 * Unfiltered gamepad values associated with one XR input source.
 */
export interface XrInputGamepadObservation {
    /**
     * Raw axes; non-finite observations are None.
     */
    readonly axes: readonly (number | null)[];
    /**
     * Raw button order, including placeholder buttons.
     */
    readonly buttons: readonly XrInputButtonObservation[];
    /**
     * Platform mapping declaration.
     */
    readonly mapping: string;
}
/**
 * Button state copied from the active XR frame.
 */
export interface XrInputButtonObservation {
    /**
     * Platform pressed state.
     */
    readonly pressed: boolean;
    /**
     * Platform touch state.
     */
    readonly touched: boolean;
}
/**
 * Payload of one externally observed event.
 */
export interface ApplicationXrFrameObservation {
    /**
     * Runtime input-focus observation.
     */
    readonly input_focused: boolean;
    /**
     * Unmodified same-reference head and eye observations.
     */
    readonly observation: XrViewerObservation;
    /**
     * Runtime visibility observed with this frame.
     */
    readonly visibility: XrVisibility;
}
/**
 * Platform observations copied together from one frame and reference space.
 */
export interface XrViewerObservation {
    /**
     * Origin of the optical observations.
     */
    readonly source: XrViewerSource;
    /**
     * Centre-head pose, or absent when runtime tracking is unavailable.
     */
    readonly trackingFromHead?: XrObservedPose | null;
    /**
     * Platform view order is preserved until shared eye classification.
     */
    readonly views: readonly XrViewObservation[];
}
/**
 * One platform view before head-relative geometry has been derived.
 */
export interface XrViewObservation {
    /**
     * Rendering API clip-depth convention.
     */
    readonly depthRange: XrDepthRange;
    /**
     * Primary eye identity; absent for an unsupported platform view.
     */
    readonly eye?: XrEye | null;
    /**
     * Unmodified column-major runtime projection values.
     */
    readonly projection: readonly (number | null)[];
    /**
     * Eye pose in the same reference space as the centre head.
     */
    readonly trackingFromEye: XrObservedPose;
}
/**
 * Measured or initially assumed chart content dimensions in logical pixels.
 */
export interface GameUiChartExtent {
    /**
     * Content height.
     */
    readonly height: number;
    /**
     * Content width.
     */
    readonly width: number;
}
/**
 * External metrics and API availability, without derived UI state.
 */
export interface ApplicationPlatformObservation {
    /**
     * Actual API and drawing support.
     */
    readonly capabilities: ApplicationCapabilities;
    /**
     * Explicit GPU diagnostic demand, independent of automatic quality selection.
     */
    readonly gpu_timing_requested: boolean;
    /**
     * Live renderer and its startup resources; None retires that lifetime and replacement uses a new creation identity.
     */
    readonly renderer?: ApplicationRendererObservation | null;
    /**
     * Actual loaded surface font and shaping locale, absent while font I/O is pending.
     */
    readonly surface?: ApplicationSurfaceObservation | null;
    /**
     * Physical logical-pixel viewport and root font size.
     */
    readonly viewport: GameUiViewport;
}
/**
 * Capabilities implemented by the platform's I/O and drawing adapters.
 */
export interface ApplicationCapabilities {
    /**
     * Drawing adapter supports record plots.
     */
    readonly analysis_available: boolean;
    /**
     * Platform can save an exported file.
     */
    readonly flight_log_available: boolean;
    /**
     * Runtime can create a tracked head-mounted display.
     */
    readonly hmd_available: boolean;
    /**
     * Raw orientation API and permission-method availability, absent without the required browser APIs.
     */
    readonly phone_sensors?: PhoneSensorApiObservation | null;
    /**
     * Drawing adapter supports recorded playback.
     */
    readonly replay_available: boolean;
    /**
     * Platform provides persistent record I/O.
     */
    readonly stored_records_available: boolean;
}
/**
 * Browser orientation API observations before a sensor sample is available.
 */
export interface PhoneSensorApiObservation {
    /**
     * The orientation constructor exposes a gesture-only permission method.
     */
    readonly permission_required: boolean;
    /**
     * Raw screen rotation, absent when the browser cannot provide it.
     */
    readonly screen_angle_degrees?: number | null;
}
/**
 * Actual font resource and shaping locale reported by platform font I/O.
 */
export interface ApplicationSurfaceObservation {
    /**
     * Loaded font resource identity.
     */
    readonly font: MenuFont;
    /**
     * Platform text-shaping locale.
     */
    readonly locale: string;
}
/**
 * Logical pixels and root-font size used by the same responsive calculation.
 */
export interface GameUiViewport {
    /**
     * Display height in logical pixels.
     */
    readonly height: number;
    /**
     * Platform root font size in logical pixels.
     */
    readonly root_font_size: number;
    /**
     * Display width in logical pixels.
     */
    readonly width: number;
}
/**
 * Actual values observed after an authorized atomic resource binding.
 */
export interface ApplicationResourceCommit {
    /**
     * Quality actually applied by the drawing API.
     */
    readonly applied: ApplicationRenderQuality;
    /**
     * Actual framebuffer dimensions after binding.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly extent_px: readonly [
        number,
        number
    ];
}
/**
 * One update's I/O work and optional drawing submission.
 */
export interface ApplicationOutput {
    /**
     * Technical failure details for the platform diagnostic sink, separate from UI wording.
     */
    readonly diagnostic?: string | null;
    /**
     * Absent for synchronous I/O continuations that leave the visible state unchanged.
     */
    readonly drawing?: ApplicationDrawingOutput | null;
    /**
     * Whether the observed key belongs to the active flight input.
     */
    readonly keyboard_consumed: boolean;
    /**
     * The executable owner can be released only after this becomes Drained.
     */
    readonly lifecycle: ApplicationLifecycle;
    /**
     * Ordered I/O requests to execute at the platform boundary.
     */
    readonly requests: readonly ApplicationRequest[];
    readonly [k: string]: unknown | undefined;
}
/**
 * One shared update's rendering and I/O work.
 */
export interface ApplicationDrawingOutput {
    /**
     * Confirmed graphics resources and optional query for this frame.
     */
    readonly graphics: ApplicationGraphicsProjection;
    /**
     * Final visibility decisions for drawing adapters.
     */
    readonly presentation: ApplicationDrawingPresentation;
    /**
     * Aircraft and camera poses ready for direct drawing-engine transfer.
     */
    readonly scene: ApplicationSceneProjection;
    /**
     * Spatial painting for an admitted frame; other notifications retain submitted painting.
     */
    readonly spatial?: ApplicationSpatialProjection | null;
    /**
     * Structural replacement or dynamic UI values.
     */
    readonly ui: ApplicationUiProjection;
    readonly [k: string]: unknown | undefined;
}
/**
 * Drawing resource identity and optional GPU query for one output.
 */
export interface ApplicationGraphicsProjection {
    /**
     * Query to begin around this frame's GPU work.
     */
    readonly query?: ApplicationGpuQuery | null;
    /**
     * Current drawable resources, absent while the renderer is unavailable.
     */
    readonly resource?: ApplicationGraphicsResource | null;
    readonly [k: string]: unknown | undefined;
}
/**
 * Identity of the committed resources used for scene preparation.
 */
export interface ApplicationGraphicsResource {
    /**
     * Owner-issued resource commit generation.
     */
    readonly commit: number;
    /**
     * Actual drawable dimensions belonging to this preparation identity.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly extent_px: readonly [
        number,
        number
    ];
    /**
     * Confirmed committed configuration.
     */
    readonly quality: ApplicationRenderQuality;
    /**
     * Graphics context or device lifetime.
     */
    readonly renderer_resource_id: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * One shared update's rendering and I/O work.
 */
export interface ApplicationDrawingPresentation {
    /**
     * Shared frame-rate text used by both Screen and Head HUD drawing.
     */
    readonly frame_rate_label: string;
    /**
     * Confirmed active display, absent during an unbound transition.
     */
    readonly mode?: GameUiPresentationMode | null;
    /**
     * Submit the prepared 3D scene for this display.
     */
    readonly scene_visible: boolean;
    /**
     * Submit the shared screen-anchored HUD.
     */
    readonly screen_hud_visible: boolean;
    /**
     * Submit the shared DOM/native screen menu.
     */
    readonly screen_ui_visible: boolean;
    readonly [k: string]: unknown | undefined;
}
/**
 * Physical display metadata and its final render-space placement.
 */
export interface ApplicationSceneProjection {
    /**
     * Current display's camera geometry.
     */
    readonly camera?: ApplicationCameraProjection | null;
    /**
     * Complete accepted mutable attributes and final articulated part transforms.
     */
    readonly geometry?: ApplicationGeometryFrame | null;
    /**
     * Final directional-light placement and shadow volume.
     */
    readonly lighting?: LakeSceneShadow | null;
    /**
     * Final reflection-pass camera and world-to-clip matrix.
     */
    readonly reflection?: PlanarReflection | null;
    /**
     * Whether the registered world geometry belongs to this admitted environment.
     */
    readonly venue_visible: boolean;
    /**
     * Final horizontal centre used by the far-water overlap clip.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly water_clip_center: readonly [
        number,
        number
    ];
    /**
     * Accepted simulation time for water animation.
     */
    readonly water_time_seconds: number;
    /**
     * Final render-world water translation, independent of aircraft rotation and altitude.
     */
    readonly water_world_from_surface: Pose;
    /**
     * Aircraft datum and orientation already converted into render-world axes.
     */
    readonly world_from_aircraft: Pose;
    readonly [k: string]: unknown | undefined;
}
/**
 * Camera geometry ready for direct transfer to a drawing engine.
 */
export interface ApplicationCameraProjection {
    /**
     * Conservative head-local OpenGL culling volume; absent means culling is disabled.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly culling_projection?: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ] | null;
    /**
     * Final left and right eye transforms, empty for Screen or unavailable optics.
     */
    readonly eyes: readonly ApplicationEyeProjection[];
    /**
     * Final monoscopic OpenGL projection, absent for stereo displays.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly screen_projection?: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ] | null;
    /**
     * Current centre camera, absent while tracking has no valid head observation.
     */
    readonly world_from_head?: Pose | null;
    readonly [k: string]: unknown | undefined;
}
/**
 * One eye's final render-world pose and unchanged projection convention.
 */
export interface ApplicationEyeProjection {
    /**
     * Projection depth convention.
     */
    readonly depth_range: XrDepthRange;
    /**
     * Primary eye identity.
     */
    readonly eye: XrEye;
    /**
     * Column-major projection matrix.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly projection: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ];
    /**
     * Exact configured Phone framebuffer viewport; HMD viewports come from the runtime.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly viewport_px?: readonly [
        number,
        number,
        number,
        number
    ] | null;
    /**
     * Eye-to-render-world transform.
     */
    readonly world_from_eye: Pose;
    readonly [k: string]: unknown | undefined;
}
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
 */
export interface Pose {
    /**
     * Child-to-parent orientation.
     */
    readonly orientation: Quaternion;
    /**
     * Child origin expressed in parent coordinates, in metres.
     */
    readonly position: Vector3;
}
/**
 * Scalar-first unit quaternion in a right-handed reference space.
 */
export interface Quaternion {
    /**
     * Scalar component.
     */
    readonly w: number;
    /**
     * Right-axis component.
     */
    readonly x: number;
    /**
     * Up-axis component.
     */
    readonly y: number;
    /**
     * Back-axis component.
     */
    readonly z: number;
}
/**
 * Metric right/up/back coordinates in the explicitly named reference space.
 */
export interface Vector3 {
    /**
     * Rightward coordinate in metres.
     */
    readonly x: number;
    /**
     * Upward coordinate in metres.
     */
    readonly y: number;
    /**
     * Backward coordinate in metres.
     */
    readonly z: number;
}
/**
 * Mutable attributes and articulation already resolved by the shared owner.
 */
export interface ApplicationGeometryFrame {
    /**
     * Whether registered aircraft geometry is available for this source.
     */
    readonly aircraft_visible: boolean;
    /**
     * Final horizontal and vertical tail transforms.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly part_poses: readonly [
        ApplicationMeshPose,
        ApplicationMeshPose
    ];
    /**
     * Geometry upload generation, advancing only after a complete accepted update.
     */
    readonly revision: number;
    /**
     * Attribute replacements; empty while the accepted coefficient is unchanged.
     */
    readonly updates: readonly ApplicationMeshUpdate[];
    readonly [k: string]: unknown | undefined;
}
/**
 * Final local pose for an articulated mesh parent.
 */
export interface ApplicationMeshPose {
    /**
     * Shared articulation parent.
     */
    readonly attachment: ApplicationMeshAttachment;
    /**
     * Final aircraft-local transform, including the physical pivot.
     */
    readonly pose: Pose;
    readonly [k: string]: unknown | undefined;
}
/**
 * One replacement attribute pair using retained mutable CPU buffers.
 */
export interface ApplicationMeshUpdate {
    /**
     * Static mesh receiving these attributes.
     */
    readonly mesh_id: number;
    /**
     * Current completed normals.
     */
    readonly normal: ApplicationBufferDescriptor;
    /**
     * Current accepted positions.
     */
    readonly position: ApplicationBufferDescriptor;
    readonly [k: string]: unknown | undefined;
}
/**
 * Complete typed layout for a borrowed or copied GPU upload.
 */
export interface ApplicationBufferDescriptor {
    /**
     * Stable identity accepted by the common buffer accessor.
     */
    readonly buffer_id: number;
    /**
     * Components per attribute item.
     */
    readonly components: number;
    /**
     * Attribute item count, rather than scalar count.
     */
    readonly count: number;
    /**
     * Scalar GPU element format.
     */
    readonly data_type: ApplicationBufferType;
    /**
     * Contiguous item stride in bytes.
     */
    readonly stride_bytes: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * Final sunlight placement and aircraft-following shadow volume in render-world axes.
 */
export interface LakeSceneShadow {
    /**
     * Normalized depth bias.
     */
    readonly depth_bias: number;
    /**
     * Surface-normal depth offset in metres.
     */
    readonly normal_bias: number;
    /**
     * Orthographic left/right/bottom/top/near/far bounds in metres.
     *
     * @minItems 6
     * @maxItems 6
     */
    readonly shadow_bounds: readonly [
        number,
        number,
        number,
        number,
        number,
        number
    ];
    /**
     * Daylight-gated shadow rendering.
     */
    readonly shadow_enabled: boolean;
    /**
     * Accepted aircraft datum targeted by the light.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly sun_target_world_position: readonly [
        number,
        number,
        number
    ];
    /**
     * Directional light origin in render East/Up/negative-North metres.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly sun_world_position: readonly [
        number,
        number,
        number
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * Final mirrored camera and its world-to-clip matrix for a horizontal water plane.
 */
export interface PlanarReflection {
    /**
     * Original column-major drawing projection.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly projection: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ];
    /**
     * Column-major projection multiplied by the reflected world-to-camera transform.
     *
     * @minItems 16
     * @maxItems 16
     */
    readonly projection_from_world: readonly [
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number,
        number
    ];
    /**
     * Reflected camera-to-world transform with reflected forward and up directions.
     */
    readonly world_from_camera: Pose;
    readonly [k: string]: unknown | undefined;
}
/**
 * Complete spatial UI produced alongside the same camera observation.
 */
export interface ApplicationSpatialProjection {
    /**
     * Head-referenced smart-glasses instruments.
     */
    readonly head_hud: ApplicationHeadHudProjection;
    /**
     * Retained world/aircraft-referenced menu.
     */
    readonly menu: ApplicationMenuProjection;
    /**
     * Opaque identity to echo after this projection has actually been submitted.
     */
    readonly presentation_id: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * Head-fixed surface whose text ink has passed the binocular readability check.
 */
export interface HeadHudPaintProjection {
    /**
     * Texture allocation required by the drawing observations.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvasSize: readonly [
        number,
        number
    ];
    /**
     * Normalized forward-view reservation.
     */
    readonly clearRegion: GameUiRect;
    /**
     * Ordered drawing commands in surface metres.
     */
    readonly items: readonly SurfacePaintItem[];
    /**
     * Head-to-surface pose.
     */
    readonly localPose: Pose;
    /**
     * Physical drawing extent in metres.
     */
    readonly size: MenuSize;
}
/**
 * Normalized control position for spatial and chart rendering.
 */
export interface GameUiRect {
    /**
     * Height.
     */
    readonly height: number;
    /**
     * Width.
     */
    readonly width: number;
    /**
     * Left edge.
     */
    readonly x: number;
    /**
     * Top edge.
     */
    readonly y: number;
}
/**
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect {
    /**
     * Height in metres.
     */
    readonly height: number;
    /**
     * Width in metres.
     */
    readonly width: number;
    /**
     * Horizontal coordinate in metres.
     */
    readonly x: number;
    /**
     * Vertical coordinate in metres.
     */
    readonly y: number;
}
/**
 * Metric point in a spatial menu document.
 */
export interface MenuPoint {
    /**
     * Horizontal coordinate in metres.
     */
    readonly x: number;
    /**
     * Vertical coordinate in metres.
     */
    readonly y: number;
}
/**
 * Measured surface dimensions in metres.
 */
export interface MenuSize {
    /**
     * Height in metres.
     */
    readonly height: number;
    /**
     * Width in metres.
     */
    readonly width: number;
}
/**
 * Final depth/blending and submission order for one explicit UI anchor.
 */
export interface XrSurfaceStyle {
    /**
     * Enables scene-depth occlusion of the surface.
     */
    readonly depth_test: boolean;
    /**
     * Publishes surface depth for later physical geometry.
     */
    readonly depth_write: boolean;
    /**
     * Shared drawing order within its depth class.
     */
    readonly render_order: number;
    /**
     * Enables alpha blending of the surface texture.
     */
    readonly transparent: boolean;
    readonly [k: string]: unknown | undefined;
}
/**
 * Final metric cursor primitive, independent of any parent surface's nonuniform scale.
 */
export interface XrCursorProjection {
    /**
     * Marker sRGB 0xRRGGBB colour.
     */
    readonly color_srgb: number;
    /**
     * Shared scene-depth admission.
     */
    readonly depth_test: boolean;
    /**
     * Diameter of the circular marker in metres.
     */
    readonly diameter_m: number;
    /**
     * Marker alpha.
     */
    readonly opacity: number;
    /**
     * Shared submission order.
     */
    readonly render_order: number;
    /**
     * Cursor-to-render-world pose, including the small surface-normal separation.
     */
    readonly world_from_cursor: Pose;
    readonly [k: string]: unknown | undefined;
}
/**
 * Spatial menu drawing commands ready for a Canvas or native surface adapter.
 */
export interface ApplicationMenuPaint {
    /**
     * Background colour.
     */
    readonly background: string;
    /**
     * Clipped document painting.
     */
    readonly content: MenuPaintLayer;
    /**
     * Fixed footer painting.
     */
    readonly footer: readonly SurfacePaintItem[];
    /**
     * Background opacity.
     */
    readonly opacity: number;
    /**
     * Surface dimensions in metres.
     */
    readonly size: MenuSize;
    /**
     * Published title.
     */
    readonly title: string;
    readonly [k: string]: unknown | undefined;
}
/**
 * Clipped spatial UI paint layer.
 */
export interface MenuPaintLayer {
    /**
     * Surface-space clipping bounds in metres.
     */
    readonly clip: MenuRect;
    /**
     * Ordered paint operations.
     */
    readonly items: readonly SurfacePaintItem[];
}
/**
 * Pure shared scene and menu projection.
 */
export interface GameUiView {
    /**
     * Shared exclusive overlay name.
     */
    readonly active_overlay?: GameUiOverlay | null;
    /**
     * Contextual Back operation selected from enabled shared controls.
     */
    readonly back_action?: GameUiAction | null;
    /**
     * Shared explanatory text.
     */
    readonly description: string;
    /**
     * Shared telemetry and instrument projection, absent outside a flight or record.
     */
    readonly hud?: GameUiHud | null;
    /**
     * Opaque semantic view identity for focus and scroll ownership.
     */
    readonly interaction_scope: string;
    /**
     * Shared responsive dimensions and scroll policy.
     */
    readonly layout: GameUiLayout;
    /**
     * Ordered semantic panels.
     */
    readonly panels: readonly GameUiPanel[];
    /**
     * Domain-derived scene.
     */
    readonly scene: GameUiScene;
    /**
     * Boundary version.
     */
    readonly schema_version: number;
    /**
     * Shared visual hierarchy.
     */
    readonly theme: GameUiTheme;
    /**
     * Shared heading.
     */
    readonly title: string;
    /**
     * Adapter query-cache revision; pure projections use zero until published.
     */
    readonly view_revision: number;
}
/**
 * Shared Web-baseline HUD model; no adapter rederives flight values or labels.
 */
export interface GameUiHud {
    /**
     * Ordered groups containing every visible readout and instrument.
     */
    readonly groups: readonly GameUiHudGroup[];
    /**
     * Shared Web-baseline instrument placement and responsive dimensions.
     */
    readonly layout: GameUiHudLayout;
}
/**
 * One shared horizontal row; identifiers only track its retained elements.
 */
export interface GameUiHudGroup {
    /**
     * Stable group identity.
     */
    readonly id: string;
    /**
     * Complete visible row in its shared left-to-right order.
     */
    readonly nodes: readonly GameUiHudNode[];
    /**
     * Shared placement relative to the unobstructed forward view.
     */
    readonly placement: GameUiHudPlacement;
}
/**
 * One existing widget instance; adding an instance requires no adapter-specific field.
 */
export interface GameUiHudNode {
    /**
     * Shared normalized rectangle within the bottom HUD rail.
     */
    readonly bounds: GameUiRect;
    /**
     * Exclusive widget geometry and display values.
     */
    readonly content: GameUiHudNodeContent;
    /**
     * Stable identity used only to retain this element.
     */
    readonly id: string;
    /**
     * Instrument drawing height relative to the shared HUD font size; zero for text.
     */
    readonly instrument_height_em: number;
    /**
     * Caption preceding the value in one formatted paragraph, independent of widget type.
     */
    readonly label: string;
}
/**
 * Completed ADI drawing in its fixed logical canvas.
 */
export interface GameUiHudAdiGeometry {
    /**
     * Fixed aircraft reference segments.
     *
     * @minItems 6
     * @maxItems 6
     */
    readonly aircraft_segments: readonly [
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ]
    ];
    /**
     * Complete logical drawing extent.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly canvas_size: readonly [
        number,
        number
    ];
    /**
     * Rounded horizon clipping and outline rectangle.
     */
    readonly clip: GameUiRect;
    /**
     * Radius of the rounded clipping corners.
     */
    readonly clip_radius: number;
    /**
     * Flight-path circle centre, absent when its direction is undefined.
     */
    readonly flight_path_center?: GameUiChartPoint | null;
    /**
     * Flight-path circle radius.
     */
    readonly flight_path_radius: number;
    /**
     * Final horizon marks, absent for a flight-path-only instrument.
     */
    readonly horizon?: GameUiHudAttitudeGeometry | null;
}
/**
 * Two-dimensional display point in logical pixels.
 */
export interface GameUiChartPoint {
    /**
     * Horizontal component.
     */
    readonly x: number;
    /**
     * Vertical component.
     */
    readonly y: number;
}
/**
 * Completed horizon marks and label orientation.
 */
export interface GameUiHudAttitudeGeometry {
    /**
     * Clockwise rotation of each label about its final baseline anchor.
     */
    readonly label_rotation_degrees: number;
    /**
     * Completed pitch ladder, clipped to the rounded horizon region.
     *
     * @minItems 7
     * @maxItems 7
     */
    readonly pitch_ticks: readonly [
        GameUiHudPitchTick,
        GameUiHudPitchTick,
        GameUiHudPitchTick,
        GameUiHudPitchTick,
        GameUiHudPitchTick,
        GameUiHudPitchTick,
        GameUiHudPitchTick
    ];
}
/**
 * One final pitch ladder segment and its optional numeric label.
 */
export interface GameUiHudPitchTick {
    /**
     * Final baseline anchor with horizontal centre alignment, absent for an unlabelled mark.
     */
    readonly label_anchor?: GameUiChartPoint | null;
    /**
     * Absolute pitch in degrees; the horizon has no numeric label.
     */
    readonly label_degrees?: number | null;
    /**
     * Final segment endpoints, absent when the mark is outside the clip.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly segment?: readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ] | null;
}
/**
 * One completed heading tape mark in logical canvas pixels.
 */
export interface GameUiHudHeadingTick {
    /**
     * Compass label, empty for a minor tick.
     */
    readonly label: string;
    /**
     * Centred alphabetic label baseline, present for visible labels.
     */
    readonly label_anchor?: GameUiChartPoint | null;
    /**
     * Tick endpoints.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly segment: readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ];
    /**
     * Tick stroke width in logical pixels.
     */
    readonly stroke_width: number;
}
/**
 * Instrument placement in logical pixels with platform safe-area insets applied by adapters.
 */
export interface GameUiHudLayout {
    /**
     * ADI width and height.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly attitude_size_px: readonly [
        number,
        number
    ];
    /**
     * Readout font size.
     */
    readonly font_size_px: number;
    /**
     * Horizontal and vertical grid gaps.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly gap_px: readonly [
        number,
        number
    ];
    /**
     * Shared rail height enclosing every published node.
     */
    readonly height_px: number;
    /**
     * Left and bottom viewport insets; the bottom edge is the viewport anchor.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly offset_px: readonly [
        number,
        number
    ];
    /**
     * Individual card background opacity; the surrounding rail is transparent.
     */
    readonly opacity: number;
    /**
     * Horizontal and vertical padding.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly padding_px: readonly [
        number,
        number
    ];
    /**
     * Responsive rail width without scrolling.
     */
    readonly width_px: number;
}
/**
 * Responsive shell selected using the Web baseline.
 */
export interface GameUiLayout {
    /**
     * Responsive choice grid count.
     */
    readonly choice_columns: number;
    /**
     * Horizontal overflow is clipped.
     */
    readonly clip_x: boolean;
    /**
     * Small-screen branch selected below the Web 600px breakpoint.
     */
    readonly compact: boolean;
    /**
     * Shared control gap.
     */
    readonly gap_px: number;
    /**
     * Maximum reachable scroll viewport height.
     */
    readonly maximum_height_px: number;
    /**
     * Shared shell padding.
     */
    readonly padding_px: number;
    /**
     * Input-transparent forward view outside explicit controls.
     */
    readonly passthrough: boolean;
    /**
     * Vertical scrolling is admitted for the shell.
     */
    readonly scroll_y: boolean;
    /**
     * Shared shell placement.
     */
    readonly shell: GameUiShell;
    /**
     * Shell width in logical pixels.
     */
    readonly width_px: number;
}
/**
 * Shared ordered menu panel.
 */
export interface GameUiPanel {
    /**
     * Explicit placement anchor.
     */
    readonly anchor: GameUiAnchor;
    /**
     * Ordered controls and readouts.
     */
    readonly controls: readonly GameUiControl[];
    /**
     * Stable current Web identifier.
     */
    readonly id: string;
    /**
     * Local offset in metres, with rendering forward on negative Z.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly local_position: readonly [
        number,
        number,
        number
    ];
    /**
     * Shared ordered presentation sections.
     */
    readonly sections: readonly GameUiSection[];
    /**
     * Spatial width and height in metres.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly size: readonly [
        number,
        number
    ];
    /**
     * Shared heading.
     */
    readonly title: string;
}
/**
 * Shared control, including data and admitted semantic action.
 */
export interface GameUiControl {
    /**
     * Exclusive control data.
     */
    readonly content: GameUiControlContent;
    /**
     * Shared admission flag.
     */
    readonly enabled: boolean;
    /**
     * Stable current Web control identifier.
     */
    readonly id: string;
    /**
     * Shared visible label.
     */
    readonly label: string;
    /**
     * Section identifier.
     */
    readonly section: string;
}
/**
 * One shared formatted numeric measurement.
 */
export interface GameUiNumericField {
    /**
     * Stable measurement identity.
     */
    readonly id: string;
    /**
     * Measurement label.
     */
    readonly label: string;
    /**
     * Unit displayed next to the value.
     */
    readonly unit: string;
    /**
     * Shared formatted numeric value.
     */
    readonly value: string;
}
/**
 * Complete engine-independent recorded chart.
 */
export interface GameUiChart {
    /**
     * Record-interpolated cursor positions.
     */
    readonly cursor_points: readonly GameUiChartCursorPoint[];
    /**
     * Horizontal cursor position in logical pixels; absent for maps.
     */
    readonly cursor_x?: number | null;
    /**
     * Preserves metre-for-metre map scale.
     */
    readonly equal_axis_scale: boolean;
    /**
     * Launch, terminal and venue markers.
     */
    readonly markers: readonly GameUiChartMarker[];
    /**
     * Shared chart layout and labels in logical display pixels.
     */
    readonly presentation: GameUiChartPresentation;
    /**
     * Horizontal reference lines.
     */
    readonly reference_lines: readonly GameUiChartReferenceLine[];
    /**
     * Ordered curves in logical display pixels, including venue geometry.
     */
    readonly series: readonly GameUiChartSeries[];
    /**
     * Recorded-time map markers.
     */
    readonly time_markers: readonly GameUiChartMarker[];
    /**
     * Clipped ambient-wind arrows.
     */
    readonly vectors: readonly GameUiChartVector[];
    /**
     * Horizontal axis label and unit.
     */
    readonly x_axis_label: string;
    /**
     * Horizontal upper bound.
     */
    readonly x_maximum: number;
    /**
     * Horizontal lower bound.
     */
    readonly x_minimum: number;
    /**
     * Vertical axis label and unit.
     */
    readonly y_axis_label: string;
    /**
     * Vertical upper bound.
     */
    readonly y_maximum: number;
    /**
     * Vertical lower bound.
     */
    readonly y_minimum: number;
}
/**
 * Record-interpolated position with its shared series colour.
 */
export interface GameUiChartCursorPoint {
    /**
     * Colour of the corresponding shared series.
     */
    readonly color: string;
    /**
     * Position in logical display pixels.
     */
    readonly point: GameUiChartPoint;
}
/**
 * One labelled chart location.
 */
export interface GameUiChartMarker {
    /**
     * Shared colour.
     */
    readonly color: string;
    /**
     * Shared label.
     */
    readonly label: string;
    /**
     * Position in logical display pixels.
     */
    readonly point: GameUiChartPoint;
}
/**
 * Shared static plot geometry and Web-baseline typography.
 */
export interface GameUiChartPresentation {
    /**
     * Shared axis stroke colour.
     */
    readonly axes_color: string;
    /**
     * Axis font size in logical pixels.
     */
    readonly axis_font_size: number;
    /**
     * Shared grid stroke colour.
     */
    readonly grid_color: string;
    /**
     * Static grid segments in descriptor coordinates.
     */
    readonly grid_lines: readonly (readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ])[];
    /**
     * Grid stroke opacity.
     */
    readonly grid_opacity: number;
    /**
     * Ordered shared legend entries.
     */
    readonly legend: readonly GameUiChartLegend[];
    /**
     * Legend font size in root-font units.
     */
    readonly legend_font_rem: number;
    /**
     * Legend item gap in root-font units.
     */
    readonly legend_gap_rem: number;
    /**
     * Gradient swatch corner radius in root-font units.
     */
    readonly legend_gradient_radius_rem: number;
    /**
     * Gradient legend swatch dimensions in root-font units.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly legend_gradient_rem: readonly [
        number,
        number
    ];
    /**
     * Gap after each legend swatch in root-font units.
     */
    readonly legend_swatch_gap_rem: number;
    /**
     * Solid legend swatch dimensions in root-font units.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly legend_swatch_rem: readonly [
        number,
        number
    ];
    /**
     * Metre scale and north reference for equal-scale maps.
     */
    readonly map_guides?: GameUiChartMapGuides | null;
    /**
     * Marker labels in the same order as the chart markers.
     */
    readonly marker_labels: readonly GameUiChartText[];
    /**
     * Minimum usable measured dimensions in logical pixels.
     */
    readonly measurement_minimum: GameUiChartExtent;
    /**
     * Plot rectangle ordered left, top, right, bottom.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly plot: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Reference labels in the same order as the reference lines.
     */
    readonly reference_labels: readonly GameUiChartText[];
    /**
     * Keeps caption, viewport and legend inside one scrollable chart host.
     */
    readonly scroll_overflow: boolean;
    /**
     * Allocated chart height in logical pixels, independent of its measured extent.
     */
    readonly target_height_px: number;
    /**
     * Tick font size in logical pixels.
     */
    readonly tick_font_size: number;
    /**
     * Recorded-time labels in the same order as the time markers.
     */
    readonly time_marker_labels: readonly GameUiChartText[];
    /**
     * Coordinate extent of this descriptor.
     */
    readonly viewport: GameUiChartExtent;
    /**
     * Minimum viewport width and height in root-font units.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly viewport_minimum_rem: readonly [
        number,
        number
    ];
    /**
     * Horizontal axis title.
     */
    readonly x_axis: GameUiChartText;
    /**
     * Ordered horizontal ticks.
     */
    readonly x_ticks: readonly GameUiChartTick[];
    /**
     * Vertical axis title.
     */
    readonly y_axis: GameUiChartText;
    /**
     * Ordered vertical ticks.
     */
    readonly y_ticks: readonly GameUiChartTick[];
}
/**
 * One ordered legend item with an optional progress gradient.
 */
export interface GameUiChartLegend {
    /**
     * Solid swatch colour.
     */
    readonly color: string;
    /**
     * Shared label.
     */
    readonly label: string;
    /**
     * First and last progress colours when the series uses a gradient.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly segment_gradient?: readonly [
        string,
        string
    ] | null;
    /**
     * Shared provider for labels that follow the selected record cursor.
     */
    readonly source: GameUiReadoutSource;
}
/**
 * Static map references in descriptor coordinates.
 */
export interface GameUiChartMapGuides {
    /**
     * North label.
     */
    readonly north_label: GameUiChartText;
    /**
     * North arrow shaft and its two head segments.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly north_segments: readonly [
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ]
    ];
    /**
     * Represented distance in metres.
     */
    readonly scale_distance_m: number;
    /**
     * Metre scale label.
     */
    readonly scale_label: GameUiChartText;
    /**
     * Scale bar and its two end caps.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly scale_segments: readonly [
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ],
        readonly [
            GameUiChartPoint,
            GameUiChartPoint
        ]
    ];
}
/**
 * Static label placement independent of the rendering engine.
 */
export interface GameUiChartText {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: GameUiChartTextAnchor;
    /**
     * Shared text colour.
     */
    readonly color: string;
    /**
     * Font size in logical pixels.
     */
    readonly font_size: number;
    /**
     * CSS-compatible font weight.
     */
    readonly font_weight: number;
    /**
     * Already formatted content.
     */
    readonly label: string;
    /**
     * Alphabetic-baseline position in logical pixels; adapters apply actual font metrics.
     */
    readonly position: GameUiChartPoint;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Tick value, placement and already formatted label.
 */
export interface GameUiChartTick {
    /**
     * Increasing-axis fraction between zero and one.
     */
    readonly ratio: number;
    /**
     * Shared text and placement.
     */
    readonly text: GameUiChartText;
    /**
     * Axis coordinate.
     */
    readonly value: number;
}
/**
 * One horizontal plot reference.
 */
export interface GameUiChartReferenceLine {
    /**
     * Shared colour.
     */
    readonly color: string;
    /**
     * Shared label.
     */
    readonly label: string;
    /**
     * Vertical position in logical display pixels.
     */
    readonly position_y_px: number;
}
/**
 * One curve and its shared visual identity.
 */
export interface GameUiChartSeries {
    /**
     * Shared base colour.
     */
    readonly color: string;
    /**
     * Legend label.
     */
    readonly label: string;
    /**
     * Ordered positions in logical display pixels.
     */
    readonly points: readonly GameUiChartPoint[];
    /**
     * Per-segment colours; empty means the base colour.
     */
    readonly segment_colors: readonly string[];
}
/**
 * Clipped vector with simultaneously present or absent endpoints.
 */
export interface GameUiChartVector {
    /**
     * Shared colour.
     */
    readonly color: string;
    /**
     * Legend label.
     */
    readonly label: string;
    /**
     * Clipped body and arrowhead lines in logical pixels; empty for zero wind.
     */
    readonly segments: readonly (readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ])[];
}
/**
 * Shared control group and placement mode.
 */
export interface GameUiSection {
    /**
     * Stable group identifier.
     */
    readonly id: string;
    /**
     * Optional visible group heading.
     */
    readonly label: string;
    /**
     * Shared layout mode.
     */
    readonly layout: GameUiSectionLayout;
}
/**
 * Engine-independent colors, opacity and minimum hit dimensions.
 */
export interface GameUiTheme {
    /**
     * Ordinary button minimum height.
     */
    readonly button_min_height_px: number;
    /**
     * Unselected choice fill.
     */
    readonly choice_background: string;
    /**
     * Keyboard focus outline.
     */
    readonly focus_outline: string;
    /**
     * Head-relative HUD background opacity, preserving forward visibility.
     */
    readonly hud_opacity: number;
    /**
     * Secondary text color.
     */
    readonly muted_text: string;
    /**
     * Ordinary action fill.
     */
    readonly normal_background: string;
    /**
     * Panel background color.
     */
    readonly panel_background: string;
    /**
     * Centered menu opacity.
     */
    readonly panel_opacity: number;
    /**
     * Primary or selected action fill.
     */
    readonly primary_background: string;
    /**
     * Principal button minimum height.
     */
    readonly primary_min_height_px: number;
    /**
     * Main text color.
     */
    readonly text: string;
}
/**
 * Lightweight per-frame UI observation associated with one published structural view.
 */
export interface GameUiFrame {
    /**
     * Replay clock play/pause label.
     */
    readonly button_labels: readonly GameUiButtonLabel[];
    /**
     * Cursor-only chart observation; plotted series and bounds remain in the view.
     */
    readonly chart_cursor?: GameUiChartCursor | null;
    /**
     * Numeric readouts that advance with the retained record cursor.
     */
    readonly fields: readonly GameUiFieldsValue[];
    /**
     * Shared HUD projection, never a second physical state.
     */
    readonly hud?: GameUiHud | null;
    /**
     * Existing range values that advance without a structural change.
     */
    readonly ranges: readonly GameUiRangeValue[];
    /**
     * Fixed record-clock and analysis-cursor readouts.
     */
    readonly readouts: readonly GameUiReadoutValue[];
    /**
     * Structural view revision retained by the query adapter.
     */
    readonly view_revision: number;
}
/**
 * Label of the retained replay play/pause action.
 */
export interface GameUiButtonLabel {
    /**
     * Retained control identity.
     */
    readonly id: string;
    /**
     * Shared clock-derived label.
     */
    readonly label: string;
}
/**
 * Lightweight cursor and selected-sample wind for the retained plot bounds.
 */
export interface GameUiChartCursor {
    /**
     * Shared plot-space sample points.
     */
    readonly cursor_points: readonly GameUiChartCursorPoint[];
    /**
     * Horizontal position in logical display pixels, absent for a map.
     */
    readonly cursor_x?: number | null;
    /**
     * Retained chart control identity.
     */
    readonly id: string;
    /**
     * Selected-sample wind, replacing the first map vector only.
     */
    readonly selected_wind_vector?: GameUiChartVector | null;
}
/**
 * Numeric values for a retained field group.
 */
export interface GameUiFieldsValue {
    /**
     * Shared ordered measurements.
     */
    readonly fields: readonly GameUiNumericField[];
    /**
     * Retained group identity.
     */
    readonly id: string;
}
/**
 * Value of a retained typed range control.
 */
export interface GameUiRangeValue {
    /**
     * Shared one-decimal display text.
     */
    readonly display_value: string;
    /**
     * Retained control identity.
     */
    readonly id: string;
    /**
     * Shared finite range value.
     */
    readonly value: number;
}
/**
 * Text of a retained record readout.
 */
export interface GameUiReadoutValue {
    /**
     * Retained control identity.
     */
    readonly id: string;
    /**
     * Shared formatted record or clock observation.
     */
    readonly value: string;
}
/**
 * Existing Personal Best index entry.
 */
export interface PersistedBest {
    /**
     * Absent for an index written before comparison versioning.
     */
    readonly comparisonVersion?: number | null;
    /**
     * Canonical comparison key.
     */
    readonly key: string;
    /**
     * Selected record key.
     */
    readonly recordId: number;
}
/**
 * Revision-scoped drawing resources prepared by the shared application owner.
 */
export interface ApplicationEnvironmentAssets {
    /**
     * Final light and fog projection.
     */
    readonly lighting: ApplicationLightingAssets;
    /**
     * The resource-preparation request owning this bundle.
     */
    readonly revision: number;
    /**
     * First immutable scene manifest; later preparations reuse the same owner-issued identities.
     */
    readonly scene?: ApplicationGeometryAssets | null;
    /**
     * Equirectangular sRGB panorama.
     */
    readonly sky: ApplicationImageAsset;
    /**
     * Shared registration decision for venue geometry.
     */
    readonly venue: EnvironmentVenueProjection;
    /**
     * Final water uniforms and data images.
     */
    readonly water: ApplicationWaterAssets;
    readonly [k: string]: unknown | undefined;
}
/**
 * Final display-light and distance-fog values in the shared render convention.
 */
export interface ApplicationLightingAssets {
    /**
     * Ambient sRGB 0xRRGGBB colour.
     */
    readonly ambient_color_srgb: number;
    /**
     * Hemispherical irradiance scale.
     */
    readonly ambient_irradiance: number;
    /**
     * Airlight sRGB 0xRRGGBB colour.
     */
    readonly haze_color_srgb: number;
    /**
     * Near/far distance-fog bounds in metres.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly haze_range_meters: readonly [
        number,
        number
    ];
    /**
     * Candidate-local initial light pose and shadow bounds for GPU pipeline preparation.
     */
    readonly initial_shadow?: LakeSceneShadow | null;
    /**
     * Sunlight sRGB 0xRRGGBB colour.
     */
    readonly sunlight_color_srgb: number;
    /**
     * Render-world sun direction XYZ and daylight enable W for direct shader submission.
     *
     * @minItems 4
     * @maxItems 4
     */
    readonly sunlight_direction: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * Daylight-gated directional irradiance scale.
     */
    readonly sunlight_irradiance: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * Immutable registered meshes and completed wing images.
 */
export interface ApplicationGeometryAssets {
    /**
     * Static triangle topology in common GPU attribute form.
     */
    readonly meshes: readonly ApplicationMeshAsset[];
    /**
     * Left and right completed wing textures.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly wing_textures: readonly [
        ApplicationGeometryTexture,
        ApplicationGeometryTexture
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * Static topology and final attributes for one scene mesh.
 */
export interface ApplicationMeshAsset {
    /**
     * Final transform parent.
     */
    readonly attachment: ApplicationMeshAttachment;
    /**
     * Optional linear vertex colour.
     */
    readonly color?: ApplicationBufferDescriptor | null;
    /**
     * Culling is disabled for articulated or deforming aircraft meshes.
     */
    readonly frustum_culled: boolean;
    /**
     * Surface-local UInt32 triangle indices.
     */
    readonly index: ApplicationBufferDescriptor;
    /**
     * Final local transform; positions already use right/up/aft or render-world axes.
     */
    readonly local_pose: Pose;
    /**
     * Contiguous material ranges.
     */
    readonly materials: readonly ApplicationMeshMaterial[];
    /**
     * Stable drawing identity.
     */
    readonly mesh_id: number;
    /**
     * Shared diagnostic name.
     */
    readonly name: string;
    /**
     * Final unit normals.
     */
    readonly normal: ApplicationBufferDescriptor;
    /**
     * Float32 triangle vertices.
     */
    readonly position: ApplicationBufferDescriptor;
    /**
     * Optional texture coordinates.
     */
    readonly uv?: ApplicationBufferDescriptor | null;
    readonly [k: string]: unknown | undefined;
}
/**
 * Engine-independent final material values for a triangle range.
 */
export interface ApplicationMeshMaterial {
    /**
     * Cast an opaque shadow.
     */
    readonly cast_shadow: boolean;
    /**
     * Opaque sRGB material colour.
     */
    readonly color_srgb: number;
    /**
     * Number of indices in this range.
     */
    readonly count: number;
    /**
     * Direct depth-buffer write flag.
     */
    readonly depth_write: boolean;
    /**
     * Render both triangle orientations.
     */
    readonly double_sided: boolean;
    /**
     * Apply scene illumination.
     */
    readonly lit: boolean;
    /**
     * Material alpha.
     */
    readonly opacity: number;
    /**
     * Receive scene shadows.
     */
    readonly receive_shadow: boolean;
    /**
     * First triangle index in the shared mesh index buffer.
     */
    readonly start: number;
    /**
     * Optional shared wing texture identity.
     */
    readonly texture_buffer_id?: number | null;
    /**
     * Direct transparent-pipeline flag.
     */
    readonly transparent: boolean;
    readonly [k: string]: unknown | undefined;
}
/**
 * Completed static RGBA image including its full linear-light mip chain.
 */
export interface ApplicationGeometryTexture {
    /**
     * Concatenated RGBA8 mip levels, largest first.
     */
    readonly buffer: ApplicationBufferDescriptor;
    /**
     * Complete mip level count.
     */
    readonly mip_level_count: number;
    /**
     * Base image texel dimensions.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly size: readonly [
        number,
        number
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * An immutable image descriptor; bytes travel through the common asset-buffer channel.
 */
export interface ApplicationImageAsset {
    /**
     * Owner-issued buffer identity, stable while the same image is reused.
     */
    readonly buffer_id: number;
    /**
     * Pixel interpretation at upload.
     */
    readonly encoding: ApplicationImageEncoding;
    /**
     * Texture dimensions in texels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly size: readonly [
        number,
        number
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * Prepared wave uniforms and immutable detail images.
 */
export interface ApplicationWaterAssets {
    /**
     * Near/far periodic images; flat water uses a shared neutral texel.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly detail: readonly [
        ApplicationWaterDetailAsset,
        ApplicationWaterDetailAsset
    ];
    /**
     * Downstream detail multiplier; zero denotes the flat-water case.
     */
    readonly detail_amplitude_scale: number;
    /**
     * Complete final-axis water meshes and conservative culling bounds.
     */
    readonly geometry: ApplicationWaterGeometry;
    /**
     * Dark, mid-tone and bright colours for direct material submission.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly palette_srgb: readonly [
        number,
        number,
        number
    ];
    /**
     * Complete GPU wave coefficients, counts and non-inversion bounds.
     */
    readonly projection: LakeWaveProjection;
    /**
     * Wind magnitude used by the shared visual condition.
     */
    readonly wind_speed_meters_per_second: number;
    /**
     * Final render X/Z wind velocity for texture drift and micro-normal sampling.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly wind_velocity_render: readonly [
        number,
        number
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * One prepared periodic normal/variance/height image.
 */
export interface ApplicationWaterDetailAsset {
    /**
     * World-space repeat length in metres.
     */
    readonly extent_meters: number;
    /**
     * Linear-data RGBA texture.
     */
    readonly image: ApplicationImageAsset;
    readonly [k: string]: unknown | undefined;
}
/**
 * Complete water geometry selected by the shared quality policy.
 */
export interface ApplicationWaterGeometry {
    /**
     * Final far-water inner clip half-extent.
     */
    readonly backing_inner_extent_meters: number;
    /**
     * Shared horizontal/vertical displacement budgets.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly displacement_meters: readonly [
        number,
        number
    ];
    /**
     * Distant backing geometry.
     */
    readonly far: ApplicationWaterChunkAsset;
    /**
     * Sixteen near chunks, in row-major order.
     */
    readonly near: readonly ApplicationWaterChunkAsset[];
    readonly [k: string]: unknown | undefined;
}
/**
 * One complete immutable water mesh and weather-expanded culling volume.
 */
export interface ApplicationWaterChunkAsset {
    /**
     * Final culling volume including the shared displacement reserve.
     */
    readonly bounds: ApplicationWaterBounds;
    /**
     * Whether the engine should apply view-frustum culling.
     */
    readonly frustum_culled: boolean;
    /**
     * Shared local spacing consumed by the wave shader.
     */
    readonly grid_spacing: ApplicationBufferDescriptor;
    /**
     * Positive-Y triangle indices.
     */
    readonly index: ApplicationBufferDescriptor;
    /**
     * Upward base normals.
     */
    readonly normal: ApplicationBufferDescriptor;
    /**
     * Final render X/0/Z positions.
     */
    readonly position: ApplicationBufferDescriptor;
    /**
     * Whole-patch texture coordinates.
     */
    readonly uv: ApplicationBufferDescriptor;
    readonly [k: string]: unknown | undefined;
}
/**
 * Completed culling volume in water-local render axes.
 */
export interface ApplicationWaterBounds {
    /**
     * Inclusive maximum coordinates.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly maximum: readonly [
        number,
        number,
        number
    ];
    /**
     * Inclusive minimum coordinates.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly minimum: readonly [
        number,
        number,
        number
    ];
    /**
     * Final bounding-sphere centre.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly sphere_center: readonly [
        number,
        number,
        number
    ];
    /**
     * Final bounding-sphere radius.
     */
    readonly sphere_radius: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * Canonical uploads and derivative bounds for the existing finite Web wave packets.
 */
export interface LakeWaveProjection {
    readonly choppiness: number;
    readonly horizontalDerivativeBound: number;
    readonly spacing: LakeMeshSpacingDomain;
    readonly visualWaveHeight: number;
    readonly waveCount: number;
    /**
     * @minItems 24
     * @maxItems 24
     */
    readonly waveKAmplitude: readonly [
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ]
    ];
    /**
     * @minItems 24
     * @maxItems 24
     */
    readonly waveOmegaPhase: readonly [
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ],
        readonly [
            number,
            number,
            number,
            number
        ]
    ];
    readonly [k: string]: unknown | undefined;
}
/**
 * Float32-compatible mesh spacing domain used by the bounded wave packet projection.
 */
export interface LakeMeshSpacingDomain {
    readonly gradientMagnitude: number;
    readonly maximum: number;
    readonly minimum: number;
    readonly [k: string]: unknown | undefined;
}
/**
 * Exact split-seed scenario identity used by aggregate resource preparation.
 */
export interface EnvironmentIdentity {
    /**
     * Aircraft model version.
     */
    readonly aircraft_model_version: number;
    /**
     * Registered catalog version.
     */
    readonly catalog_version: number;
    /**
     * Controller profile version.
     */
    readonly controller_profile_version: number;
    /**
     * Environment content version.
     */
    readonly environment_version: number;
    /**
     * Scenario identifier; registry lookup applies its own nonzero constraint.
     */
    readonly scenario_id: number;
    /**
     * Scenario content version.
     */
    readonly scenario_version: number;
    /**
     * High 32 bits of the exact seed.
     */
    readonly seed_high: number;
    /**
     * Low 32 bits of the exact seed.
     */
    readonly seed_low: number;
}
/**
 * Final desired resource configuration, independent of a confirmed commit counter.
 */
export interface ApplicationGraphicsTarget {
    /**
     * Exact target framebuffer dimensions in physical pixels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly extent_px: readonly [
        number,
        number
    ];
    /**
     * Complete resource quality selected by the shared policy.
     */
    readonly quality: ApplicationRenderQuality;
    /**
     * Final reflection allocation in physical pixels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly reflection_extent_px: readonly [
        number,
        number
    ];
    /**
     * Graphics context or device lifetime.
     */
    readonly renderer_resource_id: number;
    /**
     * Final square shadow allocation in physical pixels.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly shadow_extent_px: readonly [
        number,
        number
    ];
    readonly [k: string]: unknown | undefined;
}
