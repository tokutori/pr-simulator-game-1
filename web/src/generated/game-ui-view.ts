/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Exclusive current overlay names retained by both renderer adapters.
 */
export type GameUiOverlay = "Credits" | "Pause" | "PauseSettings" | "PauseHelp";
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
     * Ordered groups containing every visible readout and instrument.
     */
    readonly groups: readonly GameUiHudGroup[];
    readonly layout: GameUiHudLayout;
}
/**
 * A shared HUD group; identifiers track retained nodes without encoding their meaning.
 */
export interface GameUiHudGroup {
    /**
     * Stable group identity.
     */
    readonly id: string;
    /**
     * Shared row, column, or responsive grid composition.
     */
    readonly layout: {
        readonly kind: "row";
    } | {
        readonly kind: "column";
    } | {
        /**
         * Preferred column count before the adapter applies measured width constraints.
         */
        readonly columns: number;
        readonly kind: "grid";
    };
    /**
     * Complete visible node sequence.
     */
    readonly nodes: readonly GameUiHudNode[];
    /**
     * Shared placement relative to the unobstructed forward view.
     */
    readonly placement: "readouts" | "instruments" | "details";
}
/**
 * One existing widget instance; adding an instance requires no adapter-specific field.
 */
export interface GameUiHudNode {
    /**
     * Exclusive widget geometry and display values.
     */
    readonly content: {
        readonly kind: "text";
        /**
         * Shared visual priority.
         */
        readonly tone: "normal" | "muted" | "warning";
        /**
         * Fully formatted text; adapters never parse its wording.
         */
        readonly value: string;
    } | {
        /**
         * Shared horizon and marker geometry.
         */
        readonly geometry: {
            /**
             * Flight path marker position relative to the ADI height.
             */
            readonly flight_path_y_ratio: number | null;
            readonly horizon: GameUiHudAttitudeGeometry;
            readonly kind: "attitude";
        } | {
            readonly kind: "flight_path_only";
            /**
             * Flight path marker position relative to the ADI height.
             */
            readonly y_ratio: number;
        };
        readonly kind: "attitude";
        /**
         * Fully formatted pitch, roll, or flight-path reading.
         */
        readonly value: string;
    } | {
        readonly kind: "heading_tape";
        /**
         * Ordered normalized marks and labels.
         */
        readonly ticks: readonly GameUiHudHeadingTick[];
        /**
         * Formatted central reading.
         */
        readonly value: string;
    } | {
        readonly kind: "direction";
        /**
         * Clockwise display rotation, absent when no direction is defined.
         */
        readonly rotation_degrees: number | null;
        /**
         * Formatted direction and magnitude reading.
         */
        readonly value: string;
    } | {
        readonly kind: "linear_gauge";
        /**
         * Right endpoint label.
         */
        readonly maximum_label: string;
        /**
         * Left endpoint label.
         */
        readonly minimum_label: string;
        /**
         * Normalized marker position, absent when the reading is undefined.
         */
        readonly position_ratio: number | null;
        /**
         * Formatted reading including its unit.
         */
        readonly value: string;
    };
    /**
     * Stable identity used only to retain this element.
     */
    readonly id: string;
    /**
     * Shared heading, independent of the widget type.
     */
    readonly label: string;
}
/**
 * Horizon rotation and translation.
 */
export interface GameUiHudAttitudeGeometry {
    /**
     * Pre-rotation pitch translation relative to the ADI height.
     */
    readonly pitch_offset_ratio: number;
    /**
     * Clockwise horizon rotation in display coordinates.
     */
    readonly rotation_degrees: number;
}
/**
 * One heading tape mark in normalized instrument coordinates.
 */
