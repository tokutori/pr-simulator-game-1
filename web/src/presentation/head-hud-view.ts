import { IDENTITY_POSE, pose, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { HeadHudElement, HeadHudRect, VisibleHeadHud } from "../render/contracts/head-hud.js";
import { headPlaneFitsViews } from "../render/contracts/viewer-frame.js";
import type { ViewerFrame, ViewerGeometryUnavailableReason } from "../render/contracts/viewer-frame.js";
import type { FlightHudModel } from "./flight-hud-model.js";
import { FLIGHT_MENU_GEOMETRY } from "../render/contracts/ui.js";

export interface HeadHudLayoutProfile {
  readonly distanceMeters: number;
  readonly maximumWidthMeters: number;
  readonly maximumHeightMeters: number;
  readonly clearHorizontalHalfAngleDegrees: number;
  readonly clearVerticalHalfAngleDegrees: number;
  readonly textHeightDegrees: number;
  readonly clipMargin: number;
  readonly backgroundAlpha: number;
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
  backgroundAlpha: 0.28,
  foregroundAlpha: 1
});

export function createHeadHudView(model: FlightHudModel, viewer: ViewerFrame, locale: string, profile: HeadHudLayoutProfile = DEFAULT_HEAD_HUD_PROFILE): HeadHudView {
  if (locale.trim() === "") throw new RangeError("Head HUD display locale must be explicit");
  validateProfile(profile);
  const instruments = instrumentElements(model);
  const readouts = model.readouts.split("\n").filter((line) => !line.startsWith("PITCH ")).join("\n");
  const basicText = [readouts, model.telemetry].filter((line) => line !== "").join("\n").replaceAll(" · ", "\n");
  const text = [basicText, ...model.supplementaryReadouts].filter((line) => line !== "").join("\n");
  const textElements: HeadHudElement[] = [];
  if (model.warning !== null) textElements.push(Object.freeze({
    kind: "text", id: "head-warning", label: "WARNING", bounds: unitRect,
    value: model.warning, tone: "warning"
  }));
  if (text !== "") textElements.push(Object.freeze({
    kind: "text", id: "head-readouts", label: model.status, bounds: unitRect, value: text, tone: "normal"
  }));
  if (textElements.length === 0 && instruments.length === 0) return NO_HEAD_HUD_VIEW;
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
    const textBandHeight = stackedHeight(topHeights, gap);
    if (textBandHeight > topBandHeight) continue;
    for (const columns of [3, 2, 1]) {
      if (instruments.length > 0 && columns > instruments.length) continue;
      const instrumentWidth = (cardWidth - (columns - 1) * gap) / columns;
      if (instruments.length !== 0 && instrumentWidth < 8 * textHeightMeters) continue;
      const rows = Math.ceil(instruments.length / columns);
      const rowHeights = Array.from({ length: rows }, (_, row) => Math.max(...instruments.slice(row * columns, (row + 1) * columns)
        .map((element) => minimumCardHeight(element, instrumentWidth, textHeightMeters, padding))));
      const bottomRows = Array.from({ length: rows + 1 }, (_, index) => rows - index).find((count) =>
        stackedHeight(rowHeights.slice(0, count), gap) <= bottomBandHeight &&
        textBandHeight + (textElements.length > 0 && count < rows ? gap : 0) + stackedHeight(rowHeights.slice(count), gap) <= topBandHeight
      );
      if (bottomRows === undefined) continue;
      const elements: HeadHudElement[] = [];
      let top = padding;
      textElements.forEach((element, index) => {
        const cardHeight = topHeights[index] ?? 0;
        elements.push(Object.freeze({ ...element, bounds: rect(padding / width, top / height, cardWidth / width, cardHeight / height) }));
        top += cardHeight + gap;
      });
      instruments.forEach((element, index) => {
        const row = Math.floor(index / columns);
        const upperRow = row >= bottomRows;
        const precedingRows = upperRow ? rowHeights.slice(bottomRows, row) : rowHeights.slice(0, row);
        const rowTop = (upperRow ? top : height / 2 - clearBottom + gap) + precedingRows.reduce((sum, value) => sum + value + gap, 0);
        elements.push(Object.freeze({ ...element, bounds: rect(
          (padding + (index % columns) * (instrumentWidth + gap)) / width,
          rowTop / height,
          instrumentWidth / width, (rowHeights[row] ?? 0) / height
        ) }));
      });
      const clearWidth = Math.min(width / 2, clearRight) - Math.max(-width / 2, clearLeft);
      return Object.freeze({
        kind: "visible", textHeightMeters,
        layer: Object.freeze({
          kind: "visible", anchor: "head", localPose, locale,
          size: Object.freeze({ width, height }),
          clearRegion: rect(0.5 + Math.max(-width / 2, clearLeft) / width, 0.5 - clearTop / height, clearWidth / width, (clearTop - clearBottom) / height),
          backgroundAlpha: profile.backgroundAlpha, foregroundAlpha: profile.foregroundAlpha,
          elements: Object.freeze(elements)
        })
      });
    }
  }
  return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
}

