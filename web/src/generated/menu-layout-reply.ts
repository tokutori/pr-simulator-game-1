/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Layout result or a precise measurement failure.
 */
export type MenuLayoutReply = {
    readonly geometry: MenuDocumentGeometry;
    readonly kind: "ready";
    /**
     * Observed text for the fitted contract.
     */
    readonly measurements: readonly MenuTextMeasurement[];
    readonly plan: MenuLayoutPlan;
} | {
    readonly kind: "viewport";
    readonly projection: MenuViewportProjection;
} | {
    readonly kind: "unavailable";
    readonly reason: MenuLayoutFailure;
};
/**
 * Horizontal alignment relative to a chart label's baseline anchor.
 */
export type GameUiChartTextAnchor = "start" | "middle" | "end";
/**
 * Widget presentation data used for layout and measurement.
 */
export type MenuWidget = {
    readonly kind: "button";
    readonly presentation: GameUiButtonPresentation;
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
 * Logical role of one measured control text.
 */
export type MenuTextRole = "label" | "value" | "x-axis" | "y-axis" | "x-tick" | "y-tick" | "legend" | "annotation" | "north" | "scale";
/**
 * Font slant selected for measurement.
 */
export type MenuFontStyle = "normal" | "italic";
/**
 * Group placement strategy, selected in shared Rust.
 */
export type GameUiSectionLayout = "column" | "choice_grid" | "row";
/**
 * Stable identity of a measured text within its source panel.
 */
export type MenuTextSlot = {
    readonly kind: "fixed";
    readonly role: MenuFixedRole;
} | {
    readonly kind: "section";
    readonly role: MenuHeadingRole;
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
    readonly role: MenuTextRole;
};
/**
 * Logical role of one fixed menu text.
 */
export type MenuFixedRole = "title" | "caption" | "previous" | "next";
/**
 * Section heading marker.
 */
export type MenuHeadingRole = "heading";
/**
 * Rejection class of a measured menu document.
 */
export type MenuLayoutFailure = "stale-view" | "measurement-unavailable" | "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry" | "insufficient-view-area" | "insufficient-ink-angle" | "missing-text-metrics" | "stale-text-metrics" | "invalid-text-metrics" | "invalid-layout";
/**
 * Shared document geometry; source objects remain with the adapter.
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
    readonly sectionHeadings: readonly MenuTextPlacement[];
    readonly title: MenuTextPlacement;
}
/**
 * Placement of one measured text within its document.
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
 * Spatial chart text projection around its baseline anchor.
 */
export interface MenuChartProjection {
    readonly anchor: GameUiChartTextAnchor;
    readonly baseline: MenuPoint;
    /**
     * Rotation about the baseline in degrees.
     */
    readonly rotationDegrees: number;
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
 * Baseline and ink bounds of one already measured line.
 */
export interface MenuLineGeometry {
    readonly anchor: MenuPoint;
    readonly bounds: MenuRect;
}
/**
 * Placement of one control and its measured labels.
 */
export interface MenuControlPlacement {
    readonly bounds: MenuRect;
    /**
     * Index of the corresponding source control.
     */
    readonly controlIndex: number;
    /**
     * Chart plot rectangle ordered left, top, right and bottom.
     */
    readonly plot: MenuRect | null;
    /**
     * Ordered measurement requests or placed labels.
     */
    readonly texts: readonly MenuTextPlacement[];
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
 * Prepared layout and its complete measurement contract.
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
 * Complete source and typography observation for one layout pass.
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
    readonly widget: MenuWidget;
}
/**
 * Static chart geometry needed by layout, independent of sample arrays.
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
    readonly role: MenuTextRole;
    readonly text: GameUiChartText;
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
    readonly style: MenuFontStyle;
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
    readonly layout: GameUiSectionLayout;
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
 * Adapter measurement request derived from the shared row plan.
 */
export interface MenuTextRequest {
    /**
     * Optional shared chart text descriptor.
     */
    readonly chartText?: GameUiChartText | null;
    readonly font: MenuFont;
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
    readonly slot: MenuTextSlot;
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
 * Shared viewport placement and ordered coordinate projections.
 */
export interface MenuViewportProjection {
    readonly contentClip: MenuRect;
    /**
     * Center-origin panel coordinates in request order.
     */
    readonly documentPoints: readonly MenuPoint[];
    /**
     * Full metric scroll range.
     */
    readonly maximumOffsetMeters: number;
    readonly nextBounds: MenuRect;
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
    readonly panelPoints: readonly (MenuPoint | null)[];
    readonly previousBounds: MenuRect;
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
 * Surface-space rectangle and its intersection with the visible content.
 */
export interface MenuViewportRect {
    readonly full: MenuRect;
    /**
     * Intersection with content, or absent when entirely outside.
     */
    readonly visible: MenuRect | null;
}
