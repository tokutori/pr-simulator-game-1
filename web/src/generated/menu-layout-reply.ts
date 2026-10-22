/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Layout result or a precise measurement failure.
 */
export type MenuLayoutReply = {
    /**
     * Bounded continuation index to return with the measurements.
     */
    readonly attempt: number;
    readonly kind: "measure";
    readonly plan: MenuLayoutPlan;
} | {
    readonly kind: "plan";
    readonly plan: MenuLayoutPlan1;
} | {
    readonly geometry: MenuDocumentGeometry;
    readonly kind: "ready";
} | {
    readonly kind: "viewport";
    readonly projection: MenuViewportProjection;
} | {
    readonly kind: "unavailable";
    /**
     * Reason for rejecting the layout.
     */
    readonly reason: "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area" | "insufficient-ink-angle" | "missing-text-metrics" | "stale-text-metrics" | "invalid-text-metrics" | "invalid-layout";
};
/**
 * Complete fitted measurement contract.
 */
export interface MenuLayoutPlan {
    /**
     * Spacing between rows in metres.
     */
    readonly gapMeters: number;
    /**
     * Line spacing in metres.
     */
    readonly lineHeightMeters: number;
    /**
     * Inner padding in metres.
     */
    readonly paddingMeters: number;
    /**
     * Responsive section and control allocation.
     */
    readonly rows: readonly MenuRow[];
    readonly source: MenuLayoutSource;
    /**
     * Ordered measurement requests or placed labels.
     */
    readonly texts: readonly MenuTextRequest[];
}
/**
 * One responsive menu row.
 */
export interface MenuRow {
    /**
     * Ordered source controls or allocated row items.
     */
    readonly controls: readonly MenuRowItem[];
    /**
     * Optional heading at the start of a section.
     */
    readonly heading: MenuHeading | null;
}
/**
 * Control reference and its metric column allocation.
 */
export interface MenuRowItem {
    /**
     * Stable source index.
     */
    readonly index: number;
    /**
     * Horizontal column offset in metres.
     */
    readonly left: number;
    /**
     * Width in metres.
     */
    readonly width: number;
}
/**
 * One section heading retained in the row plan.
 */
export interface MenuHeading {
    /**
     * Published source identifier.
     */
    readonly id: string;
    /**
     * Already localized visible label.
     */
    readonly label: string;
}
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
    readonly sections: readonly GameUiSection[] | null;
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
    readonly section: string | null;
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
    readonly segment_gradient: readonly [
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
 * Adapter measurement request derived from the shared row plan.
 */
export interface MenuTextRequest {
    /**
     * Optional shared chart text descriptor.
     */
    readonly chartText?: GameUiChartText1 | null;
    readonly font: MenuFont1;
    /**
     * Nominal font size in metres.
     */
    readonly fontMeters: number;
    /**
     * Exact identity of the measurement request.
     */
    readonly identity: string;
    /**
     * Shared legend entries.
     */
    readonly legend?: GameUiChartLegend | null;
    /**
     * Explicit text measurement locale.
     */
    readonly locale: string;
    /**
     * Text's logical source location.
     */
    readonly slot: {
        readonly kind: "fixed";
        /**
         * Logical text role.
         */
        readonly role: "title" | "caption" | "previous" | "next";
    } | {
        readonly kind: "section";
        /**
         * Logical text role.
         */
        readonly role: "heading";
        /**
         * Published section identifier.
         */
        readonly sectionId: string;
    } | {
        /**
         * Published control identifier.
         */
        readonly controlId: string;
        /**
         * Stable source index.
         */
        readonly index: number;
        readonly kind: "control";
        /**
         * Logical text role.
         */
        readonly role: "label" | "value" | "x-axis" | "y-axis" | "x-tick" | "y-tick" | "legend" | "annotation" | "north" | "scale";
    };
    /**
     * Already formatted content or current toggle selection.
     */
    readonly value: string;
    /**
     * Maximum measured line width in metres.
     */
    readonly widthMeters: number;
}
/**
 * Static label placement independent of the rendering engine.
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
    readonly position: GameUiChartPoint;
    /**
     * Rotation around the baseline anchor in degrees.
     */
    readonly rotation_degrees: number;
}
/**
 * Observed font identity.
 */
