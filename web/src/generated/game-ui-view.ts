/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Exclusive current overlay names retained by both renderer adapters.
 */
export type GameUiOverlay = "Pause" | "PauseSettings" | "PauseHelp";
/**
 * Pure shared scene and menu projection.
 */
export interface GameUiView {
    /**
     * Shared exclusive overlay name.
     */
    readonly active_overlay: GameUiOverlay | null;
    /**
     * Shared explanatory text.
     */
    readonly description: string;
    /**
     * Shared telemetry and instrument projection, absent outside a flight or record.
     */
    readonly hud: GameUiHud | null;
    readonly layout: GameUiLayout;
    /**
     * Ordered semantic panels.
     */
    readonly panels: readonly GameUiPanel[];
    /**
     * Domain-derived scene.
     */
    readonly scene: "Boot" | "Title" | "FlightSetup" | "Briefing" | "Countdown" | "Flight" | "Result" | "Replay";
    /**
     * Boundary version.
     */
    readonly schema_version: number;
    readonly selections: GameUiState;
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
     * AoA text, including explicit missing-angle marker.
     */
    readonly angle_of_attack: string | null;
    /**
     * Defined angle of attack in degrees.
     */
    readonly angle_of_attack_degrees: number | null;
    /**
     * AoA instrument label.
     */
    readonly angle_of_attack_label: string;
    /**
     * ADI input already converted into display degrees.
     */
    readonly attitude: GameUiHudAttitude | null;
    /**
     * Canonical input instructions.
     */
    readonly controls_description: string;
    /**
     * Flight path indicator angle.
     */
    readonly flight_path_angle_degrees: number | null;
    /**
     * Heading instrument text.
     */
    readonly heading: string | null;
    /**
     * True heading in [0,360).
     */
    readonly heading_degrees: number | null;
    /**
     * Heading instrument label.
     */
    readonly heading_label: string;
    readonly layout: GameUiHudLayout;
    /**
     * Local-frame geographic display.
     */
    readonly location: string;
    /**
     * Preserved map attribution.
     */
    readonly map_attribution: string;
    /**
     * Pilot displacement instrument text.
     */
    readonly pilot_position: string | null;
    /**
     * Pilot instrument label.
     */
    readonly pilot_position_label: string;
    /**
     * Primary readouts with shared wording and precision.
     */
    readonly readouts: string;
    /**
     * Domain-derived state label.
     */
    readonly status: string;
    /**
     * Angular rates, physical incidences and pilot target text.
     */
    readonly supplementary_readouts: readonly string[];
    /**
     * Distance, time and ground-speed text.
     */
    readonly telemetry: string;
    /**
     * Terminal or model warning.
     */
    readonly warning: string | null;
    /**
     * NED wind readout.
     */
    readonly wind: string | null;
    /**
     * Ambient air movement direction in [0,360).
     */
    readonly wind_direction_degrees: number | null;
    /**
     * Wind instrument label.
     */
    readonly wind_label: string;
}
/**
 * Shared ADI orientation input.
 */
