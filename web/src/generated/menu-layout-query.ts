/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Aggregate layout protocol spanning preparation and measured arrangement.
 */
export type MenuLayoutQuery = {
    readonly kind: "prepare";
    readonly menu_from_panel: Pose;
    /**
     * Reply to the current requested candidate, or absent at opening.
     */
    readonly observation?: MenuPreparationObservation | null;
    readonly source: MenuLayoutSource;
    readonly viewer: XrViewerFrame;
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
 * Horizontal alignment relative to a chart label's baseline anchor.
 */
export type GameUiChartTextAnchor = "start" | "middle" | "end";
/**
 * Font slant selected for measurement.
 */
export type MenuFontStyle = "normal" | "italic";
/**
 * Group placement strategy, selected in shared Rust.
 */
export type GameUiSectionLayout = "column" | "choice_grid" | "row";
/**
 * Same-frame stereo geometry, separate from tracked world-camera placement.
 */
export type XrViewerFrame = {
    readonly reason: XrViewerUnavailableReason;
    readonly source: "unavailable";
    /**
     * Valid centre tracking, if separately available.
     */
    readonly trackingFromHead?: Pose | null;
} | {
    /**
     * Left and right eye geometry, in that order.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly eyes: readonly [
        XrEyeGeometry,
        XrEyeGeometry
    ];
    readonly source: "configured";
    /**
     * Explicit null at the existing wire boundary.
     */
    readonly trackingFromHead: null;
} | {
    /**
     * Left and right eye geometry, in that order.
     *
     * @minItems 2
     * @maxItems 2
     */
    readonly eyes: readonly [
        XrEyeGeometry,
        XrEyeGeometry
    ];
    readonly source: "runtime-derived";
    readonly trackingFromHead: Pose;
};
/**
 * Classified absence of usable stereo observations.
 */
export type XrViewerUnavailableReason = "not-stereo" | "viewer-unavailable" | "unsupported-view-configuration" | "invalid-view-geometry";
/**
 * Homogeneous depth-clip convention of the rendering API's projection matrix.
 */
export type XrDepthRange = "negative-one-to-one" | "zero-to-one";
/**
 * Eye identity within the supported primary stereo view configuration.
 */
export type XrEye = "left" | "right";
/**
 * Rigid child-to-parent transform; the caller names both reference spaces.
 */
export interface Pose {
    readonly orientation: Quaternion;
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
 * One adapter observation of a requested preparation candidate.
 */
export interface MenuPreparationObservation {
    /**
     * Candidate index supplied by the shared preparation result.
     */
    readonly attempt: number;
    /**
     * Actual text metrics for that candidate's exact measurement identities.
     */
    readonly measurements: readonly MenuTextMeasurement[];
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
    readonly segment_gradient?: readonly [
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
 * One same-frame eye pose and the runtime's column-major projection matrix.
 */
export interface XrEyeGeometry {
    readonly depthRange: XrDepthRange;
    readonly eye: XrEye;
    readonly headFromEye: Pose;
    /**
     * Unmodified column-major 4-by-4 projection matrix.
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
 * Metric observations required to place a scrollable document on its surface.
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
    readonly surfaceSize: MenuSize;
}