export interface MenuFont1 {
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
 * Complete measurement contract.
 */
export interface MenuLayoutPlan1 {
    /**
     * Spacing between rows in metres.
     */
    readonly gapMeters: number;
    /**
     * Line spacing in metres.
     */
    readonly lineHeightMeters: number;
    /**
     * Inner padding in metres.
     */
    readonly paddingMeters: number;
    /**
     * Responsive section and control allocation.
     */
    readonly rows: readonly MenuRow[];
    readonly source: MenuLayoutSource;
    /**
     * Ordered measurement requests or placed labels.
     */
    readonly texts: readonly MenuTextRequest[];
}
/**
 * Resolved document geometry.
 */
export interface MenuDocumentGeometry {
    readonly caption: MenuTextPlacement;
    /**
     * Full scrollable content height.
     */
    readonly contentHeightMeters: number;
    /**
     * Ordered source controls or allocated row items.
     */
    readonly controls: readonly MenuControlPlacement[];
    /**
     * Measured page-control height.
     */
    readonly footerHeightMeters: number;
    /**
     * Measurement index for the next-page label.
     */
    readonly nextIndex: number;
    /**
     * Measurement index for the previous-page label.
     */
    readonly previousIndex: number;
    /**
     * Placed section headings.
     */
    readonly sectionHeadings: readonly MenuTextPlacement1[];
    readonly title: MenuTextPlacement2;
}
/**
 * Panel description.
 */
export interface MenuTextPlacement {
    readonly bounds: MenuRect;
    /**
     * Optional chart baseline projection.
     */
    readonly chartProjection?: MenuChartProjection | null;
    /**
     * Projected line anchors and ink bounds.
     */
    readonly lineGeometry: readonly MenuLineGeometry[];
    /**
     * Index of the corresponding measurement request.
     */
    readonly textIndex: number;
}
/**
 * Metric document bounds.
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
 * Spatial chart text projection around its baseline anchor.
 */
export interface MenuChartProjection {
    /**
     * Horizontal text anchor.
     */
    readonly anchor: "start" | "middle" | "end";
    readonly baseline: MenuPoint;
    /**
     * Rotation about the baseline in degrees.
     */
    readonly rotationDegrees: number;
}
/**
 * Metric alphabetic baseline anchor.
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
 * Baseline and ink bounds of one already measured line.
 */
export interface MenuLineGeometry {
    readonly anchor: MenuPoint1;
    readonly bounds: MenuRect1;
}
/**
 * Horizontal text anchor.
 */
export interface MenuPoint1 {
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
 * Metric document bounds.
 */
export interface MenuRect1 {
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
 * Placement of one control and its measured labels.
 */
export interface MenuControlPlacement {
    readonly bounds: MenuRect2;
    /**
     * Index of the corresponding source control.
     */
    readonly controlIndex: number;
    /**
     * Chart plot rectangle ordered left, top, right and bottom.
     */
    readonly plot: MenuRect3 | null;
    /**
     * Ordered measurement requests or placed labels.
     */
    readonly texts: readonly MenuTextPlacement1[];
}
/**
 * Metric document bounds.
 */
export interface MenuRect2 {
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
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect3 {
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
 * Placement of one measured text within its document.
 */
export interface MenuTextPlacement1 {
    readonly bounds: MenuRect;
    /**
     * Optional chart baseline projection.
     */
    readonly chartProjection?: MenuChartProjection | null;
    /**
     * Projected line anchors and ink bounds.
     */
    readonly lineGeometry: readonly MenuLineGeometry[];
    /**
     * Index of the corresponding measurement request.
     */
    readonly textIndex: number;
}
/**
 * Placement of one measured text within its document.
 */
export interface MenuTextPlacement2 {
    readonly bounds: MenuRect;
    /**
     * Optional chart baseline projection.
     */
    readonly chartProjection?: MenuChartProjection | null;
    /**
     * Projected line anchors and ink bounds.
     */
    readonly lineGeometry: readonly MenuLineGeometry[];
    /**
     * Index of the corresponding measurement request.
     */
    readonly textIndex: number;
}
/**
 * Scroll geometry, clipped rectangles and transformed points.
 */
export interface MenuViewportProjection {
    readonly contentClip: MenuRect4;
    /**
     * Center-origin panel coordinates in request order.
     */
    readonly documentPoints: readonly MenuPoint2[];
    /**
     * Full metric scroll range.
     */
    readonly maximumOffsetMeters: number;
    readonly nextBounds: MenuRect5;
    /**
     * Current metric scroll offset.
     */
    readonly offsetMeters: number;
    /**
     * Normalized increment for one visible page.
     */
    readonly pageProgress: number;
    /**
     * Document coordinates, absent for panel points outside the content clip.
     */
    readonly panelPoints: readonly (MenuPoint2 | null)[];
    readonly previousBounds: MenuRect6;
    /**
     * Current normalized scroll position.
     */
    readonly progress: number;
    /**
     * Translated and clipped rectangles in request order.
     */
    readonly rectangles: readonly MenuViewportRect[];
}
/**
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect4 {
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
export interface MenuPoint2 {
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
export interface MenuRect5 {
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
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect6 {
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
 * Surface-space rectangle and its intersection with the visible content.
 */
export interface MenuViewportRect {
    readonly full: MenuRect7;
    /**
     * Intersection with content, or absent when entirely outside.
     */
    readonly visible: MenuRect3 | null;
}
/**
 * Metric rectangle in a spatial menu document.
 */
export interface MenuRect7 {
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