export function headHudElementValue(element: HeadHudElement): string {
  switch (element.kind) {
    case "attitude": return `P ${element.pitchDegrees.toFixed(0)}°  R ${element.rollDegrees.toFixed(0)}°`;
    case "heading": return `${element.degrees.toFixed(0)}°`;
    case "text":
    case "pilot-position":
    case "wind":
    case "angle-of-attack":
    case "flight-path": return element.value;
  }
}

export function headHudInstrumentHeight(element: HeadHudElement, textHeight: number): number {
  return element.kind === "text" ? 0 : textHeight * (element.kind === "attitude" ? 5 : 3);
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

function instrumentElements(model: FlightHudModel): readonly HeadHudElement[] {
  const elements: HeadHudElement[] = [];
  if (model.attitude !== null) elements.push(Object.freeze({
    kind: "attitude", id: "head-attitude", label: "ADI", bounds: unitRect,
    ...model.attitude, flightPathAngleDegrees: model.flightPathAngleDegrees
  }));
  if (model.headingDegrees !== null) elements.push(Object.freeze({
    kind: "heading", id: "head-heading", label: "HDG", bounds: unitRect, degrees: model.headingDegrees
  }));
  if (model.pilotPositionRatio !== null && model.pilotPosition !== null) elements.push(Object.freeze({
    kind: "pilot-position", id: "head-pilot", label: "PILOT CG", bounds: unitRect,
    ratio: model.pilotPositionRatio, value: model.pilotPosition
  }));
  if (model.pilotPositionRatio === null && model.pilotPosition !== null) elements.push(Object.freeze({
    kind: "text", id: "head-pilot", label: "PILOT POSITION", bounds: unitRect, value: model.pilotPosition, tone: "normal"
  }));
  if (model.wind !== null) elements.push(model.windDirectionDegrees === null
    ? Object.freeze({ kind: "text", id: "head-wind", label: "WIND", bounds: unitRect, value: model.wind, tone: "normal" })
    : Object.freeze({ kind: "wind", id: "head-wind", label: "WIND", bounds: unitRect, degrees: model.windDirectionDegrees, value: model.wind }));
  if (model.angleOfAttack !== null) elements.push(model.angleOfAttackDegrees === null
    ? Object.freeze({ kind: "text", id: "head-aoa", label: "AOA", bounds: unitRect, value: "unavailable", tone: "normal" })
    : Object.freeze({ kind: "angle-of-attack", id: "head-aoa", label: "AOA", bounds: unitRect, degrees: model.angleOfAttackDegrees, value: model.angleOfAttack }));
  if (model.attitude === null && model.flightPathAngleDegrees !== null) elements.push(Object.freeze({
    kind: "flight-path", id: "head-flight-path", label: "FLIGHT PATH", bounds: unitRect,
    degrees: model.flightPathAngleDegrees, value: `${model.flightPathAngleDegrees.toFixed(1)}°`
  }));
  return Object.freeze(elements);
}

const unitRect = Object.freeze({ left: 0, top: 0, width: 1, height: 1 });

function rect(left: number, top: number, width: number, height: number): HeadHudRect {
  return Object.freeze({ left, top, width, height });
}

function validateProfile(profile: HeadHudLayoutProfile): void {
  if (![profile.distanceMeters, profile.maximumWidthMeters, profile.maximumHeightMeters, profile.textHeightDegrees].every((value) => Number.isFinite(value) && value > 0) ||
      ![profile.clearHorizontalHalfAngleDegrees, profile.clearVerticalHalfAngleDegrees].every((value) => Number.isFinite(value) && value > 0 && value < 90) ||
      !Number.isFinite(profile.clipMargin) || profile.clipMargin < 0 || profile.clipMargin >= 1 ||
      ![profile.backgroundAlpha, profile.foregroundAlpha].every((value) => Number.isFinite(value) && value >= 0 && value <= 1) ||
      profile.textHeightDegrees >= 90) {
    throw new RangeError("Head HUD profile requires positive finite dimensions, valid angles, and alpha in [0, 1]");
  }
}
