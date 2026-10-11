/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
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
    readonly chart_cursor: GameUiChartCursor | null;
    /**
     * Shared HUD projection, never a second physical state.
     */
    readonly hud: GameUiHud | null;
    /**
     * Existing range values that advance without a structural change.
     */
    readonly ranges: readonly GameUiRangeValue[];
    /**
     * Fixed record-clock and analysis-cursor readouts.
     */
    readonly readouts: readonly GameUiReadoutValue[];
    readonly selections: GameUiState;
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
     * Time-axis cursor, absent for a horizontal map.
     */
    readonly cursor_x: number | null;
    /**
     * Retained chart control identity.
     */
    readonly id: string;
    /**
     * Selected-sample wind, replacing the first map vector only.
     */
    readonly selected_wind_vector: GameUiChartVector | null;
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
        GameUiChartPoint1,
        GameUiChartPoint1
    ] | null;
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
    readonly geometry: GameUiHudGeometry;
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
 * Shared Web-baseline instrument projections, independent of engine pixels.
 */
export interface GameUiHudGeometry {
    /**
     * Visible ADI layers, absent when both attitude and flight path cues are hidden.
     */
    readonly adi: GameUiHudAdiGeometry | null;
    /**
     * AoA position between the displayed -10 and +20 degree endpoints.
     */
    readonly angle_of_attack_ratio: number | null;
    /**
     * Ordered heading tape marks and compass labels.
     */
    readonly heading_ticks: readonly GameUiHudHeadingTick[];
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
 * Value of a retained typed range control.
 */
export interface GameUiRangeValue {
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