export interface GameUiHudHeadingTick {
    /**
     * Compass label, empty for a minor tick.
     */
    readonly label: string;
    /**
     * Major marks receive the long tick and a compass label.
     */
    readonly major: boolean;
    /**
     * Position relative to the complete tape width.
     */
    readonly position_ratio: number;
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
            readonly kind: "set_credits";
            /**
             * Whether the credits replace the current menu.
             */
            readonly open: boolean;
        } | {
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
                readonly kind: "set_screen_render_scale";
                /**
                 * Requested fraction of each physical rendering dimension.
                 */
                readonly scale: "100" | "75" | "50";
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
        readonly target: "analysis" | "replay";
        /**
         * Current time.
         */
        readonly value: number;
    } | {
        readonly kind: "status";
        /**
         * Typed dynamic-value provider; static values require no frame patch.
         */
        readonly source: "static" | "countdown" | "playback_time" | "playback_summary" | "analysis_cursor";
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
    readonly cursor_points: readonly GameUiChartCursorPoint[];
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
    readonly presentation: GameUiChartPresentation;
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
 * Record-interpolated position with its shared series colour.
 */
export interface GameUiChartCursorPoint {
    /**
     * Colour of the corresponding shared series.
     */
    readonly color: string;
    readonly point: GameUiChartPoint;
}
/**
 * Position in the chart axes.
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
 * Chart coordinates.
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
 * Shared chart layout and labels in logical display pixels.
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
        GameUiChartPoint2,
        GameUiChartPoint2
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
    readonly map_guides: GameUiChartMapGuides | null;
    /**
     * Marker labels in the same order as the chart markers.
     */
    readonly marker_labels: readonly GameUiChartText2[];
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
    readonly reference_labels: readonly GameUiChartText2[];
    /**
     * Keeps caption, viewport and legend inside one scrollable chart host.
     */
    readonly scroll_overflow: boolean;
    /**
     * Tick font size in logical pixels.
     */
    readonly tick_font_size: number;
    /**
     * Recorded-time labels in the same order as the time markers.
     */
    readonly time_marker_labels: readonly GameUiChartText2[];
    readonly viewport: GameUiChartExtent1;
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
    readonly x_axis: GameUiChartText3;
    /**
     * Ordered horizontal ticks.
     */
    readonly x_ticks: readonly GameUiChartTick[];
    readonly y_axis: GameUiChartText5;
    /**
     * Ordered vertical ticks.
     */
    readonly y_ticks: readonly GameUiChartTick[];
}
/**
 * One chart-space point with units defined by the axes.
 */
export interface GameUiChartPoint2 {
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
    readonly segment_gradient: readonly [
        string,
        string
    ] | null;
}
/**
 * Static map references in descriptor coordinates.
 */
export interface GameUiChartMapGuides {
    readonly north_label: GameUiChartText;
    /**
     * North arrow shaft and its two head segments.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly north_segments: readonly [
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ],
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ],
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ]
    ];
    /**
     * Represented distance in metres.
     */
    readonly scale_distance_m: number;
    readonly scale_label: GameUiChartText1;
    /**
     * Scale bar and its two end caps.
     *
     * @minItems 3
     * @maxItems 3
     */
    readonly scale_segments: readonly [
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ],
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ],
        readonly [
            GameUiChartPoint2,
            GameUiChartPoint2
        ]
    ];
}
/**
 * North label.
 */
export interface GameUiChartText {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * One chart-space point with units defined by the axes.
 */
export interface GameUiChartPoint3 {
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
 * Metre scale label.
 */
export interface GameUiChartText1 {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Static label placement independent of the rendering engine.
 */
export interface GameUiChartText2 {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Minimum usable measured dimensions in logical pixels.
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
 * Coordinate extent of this descriptor.
 */
export interface GameUiChartExtent1 {
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
 * Static label placement independent of the rendering engine.
 */
export interface GameUiChartText3 {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
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
    readonly text: GameUiChartText4;
    /**
     * Axis coordinate.
     */
    readonly value: number;
}
/**
 * Static label placement independent of the rendering engine.
 */
export interface GameUiChartText4 {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Static label placement independent of the rendering engine.
 */
export interface GameUiChartText5 {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
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
    readonly position: GameUiChartPoint3;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
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
    readonly points: readonly GameUiChartPoint2[];
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
        GameUiChartPoint2,
        GameUiChartPoint2
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
    readonly layout: "column" | "choice_grid" | "row";
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
     * Shared credits shown without changing the domain phase.
     */
    readonly credits_open: boolean;
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
