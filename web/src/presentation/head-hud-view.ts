import { IDENTITY_POSE, pose, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { HeadHudElement, HeadHudRect, VisibleHeadHud } from "../render/contracts/head-hud.js";
import { headPlaneFitsViews } from "../render/contracts/viewer-frame.js";
import type { ViewerFrame, ViewerGeometryUnavailableReason } from "../render/contracts/viewer-frame.js";
import type { FlightHudModel } from "./flight-hud-model.js";
import { FLIGHT_MENU_GEOMETRY } from "../render/contracts/ui.js";
import { formatFrameRate } from "./frame-rate.js";

export interface HeadHudLayoutProfile {
  readonly distanceMeters: number;
  readonly maximumWidthMeters: number;
  readonly maximumHeightMeters: number;
  readonly clearHorizontalHalfAngleDegrees: number;
  readonly clearVerticalHalfAngleDegrees: number;
  readonly textHeightDegrees: number;
  readonly clipMargin: number;
  readonly foregroundAlpha: number;
}

export type HeadHudUnavailableReason = ViewerGeometryUnavailableReason | "insufficient-view-area" | "text-overflow";

export type HeadHudView =
  | { readonly kind: "absent" }
  | { readonly kind: "unavailable"; readonly reason: HeadHudUnavailableReason }
  | { readonly kind: "visible"; readonly layer: VisibleHeadHud; readonly textHeightMeters: number };

export const NO_HEAD_HUD_VIEW = Object.freeze({ kind: "absent" } as const);
export const HEAD_HUD_LINE_HEIGHT = 1.5;
export const HEAD_HUD_CARD_PADDING = 0.5;
export const HEAD_HUD_CARD_GAP = 0.6;

export const DEFAULT_HEAD_HUD_PROFILE: HeadHudLayoutProfile = Object.freeze({
  distanceMeters: 2.4,
  maximumWidthMeters: 2.16,
  maximumHeightMeters: 2.4,
  clearHorizontalHalfAngleDegrees: 15,
  clearVerticalHalfAngleDegrees: 10,
  textHeightDegrees: 0.6,
  clipMargin: 0.035,
  foregroundAlpha: 1
});

export function createHeadHudView(model: FlightHudModel | null, viewer: ViewerFrame, locale: string, profile: HeadHudLayoutProfile = DEFAULT_HEAD_HUD_PROFILE,
  framesPerSecond?: number | null, sharedTheme: FlightHudModel["theme"] | null = model?.theme ?? null): HeadHudView {
  if (locale.trim() === "") throw new RangeError("Head HUD display locale must be explicit");
  validateProfile(profile);
  const instrumentGroups = model?.groups.filter((group) => group.placement !== "readouts") ?? [];
  const instruments = instrumentGroups.flatMap((group) => group.nodes);
  const frameRateLabel = framesPerSecond === undefined ? null : formatFrameRate(framesPerSecond);
  const textElements = model?.groups.filter((group) => group.placement === "readouts")
    .flatMap((group) => group.nodes.map((node) => Object.freeze({ ...node, bounds: unitRect }))) ?? [];
  if (textElements.length === 0 && instruments.length === 0 && frameRateLabel === null) return NO_HEAD_HUD_VIEW;
  if (sharedTheme === null) return NO_HEAD_HUD_VIEW;
  if (viewer.source === "unavailable") return Object.freeze({ kind: "unavailable", reason: viewer.reason });

  const distance = profile.distanceMeters;
  const localPose = pose(vec3(0, 0, -distance), IDENTITY_POSE.orientation);
  const textHeightMeters = distance * Math.tan(profile.textHeightDegrees * Math.PI / 180);
  const clearHalfHeight = distance * Math.tan(profile.clearVerticalHalfAngleDegrees * Math.PI / 180);
  let clearTop = clearHalfHeight;
  let clearBottom = -clearHalfHeight;
  let clearLeft = -distance * Math.tan(profile.clearHorizontalHalfAngleDegrees * Math.PI / 180);
  let clearRight = -clearLeft;
  for (const eye of viewer.eyes) for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
    const direction = rotateVec3(eye.headFromEye.orientation, vec3(
      horizontal * Math.tan(profile.clearHorizontalHalfAngleDegrees * Math.PI / 180),
      vertical * Math.tan(profile.clearVerticalHalfAngleDegrees * Math.PI / 180), -1));
    const intersectionDistance = (-distance - eye.headFromEye.position.z) / direction.z;
    if (!Number.isFinite(intersectionDistance) || intersectionDistance <= 0) return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
    const horizontalPoint = eye.headFromEye.position.x + intersectionDistance * direction.x;
    const verticalPoint = eye.headFromEye.position.y + intersectionDistance * direction.y;
    clearTop = Math.max(clearTop, verticalPoint);
    clearBottom = Math.min(clearBottom, verticalPoint);
    clearLeft = Math.min(clearLeft, horizontalPoint);
    clearRight = Math.max(clearRight, horizontalPoint);
  }
  const padding = textHeightMeters * HEAD_HUD_CARD_PADDING;
  const gap = textHeightMeters * HEAD_HUD_CARD_GAP;
  for (let step = 0; step <= 24; step++) {
    const height = profile.maximumHeightMeters * (1 - step / 32);
    const topBandHeight = height / 2 - clearTop - gap - padding;
    const menuTop = -distance * (FLIGHT_MENU_GEOMETRY.centerY + FLIGHT_MENU_GEOMETRY.height / 2) / FLIGHT_MENU_GEOMETRY.distanceMeters;
    const bottomBandHeight = Math.min(height / 2 + clearBottom - gap - padding, menuTop + clearBottom - 2 * gap);
    if (topBandHeight <= 0 || bottomBandHeight <= 0) break;
    let lower = 0;
    let upper = profile.maximumWidthMeters;
    if (!headPlaneFitsViews(viewer, localPose, textHeightMeters, height, profile.clipMargin)) continue;
    for (let iteration = 0; iteration < 28; iteration++) {
      const candidate = (lower + upper) / 2;
      if (headPlaneFitsViews(viewer, localPose, candidate, height, profile.clipMargin)) lower = candidate;
      else upper = candidate;
    }
    const width = lower;
    const cardWidth = width - 2 * padding;
    if (cardWidth < 8 * textHeightMeters) continue;
    const topHeights = textElements.map((element) => minimumCardHeight(element, cardWidth, textHeightMeters, padding));
    const frameRateHeight = frameRateLabel === null ? 0 : (2 * HEAD_HUD_CARD_PADDING + HEAD_HUD_LINE_HEIGHT) * textHeightMeters;
    const frameRateOffset = frameRateHeight === 0 ? 0 : frameRateHeight + gap;
    const textBandHeight = stackedHeight(topHeights, gap) + frameRateOffset;
    if (textBandHeight > topBandHeight) continue;
    const maximumColumns = Math.max(1, ...instrumentGroups.map((group) => group.layout.kind === "column" ? 1
      : group.layout.kind === "row" ? group.nodes.length : group.layout.columns));
    for (let columnLimit = maximumColumns; columnLimit > 0; columnLimit--) {
      const instrumentRows = instrumentGroups.flatMap((group) => {
        const columns = Math.min(columnLimit, group.layout.kind === "column" ? 1
          : group.layout.kind === "row" ? Math.max(1, group.nodes.length) : group.layout.columns);
        const instrumentWidth = (cardWidth - (columns - 1) * gap) / columns;
        return Array.from({ length: Math.ceil(group.nodes.length / columns) }, (_, row) => {
          const nodes = group.nodes.slice(row * columns, (row + 1) * columns);
          return { nodes, width: instrumentWidth, height: Math.max(...nodes.map((node) =>
            minimumCardHeight({ ...node, bounds: unitRect }, instrumentWidth, textHeightMeters, padding))) };
        });
      });
      if (instrumentRows.some((row) => row.width < 8 * textHeightMeters)) continue;
      const rows = instrumentRows.length;
      const rowHeights = instrumentRows.map((row) => row.height);
      const bottomRows = Array.from({ length: rows + 1 }, (_, index) => rows - index).find((count) =>
        stackedHeight(rowHeights.slice(0, count), gap) <= bottomBandHeight &&
        textBandHeight + (textElements.length > 0 && count < rows ? gap : 0) + stackedHeight(rowHeights.slice(count), gap) <= topBandHeight
      );
      if (bottomRows === undefined) continue;
      const elements: HeadHudElement[] = [];
      if (frameRateLabel !== null) {
        const frameRateWidth = Math.min(cardWidth, (frameRateLabel.length + 2) * textHeightMeters);
        elements.push(Object.freeze({ id: "head-frame-rate", label: "", content: Object.freeze({ kind: "text", value: frameRateLabel, tone: "normal" }),
          bounds: rect((width - padding - frameRateWidth) / width, padding / height, frameRateWidth / width, frameRateHeight / height) }));
      }
      let top = padding + frameRateOffset;
      textElements.forEach((element, index) => {
        const cardHeight = topHeights[index] ?? 0;
        elements.push(Object.freeze({ ...element, bounds: rect(padding / width, top / height, cardWidth / width, cardHeight / height) }));
        top += cardHeight + gap;
      });
      instrumentRows.forEach((instrumentRow, row) => {
        const upperRow = row >= bottomRows;
        const precedingRows = upperRow ? rowHeights.slice(bottomRows, row) : rowHeights.slice(0, row);
        const rowTop = (upperRow ? top : height / 2 - clearBottom + gap) + precedingRows.reduce((sum, value) => sum + value + gap, 0);
        instrumentRow.nodes.forEach((node, column) => elements.push(Object.freeze({ ...node, bounds: rect(
          (padding + column * (instrumentRow.width + gap)) / width,
          rowTop / height, instrumentRow.width / width, instrumentRow.height / height
        ) })));
      });
      const clearWidth = Math.min(width / 2, clearRight) - Math.max(-width / 2, clearLeft);
      return Object.freeze({
        kind: "visible", textHeightMeters,
        layer: Object.freeze({
          kind: "visible", anchor: "head", localPose, locale,
          size: Object.freeze({ width, height }),
          clearRegion: rect(0.5 + Math.max(-width / 2, clearLeft) / width, 0.5 - clearTop / height, clearWidth / width, (clearTop - clearBottom) / height),
          backgroundAlpha: model?.layout.opacity ?? sharedTheme.hud_opacity, foregroundAlpha: profile.foregroundAlpha,
          theme: sharedTheme,
          elements: Object.freeze(elements)
        })
      });
    }
  }
  return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
}

