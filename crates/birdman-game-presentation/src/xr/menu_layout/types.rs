use crate::game_ui::{GameUiButtonPresentation, GameUiChartLegend, GameUiChartText, GameUiSection};
use serde::{Deserialize, Serialize};

/// Metric point in a spatial menu document.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuPoint {
    /// Horizontal coordinate in metres.
    pub x: f64,
    /// Vertical coordinate in metres.
    pub y: f64,
}

/// Metric rectangle in a spatial menu document.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuRect {
    /// Horizontal coordinate in metres.
    pub x: f64,
    /// Vertical coordinate in metres.
    pub y: f64,
    /// Width in metres.
    pub width: f64,
    /// Height in metres.
    pub height: f64,
}

/// Font resource observation supplied by the drawing adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuFont {
    /// Adapter font family identifier.
    pub family: String,
    /// Font weight in the range 1 through 1000.
    pub weight: u16,
    /// Observed font slant.
    pub style: MenuFontStyle,
    /// Adapter font resource generation.
    pub generation: u64,
}

/// Font slant selected for measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum MenuFontStyle {
    /// Upright font.
    Normal,
    /// Italic font.
    Italic,
}

/// Logical role of one measured control text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum MenuTextRole {
    /// Control label.
    Label,
    /// Current control value.
    Value,
    /// Horizontal axis title.
    XAxis,
    /// Vertical axis title.
    YAxis,
    /// Horizontal axis tick.
    XTick,
    /// Vertical axis tick.
    YTick,
    /// Legend entry.
    Legend,
    /// Chart annotation.
    Annotation,
    /// North reference.
    North,
    /// Map scale reference.
    Scale,
}

/// Logical role of one fixed menu text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum MenuFixedRole {
    /// Panel title.
    Title,
    /// Panel description.
    Caption,
    /// Previous-page label.
    Previous,
    /// Next-page label.
    Next,
}

/// Stable identity of a measured text within its source panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MenuTextSlot {
    /// Fixed document text.
    Fixed {
        /// Logical text role.
        role: MenuFixedRole,
    },
    /// Section heading.
    Section {
        /// Logical text role.
        role: MenuHeadingRole,
        /// Published section identifier.
        section_id: String,
    },
    /// Control-owned text.
    Control {
        /// Published control identifier.
        control_id: String,
        /// Logical text role.
        role: MenuTextRole,
        /// Stable source index.
        index: usize,
    },
}

/// Section heading marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum MenuHeadingRole {
    /// Section heading marker.
    Heading,
}

/// Shared chart label projected into the menu's text measurement pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuChartLabel {
    /// Logical text role.
    pub role: MenuTextRole,
    /// Stable source index.
    pub index: usize,
    /// Shared chart label descriptor.
    pub text: GameUiChartText,
}

/// Static chart geometry needed by layout, independent of sample arrays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuChartLayout {
    /// Chart viewport width and height.
    pub size: [f64; 2],
    /// Chart plot rectangle ordered left, top, right and bottom.
    pub plot: [f64; 4],
    /// Reference font size for chart label scaling.
    pub tick_font_size: f64,
    /// Ordered chart text descriptors.
    pub labels: Vec<MenuChartLabel>,
    /// Shared legend entries.
    pub legend: Vec<GameUiChartLegend>,
}

/// Widget presentation data used for layout and measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MenuWidget {
    /// Action or choice button.
    Button {
        /// Shared button presentation.
        presentation: GameUiButtonPresentation,
    },
    /// Boolean selection.
    Toggle {
        /// Already formatted content or current toggle selection.
        value: bool,
    },
    /// Numeric range selection.
    Range {
        /// Formatted range selection.
        display_value: String,
    },
    /// Read-only status.
    Status {
        /// Already formatted content or current toggle selection.
        value: String,
    },
    /// Static chart layout.
    Chart {
        /// Static chart layout.
        layout: MenuChartLayout,
    },
}

/// Compact source control, excluding record samples and semantic actions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuLayoutControl {
    /// Published source identifier.
    pub id: String,
    /// Published section membership.
    pub section: Option<String>,
    /// Already localized visible label.
    pub label: String,
    /// Widget presentation needed by measurement.
    pub widget: MenuWidget,
}

/// Complete source and typography observation for one layout pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuLayoutSource {
    /// Published panel identifier.
    pub panel_id: String,
    /// Panel heading.
    pub title: String,
    /// Declared sections or implicit vertical flow.
    pub sections: Option<Vec<GameUiSection>>,
    /// Ordered source controls or allocated row items.
    pub controls: Vec<MenuLayoutControl>,
    /// Metric width and height of the menu surface.
    pub surface_size: MenuSize,
    /// Nominal font size in metres.
    pub font_meters: f64,
    /// Observed font identity.
    pub font: MenuFont,
    /// Explicit text measurement locale.
    pub locale: String,
    /// Panel description.
    pub caption: String,
    /// Localized previous-page label.
    pub previous_label: String,
    /// Localized next-page label.
    pub next_label: String,
}

