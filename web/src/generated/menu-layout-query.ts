/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Aggregate layout protocol spanning preparation and measured arrangement.
 */
export type MenuLayoutQuery = {
    readonly kind: "plan";
    readonly source: MenuLayoutSource;
} | {
    readonly kind: "arrange";
    /**
     * Actual adapter text measurements.
     */
    readonly measurements: readonly MenuTextMeasurement[];
    readonly source: MenuLayoutSource1;
} | {
    /**
     * Document points to map into center-origin panel coordinates.
     */
    readonly document_points: readonly MenuPoint[];
    readonly kind: "viewport";
    /**
     * Center-origin panel points to map into visible document content.
     */
    readonly panel_points: readonly MenuPoint[];
    /**
     * Document rectangles to translate and clip together.
     */
    readonly rectangles: readonly MenuRect[];
    readonly source: MenuViewportSource;
};
/**
 * Compact source and typography observation.
 */
export interface MenuLayoutSource {
    /**
     * Panel description.
     */
    readonly caption: string;
    /**
     * Ordered source controls or allocated row items.
     */
    readonly controls: readonly MenuLayoutControl[];
    readonly font: MenuFont;
    /**
     * Nominal font size in metres.
     */
    readonly fontMeters: number;
    /**
     * Explicit text measurement locale.
     */
    readonly locale: string;
    /**
     * Localized next-page label.
     */
    readonly nextLabel: string;
    /**
     * Published panel identifier.
     */
    readonly panelId: string;
    /**
     * Localized previous-page label.
     */
    readonly previousLabel: string;
    /**
     * Declared sections or implicit vertical flow.
     */
    readonly sections?: readonly GameUiSection[] | null;
    readonly surfaceSize: MenuSize;
    /**
     * Panel heading.
     */
    readonly title: string;
}
/**
 * Compact source control, excluding record samples and semantic actions.
 */
export interface MenuLayoutControl {
    /**
     * Published source identifier.
     */
    readonly id: string;
    /**
     * Already localized visible label.
     */
    readonly label: string;
    /**
     * Published section membership.
     */
    readonly section?: string | null;
    /**
     * Widget presentation needed by measurement.
     */
    readonly widget: {
        readonly kind: "button";
        /**
         * Shared button presentation.
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
        readonly kind: "toggle";
        /**
         * Already formatted content or current toggle selection.
         */
        readonly value: boolean;
    } | {
        /**
         * Formatted range selection.
         */
        readonly displayValue: string;
        readonly kind: "range";
    } | {
        readonly kind: "status";
        /**
         * Already formatted content or current toggle selection.
         */
        readonly value: string;
    } | {
        readonly kind: "chart";
        readonly layout: MenuChartLayout;
    };
}
/**
 * Static chart layout.
 */
export interface MenuChartLayout {
    /**
     * Ordered chart text descriptors.
     */
    readonly labels: readonly MenuChartLabel[];
    /**
     * Shared legend entries.
     */
    readonly legend: readonly GameUiChartLegend[];
    /**
     * Chart plot rectangle ordered left, top, right and bottom.
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
     * Chart viewport width and height.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly size: readonly [
        number,
        number
    ];
    /**
     * Reference font size for chart label scaling.
     */
    readonly tickFontSize: number;
}
/**
 * Shared chart label projected into the menu's text measurement pass.
 */
export interface MenuChartLabel {
    /**
     * Stable source index.
     */
    readonly index: number;
    /**
     * Logical text role.
     */
    readonly role: "label" | "value" | "x-axis" | "y-axis" | "x-tick" | "y-tick" | "legend" | "annotation" | "north" | "scale";
    readonly text: GameUiChartText;
}
/**
 * Shared chart label descriptor.
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
    readonly position: GameUiChartPoint;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Alphabetic-baseline position in logical pixels; adapters apply actual font metrics.
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
}
/**
 * Observed font identity.
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
    readonly style: "normal" | "italic";
    /**
     * Font weight in the range 1 through 1000.
     */
    readonly weight: number;
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
 * Metric width and height of the menu surface.
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
 * Measured lines associated with one exact request identity.
 */
export interface MenuTextMeasurement {
    /**
     * Exact identity of the measurement request.
     */
    readonly identity: string;
    /**
     * Measured lines preserving explicit paragraphs.
     */
    readonly lines: readonly MenuMeasuredLine[];
}
/**
 * One measured line with signed ink bearings in metres.
 */
export interface MenuMeasuredLine {
    /**
     * Measured horizontal text advance.
     */
    readonly advanceMeters: number;
    /**
     * Measured ink ascent.
     */
    readonly ascentMeters: number;
    /**
     * Measured ink descent.
     */
    readonly descentMeters: number;
    /**
     * Signed left ink bearing.
     */
    readonly leftMeters: number;
    /**
     * Zero-based index of the original explicit paragraph.
     */
    readonly paragraphIndex: number;
    /**
     * Signed right ink bearing.
     */
    readonly rightMeters: number;
    /**
     * Already formatted content or current toggle selection.
     */
    readonly value: string;
}
/**
 * Compact source and typography observation.
 */
export interface MenuLayoutSource1 {
    /**
     * Panel description.
     */
    readonly caption: string;
    /**
     * Ordered source controls or allocated row items.
     */
    readonly controls: readonly MenuLayoutControl[];
    readonly font: MenuFont;
    /**
     * Nominal font size in metres.
     */
    readonly fontMeters: number;
    /**
     * Explicit text measurement locale.
     */
    readonly locale: string;
    /**
     * Localized next-page label.
     */
    readonly nextLabel: string;
    /**
     * Published panel identifier.
     */
    readonly panelId: string;
    /**
     * Localized previous-page label.
     */
    readonly previousLabel: string;
    /**
     * Declared sections or implicit vertical flow.
     */
    readonly sections?: readonly GameUiSection[] | null;
    readonly surfaceSize: MenuSize;
    /**
     * Panel heading.
     */
    readonly title: string;
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
 * Measured document and current scroll observation.
 */
export interface MenuViewportSource {
    /**
     * Measured scrollable document height.
     */
    readonly contentHeightMeters: number;
    /**
     * Measured footer control height.
     */
    readonly footerHeightMeters: number;
    /**
     * Separation between content and page controls.
     */
    readonly gapMeters: number;
    /**
     * Inset around document content and page controls.
     */
    readonly paddingMeters: number;
    /**
     * Normalized document scroll position.
     */
    readonly progress: number;
    readonly surfaceSize: MenuSize1;
}
/**
 * Physical panel dimensions.
 */
export interface MenuSize1 {
    /**
     * Height in metres.
     */
    readonly height: number;
    /**
     * Width in metres.
     */
    readonly width: number;
}
