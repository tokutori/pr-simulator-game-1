/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
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