/// Measured surface dimensions in metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuSize {
    /// Width in metres.
    pub width: f64,
    /// Height in metres.
    pub height: f64,
}

/// One section heading retained in the row plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuHeading {
    /// Published source identifier.
    pub id: String,
    /// Already localized visible label.
    pub label: String,
}

/// Control reference and its metric column allocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuRowItem {
    /// Stable source index.
    pub index: usize,
    /// Horizontal column offset in metres.
    pub left: f64,
    /// Width in metres.
    pub width: f64,
}

/// One responsive menu row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuRow {
    /// Optional heading at the start of a section.
    pub heading: Option<MenuHeading>,
    /// Ordered source controls or allocated row items.
    pub controls: Vec<MenuRowItem>,
}

/// Adapter measurement request derived from the shared row plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuTextRequest {
    /// Exact identity of the measurement request.
    pub identity: String,
    /// Text's logical source location.
    pub slot: MenuTextSlot,
    /// Already formatted content or current toggle selection.
    pub value: String,
    /// Explicit text measurement locale.
    pub locale: String,
    /// Nominal font size in metres.
    pub font_meters: f64,
    /// Observed font identity.
    pub font: MenuFont,
    /// Maximum measured line width in metres.
    pub width_meters: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional shared chart text descriptor.
    pub chart_text: Option<GameUiChartText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Shared legend entries.
    pub legend: Option<GameUiChartLegend>,
}

/// Prepared layout and its complete measurement contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuLayoutPlan {
    /// Compact source and typography observation.
    pub source: MenuLayoutSource,
    /// Line spacing in metres.
    pub line_height_meters: f64,
    /// Inner padding in metres.
    pub padding_meters: f64,
    /// Spacing between rows in metres.
    pub gap_meters: f64,
    /// Responsive section and control allocation.
    pub rows: Vec<MenuRow>,
    /// Ordered measurement requests or placed labels.
    pub texts: Vec<MenuTextRequest>,
}

/// One measured line with signed ink bearings in metres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuMeasuredLine {
    /// Zero-based index of the original explicit paragraph.
    pub paragraph_index: usize,
    /// Already formatted content or current toggle selection.
    pub value: String,
    /// Measured horizontal text advance.
    pub advance_meters: f64,
    /// Signed left ink bearing.
    pub left_meters: f64,
    /// Signed right ink bearing.
    pub right_meters: f64,
    /// Measured ink ascent.
    pub ascent_meters: f64,
    /// Measured ink descent.
    pub descent_meters: f64,
}

/// Measured lines associated with one exact request identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuTextMeasurement {
    /// Exact identity of the measurement request.
    pub identity: String,
    /// Measured lines preserving explicit paragraphs.
    pub lines: Vec<MenuMeasuredLine>,
}

/// Spatial chart text projection around its baseline anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuChartProjection {
    /// Metric alphabetic baseline anchor.
    pub baseline: MenuPoint,
    /// Rotation about the baseline in degrees.
    pub rotation_degrees: f64,
    /// Horizontal text anchor.
    pub anchor: crate::game_ui::GameUiChartTextAnchor,
}

/// Baseline and ink bounds of one already measured line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuLineGeometry {
    /// Horizontal text anchor.
    pub anchor: MenuPoint,
    /// Metric document bounds.
    pub bounds: MenuRect,
}

/// Placement of one measured text within its document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuTextPlacement {
    /// Index of the corresponding measurement request.
    pub text_index: usize,
    /// Metric document bounds.
    pub bounds: MenuRect,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional chart baseline projection.
    pub chart_projection: Option<MenuChartProjection>,
    /// Projected line anchors and ink bounds.
    pub line_geometry: Vec<MenuLineGeometry>,
}

/// Placement of one control and its measured labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuControlPlacement {
    /// Index of the corresponding source control.
    pub control_index: usize,
    /// Metric document bounds.
    pub bounds: MenuRect,
    /// Ordered measurement requests or placed labels.
    pub texts: Vec<MenuTextPlacement>,
    /// Chart plot rectangle ordered left, top, right and bottom.
    pub plot: Option<MenuRect>,
}

/// Shared document geometry; source objects remain with the adapter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuDocumentGeometry {
    /// Panel heading.
    pub title: MenuTextPlacement,
    /// Panel description.
    pub caption: MenuTextPlacement,
    /// Placed section headings.
    pub section_headings: Vec<MenuTextPlacement>,
    /// Ordered source controls or allocated row items.
    pub controls: Vec<MenuControlPlacement>,
    /// Full scrollable content height.
    pub content_height_meters: f64,
    /// Measured page-control height.
    pub footer_height_meters: f64,
    /// Measurement index for the previous-page label.
    pub previous_index: usize,
    /// Measurement index for the next-page label.
    pub next_index: usize,
}