export interface GameUiHudAttitude {
    /**
     * Pitch angle in degrees.
     */
    readonly pitch_degrees: number;
    /**
     * Bank angle in degrees.
     */
    readonly roll_degrees: number;
}
/**
 * Shared Web-baseline instrument placement and responsive dimensions.
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
     * Keyboard description visibility on the selected viewport.
     */
    readonly controls_visible: boolean;
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
     * Heading, pilot, wind and AoA instrument column count.
     */
    readonly instrument_columns: number;
    /**
     * Reachable vertical scroll height.
     */
    readonly maximum_height_px: number;
    /**
     * Left and top viewport offsets.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly offset_px: readonly [
        number,
        number
    ];
    /**
     * Panel background opacity.
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
     * Responsive panel width.
     */
    readonly width_px: number;
}
/**
 * Shared responsive dimensions and scroll policy.
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
    readonly shell: "center" | "flight_bar" | "pause_settings" | "analysis" | "cinematic";
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
    readonly anchor: "menu" | "head" | "cockpit" | "world";
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
    readonly content: {
        /**
         * Typed semantic action.
         */
        readonly action: {
            /**
             * Operation admitted by the current view.
             */
            readonly command: {
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
                readonly preset: "beginner" | "standard" | "expert" | "realistic" | "custom";
            } | {
                /**
                 * Existing information level.
                 */
                readonly information: "full" | "standard" | "minimal" | "realistic" | "custom";
                readonly kind: "select_information";
            } | {
                /**
                 * Existing assistance level.
                 */
                readonly assistance: "strong" | "assisted" | "light" | "manual";
                readonly kind: "select_assistance";
            } | {
                readonly kind: "select_weather";
                /**
                 * Existing weather class.
                 */
                readonly weather: "calm" | "mild" | "typical" | "challenging" | "near_limit";
            } | {
                /**
                 * Existing cue.
                 */
                readonly cue: "telemetry" | "attitude" | "wind" | "flight_path" | "angle_of_attack" | "warnings";
                readonly kind: "set_cue";
                /**
                 * Requested visibility.
                 */
                readonly visible: boolean;
            };
            readonly kind: "session";
        } | {
            /**
             * Explicit capability-bound operation.
             */
            readonly command: {
                readonly kind: "set_water_quality";
                /**
                 * Requested quality.
                 */
                readonly quality: "low" | "medium" | "high";
            } | {
                readonly kind: "set_presentation";
                /**
                 * Requested mode.
                 */
                readonly mode: "screen" | "webxr" | "phone_vr";
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
                readonly camera: "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
                readonly kind: "set_replay_camera";
            };
            readonly kind: "platform";
        } | {
            readonly kind: "toggle_technical";
        } | {
            readonly kind: "set_result_tab";
            /**
             * Requested tab.
             */
            readonly tab: "summary" | "analysis";
        } | {
            /**
             * Requested plot.
             */
            readonly chart: "map" | "altitude" | "speed";
            readonly kind: "set_analysis_chart";
        } | {
            readonly kind: "set_pause_overlay";
            /**
             * Requested overlay.
             */
            readonly overlay: "menu" | "settings" | "help";
        } | {
            readonly kind: "set_replay_view";
            /**
             * Requested view.
             */
            readonly view: "cinematic" | "telemetry" | "analysis";
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
        readonly kind: "button";
        /**
         * Shared visual hierarchy.
         */
        readonly presentation: {
            /**
             * Primary or secondary emphasis.
             */
            readonly emphasis: "primary" | "secondary" | "normal";
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
    } | {
        /**
         * Cue identifier from the existing format model.
         */
        readonly cue: "telemetry" | "attitude" | "wind" | "flight_path" | "angle_of_attack" | "warnings";
        readonly kind: "toggle";
        /**
         * Current Rust-owned visibility.
         */
        readonly value: boolean;
    } | {
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
        readonly target: "analysis" | "replay";
        /**
         * Current time.
         */
        readonly value: number;
    } | {
        readonly kind: "status";
        /**
         * Shared formatted content.
         */
        readonly value: string;
    } | {
        /**
         * Plot selection.
         */
        readonly chart: "map" | "altitude" | "speed";
        readonly kind: "chart";
        readonly plot: GameUiChart;
    };
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
    readonly rect: GameUiRect;
    /**
     * Section identifier.
     */
    readonly section: string;
}
/**
 * Fully derived axes, geometry and cursor; adapters render this value.
 */
export interface GameUiChart {
    /**
     * Record-interpolated cursor positions.
     */
    readonly cursor_points: readonly GameUiChartPoint[];
    /**
     * Time cursor; maps use cursor points only.
     */
    readonly cursor_x: number | null;
    /**
     * Preserves metre-for-metre map scale.
     */
    readonly equal_axis_scale: boolean;
    /**
     * Launch, terminal and venue markers.
     */
    readonly markers: readonly GameUiChartMarker[];
    /**
     * Horizontal reference lines.
     */
    readonly reference_lines: readonly GameUiChartReferenceLine[];
    /**
     * Ordered curves including registered venue geometry.
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
 * One chart-space point with units defined by the axes.
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
    readonly point: GameUiChartPoint1;
}
/**
 * One chart-space point with units defined by the axes.
 */
export interface GameUiChartPoint1 {
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
     * Vertical-axis coordinate.
     */
    readonly value: number;
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
     * Ordered chart coordinates.
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
     * Visible segment, absent for zero wind or outside the chart.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly segment: readonly [
        GameUiChartPoint,
        GameUiChartPoint
    ] | null;
}
/**
 * Shared spatial or normalized position.
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
    readonly layout: "column" | "choice_grid" | "row" | "normalized";
}
/**
 * Readonly observation of the Rust-owned UI selections.
 */
export interface GameUiState {
    /**
     * Selected record plot.
     */
    readonly analysis_chart: "map" | "altitude" | "speed";
    /**
     * Read-only analysis query position in seconds.
     */
    readonly analysis_time_seconds: number;
    /**
     * Paused menu selection.
     */
    readonly pause_overlay: "menu" | "settings" | "help";
    /**
     * Shared replay camera selection; adapters only apply its geometry.
     */
    readonly replay_camera: "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
    /**
     * Replay presentation selection.
     */
    readonly replay_view: "cinematic" | "telemetry" | "analysis";
    /**
     * Result presentation selection.
     */
    readonly result_tab: "summary" | "analysis";
    /**
     * Expanded preparation diagnostics.
     */
    readonly technical_open: boolean;
}
/**
 * Shared visual hierarchy.
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
