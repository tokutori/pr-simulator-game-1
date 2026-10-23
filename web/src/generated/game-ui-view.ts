/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Exclusive current overlay names retained by both renderer adapters.
 */
export type GameUiOverlay = "Credits" | "Pause" | "PauseSettings" | "PauseHelp";
export type GameUiAction = import("./game-ui-action.js").GameUiAction;
/**
 * Widget implementations must exhaustively handle each display primitive.
 */
export type GameUiHudNodeContent = {
    readonly kind: "text";
    readonly tone: GameUiHudTone;
    /**
     * Fully formatted text; adapters never parse its wording.
     */
    readonly value: string;
} | {
    readonly geometry: GameUiHudAdiGeometry;
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
 * Shared text priority; mandatory resource failures always use Warning.
 */
export type GameUiHudTone = "normal" | "muted" | "warning";
/**
 * Visible ADI region with an attitude layer or an independent flight path cue.
 */
export type GameUiHudAdiGeometry = {
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
    readonly action: GameUiAction;
    readonly kind: "button";
    readonly presentation: GameUiButtonPresentation;
} | {
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
    readonly target: GameUiRangeTarget;
    /**
     * Current time.
     */
    readonly value: number;
} | {
    readonly kind: "status";
    readonly source: GameUiReadoutSource;
    /**
     * Shared formatted content.
     */
    readonly value: string;
} | {
    readonly chart: GameUiAnalysisChart;
    readonly kind: "chart";
    readonly plot: GameUiChart;
};
/**
 * Shared button role and selection state.
 */
export type GameUiButtonPresentation = {
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
 * Stable identity for one independently configurable HUD cue.
 */
export type HudCue = "telemetry" | "attitude" | "wind" | "flight_path" | "angle_of_attack" | "warnings";
/**
 * Range action target.
 */
export type GameUiRangeTarget = "analysis" | "replay";
/**
 * Shared source for frame-varying readouts, independent of element identity.
 */
export type GameUiReadoutSource = "static" | "countdown" | "playback_time" | "playback_summary" | "analysis_cursor";
/**
 * Plot generated from the retained record.
 */
export type GameUiAnalysisChart = "map" | "altitude" | "speed";
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
 * Exclusive pause overlay.
 */
export type GameUiPauseOverlay = "menu" | "settings" | "help";
/**
 * Current Web replay camera candidates in their shared selection order.
 */
export type GameUiReplayCamera = "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
/**
 * Replay presentation selection.
 */
export type GameUiReplayView = "cinematic" | "telemetry" | "analysis";
/**
 * Result presentation mode.
 */
export type GameUiResultTab = "summary" | "analysis";
/**
 * Pure shared scene and menu projection.
 */
export interface GameUiView {
    /**
     * Shared exclusive overlay name.
     */
    readonly active_overlay: GameUiOverlay | null;
    /**
     * Contextual Back operation selected from enabled shared controls.
     */
    readonly back_action: GameUiAction | null;
    /**
     * Shared explanatory text.
     */
    readonly description: string;
    /**
     * Shared telemetry and instrument projection, absent outside a flight or record.
     */
    readonly hud: GameUiHud | null;
    /**
     * Opaque semantic view identity for focus and scroll ownership.
     */
    readonly interaction_scope: string;
    readonly layout: GameUiLayout;
    /**
     * Ordered semantic panels.
     */
    readonly panels: readonly GameUiPanel[];
    readonly scene: GameUiScene;
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
    readonly placement: GameUiHudPlacement;
}
/**
 * One existing widget instance; adding an instance requires no adapter-specific field.
 */
export interface GameUiHudNode {
    readonly bounds: GameUiRect;
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
 * Web-baseline horizon rotation and translation.
 */
export interface GameUiHudAttitudeGeometry {
    /**
     * Pre-rotation pitch translation relative to the ADI height.
     */
    readonly pitch_offset_ratio: number;
    /**
     * Shared pitch ladder in the translated and rotated horizon coordinate system.
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
    /**
     * Clockwise horizon rotation in display coordinates.
     */
    readonly rotation_degrees: number;
}
/**
 * One pitch ladder mark relative to the complete ADI width and height.
 */
export interface GameUiHudPitchTick {
    /**
     * Half of the horizontal mark width.
     */
    readonly half_width_ratio: number;
    /**
     * Absolute pitch in degrees; the horizon has no numeric label.
     */
    readonly label_degrees: number | null;
    /**
     * Vertical offset from the zero-pitch horizon.
     */
    readonly offset_ratio: number;
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
    readonly rect: GameUiRect;
    /**
     * Section identifier.
     */
    readonly section: string;
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
    readonly point: GameUiChartPoint;
}
/**
 * Two-dimensional chart point; exported geometry uses logical display pixels.
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
    readonly map_guides: GameUiChartMapGuides | null;
    /**
     * Marker labels in the same order as the chart markers.
     */
    readonly marker_labels: readonly GameUiChartText[];
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
     * Tick font size in logical pixels.
     */
    readonly tick_font_size: number;
    /**
     * Recorded-time labels in the same order as the time markers.
     */
    readonly time_marker_labels: readonly GameUiChartText[];
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
    readonly x_axis: GameUiChartText;
    /**
     * Ordered horizontal ticks.
     */
    readonly x_ticks: readonly GameUiChartTick[];
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
    readonly position: GameUiChartPoint;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
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
 * Tick value, placement and already formatted label.
 */
export interface GameUiChartTick {
    /**
     * Increasing-axis fraction between zero and one.
     */
    readonly ratio: number;
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
    readonly layout: GameUiSectionLayout;
}
/**
 * UI-only selections; physical state and domain phase remain in GameSession.
 */
export interface GameUiState {
    readonly analysis_chart: GameUiAnalysisChart;
    /**
     * Read-only analysis query position in seconds.
     */
    readonly analysis_time_seconds: number;
    /**
     * Shared credits shown without changing the domain phase.
     */
    readonly credits_open: boolean;
    readonly pause_overlay: GameUiPauseOverlay;
    readonly replay_camera: GameUiReplayCamera;
    readonly replay_view: GameUiReplayView;
    readonly result_tab: GameUiResultTab;
    /**
     * Expanded preparation diagnostics.
     */
    readonly technical_open: boolean;
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