export function headHudElementValue(element: HeadHudElement): string {
  return element.content.value;
}

export function headHudInstrumentHeight(element: HeadHudElement, textHeight: number): number {
  switch (element.content.kind) {
    case "text": return 0;
    case "attitude": return textHeight * 5;
    case "heading_tape":
    case "direction":
    case "linear_gauge": return textHeight * 3;
  }
}

function minimumCardHeight(element: HeadHudElement, width: number, textHeight: number, padding: number): number {
  const charactersPerLine = Math.max(1, Math.floor((width - padding * 2) / textHeight));
  const rows = headHudElementValue(element).split("\n").reduce((sum, value) => sum + Math.max(1, Math.ceil(Array.from(value).length / charactersPerLine)), 0);
  const labelRows = element.label === "" ? 0 : Math.ceil(Array.from(element.label).length / charactersPerLine);
  return 2 * padding + (rows + labelRows) * textHeight * HEAD_HUD_LINE_HEIGHT + headHudInstrumentHeight(element, textHeight);
}

function stackedHeight(heights: readonly number[], gap: number): number {
  return heights.reduce((sum, value) => sum + value, 0) + Math.max(0, heights.length - 1) * gap;
}

const unitRect = Object.freeze({ left: 0, top: 0, width: 1, height: 1 });

function rect(left: number, top: number, width: number, height: number): HeadHudRect {
  return Object.freeze({ left, top, width, height });
}

function validateProfile(profile: HeadHudLayoutProfile): void {
  if (![profile.distanceMeters, profile.maximumWidthMeters, profile.maximumHeightMeters, profile.textHeightDegrees].every((value) => Number.isFinite(value) && value > 0) ||
      ![profile.clearHorizontalHalfAngleDegrees, profile.clearVerticalHalfAngleDegrees].every((value) => Number.isFinite(value) && value > 0 && value < 90) ||
      !Number.isFinite(profile.clipMargin) || profile.clipMargin < 0 || profile.clipMargin >= 1 ||
      !Number.isFinite(profile.foregroundAlpha) || profile.foregroundAlpha < 0 || profile.foregroundAlpha > 1 ||
      profile.textHeightDegrees >= 90) {
    throw new RangeError("Head HUD profile requires positive finite dimensions, valid angles, and alpha in [0, 1]");
  }
}