/// Aggregate layout protocol spanning preparation and measured arrangement.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum MenuLayoutQuery {
    /// Fits a surface and requests real text observations until readable.
    Prepare {
        /// Requested content and initial typography.
        source: MenuLayoutSource,
        /// Same-frame stereo optics.
        viewer: Box<crate::xr::XrViewerFrame>,
        /// Physical panel pose relative to the menu anchor.
        menu_from_panel: crate::camera::Pose,
        /// Reply to the current requested candidate, or absent at opening.
        observation: Option<MenuPreparationObservation>,
    },
    /// Menu measurement planning phase.
    Plan {
        /// Compact source and typography observation.
        source: MenuLayoutSource,
    },
    /// Measured document arrangement phase.
    Arrange {
        /// Compact source and typography observation.
        source: MenuLayoutSource,
        /// Actual adapter text measurements.
        measurements: Vec<MenuTextMeasurement>,
    },
    /// Batch scroll clipping and panel/document coordinate projection.
    Viewport {
        /// Measured document and current scroll observation.
        source: MenuViewportSource,
        /// Document rectangles to translate and clip together.
        rectangles: Vec<MenuRect>,
        /// Center-origin panel points to map into visible document content.
        panel_points: Vec<MenuPoint>,
        /// Document points to map into center-origin panel coordinates.
        document_points: Vec<MenuPoint>,
    },
}

/// Layout result or a precise measurement failure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum MenuLayoutReply {
    /// Requested real text measurement for the next preparation candidate.
    Measure {
        /// Bounded continuation index to return with the measurements.
        attempt: usize,
        /// Complete fitted measurement contract.
        plan: MenuLayoutPlan,
    },
    /// Menu measurement planning phase.
    Plan {
        /// Complete measurement contract.
        plan: MenuLayoutPlan,
    },
    /// Complete resolved document geometry.
    Ready {
        /// Resolved document geometry.
        geometry: MenuDocumentGeometry,
    },
    /// Resolved viewport and corresponding ordered batch projections.
    Viewport {
        /// Scroll geometry, clipped rectangles and transformed points.
        projection: MenuViewportProjection,
    },
    /// Rejected layout or measurement.
    Unavailable {
        /// Reason for rejecting the layout.
        reason: MenuLayoutFailure,
    },
}

/// One adapter observation of a requested preparation candidate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuPreparationObservation {
    /// Candidate index supplied by the shared preparation result.
    pub attempt: usize,
    /// Actual text metrics for that candidate's exact measurement identities.
    pub measurements: Vec<MenuTextMeasurement>,
}

/// Metric observations required to place a scrollable document on its surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuViewportSource {
    /// Physical panel dimensions.
    pub surface_size: MenuSize,
    /// Inset around document content and page controls.
    pub padding_meters: f64,
    /// Separation between content and page controls.
    pub gap_meters: f64,
    /// Measured scrollable document height.
    pub content_height_meters: f64,
    /// Measured footer control height.
    pub footer_height_meters: f64,
    /// Normalized document scroll position.
    pub progress: f64,
}

/// Surface-space rectangle and its intersection with the visible content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuViewportRect {
    /// Translated rectangle before clipping, also used by range input tracks.
    pub full: MenuRect,
    /// Intersection with content, or absent when entirely outside.
    pub visible: Option<MenuRect>,
}

/// Shared viewport placement and ordered coordinate projections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuViewportProjection {
    /// Surface-space visible content rectangle.
    pub content_clip: MenuRect,
    /// Previous-page surface rectangle.
    pub previous_bounds: MenuRect,
    /// Next-page surface rectangle.
    pub next_bounds: MenuRect,
    /// Current normalized scroll position.
    pub progress: f64,
    /// Current metric scroll offset.
    pub offset_meters: f64,
    /// Full metric scroll range.
    pub maximum_offset_meters: f64,
    /// Normalized increment for one visible page.
    pub page_progress: f64,
    /// Translated and clipped rectangles in request order.
    pub rectangles: Vec<MenuViewportRect>,
    /// Document coordinates, absent for panel points outside the content clip.
    pub panel_points: Vec<Option<MenuPoint>>,
    /// Center-origin panel coordinates in request order.
    pub document_points: Vec<MenuPoint>,
}

/// Rejection class of a measured menu document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum MenuLayoutFailure {
    /// No stereo observation is available.
    NotStereo,
    /// Current tracking has no viewer pose.
    ViewerUnavailable,
    /// The runtime view arrangement is unsupported.
    UnsupportedViewConfiguration,
    /// Observed pose or projection is invalid.
    InvalidViewGeometry,
    /// The complete surface cannot fit into both views.
    InsufficientViewArea,
    /// Measured glyphs remain below the required angular size.
    InsufficientInkAngle,
    /// A requested measurement is absent.
    MissingTextMetrics,
    /// A measurement identity is duplicated or belongs to another plan.
    StaleTextMetrics,
    /// Measured text or its metric bounds violate the request.
    InvalidTextMetrics,
    /// Source geometry or typography cannot form a valid layout.
    InvalidLayout,
}
