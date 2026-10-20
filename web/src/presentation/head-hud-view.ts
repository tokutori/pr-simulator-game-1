import { IDENTITY_POSE, pose, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { HeadHudElement, HeadHudRect, VisibleHeadHud } from "../render/contracts/head-hud.js";
import { headPlaneFitsViews } from "../render/contracts/viewer-frame.js";
import type { ViewerFrame, ViewerGeometryUnavailableReason } from "../render/contracts/viewer-frame.js";
import type { FlightHudModel, FlightHudNode } from "./flight-hud-model.js";
import { FLIGHT_MENU_GEOMETRY } from "../render/contracts/ui.js";
import { formatFrameRate } from "./frame-rate.js";
import { game_ui_head_hud_fit_json } from "../../pkg/birdman_game_wasm.js";
import type { GameUiHeadHudFit } from "../generated/head-hud-fit.js";
import type { GameUiHeadHudSpace } from "../generated/head-hud-space.js";

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
  const nodes = model?.groups.flatMap((group) => group.nodes) ?? [];
  const sourceNodes = new Map(nodes.map((node) => [node.id, node]));
  const frameRateLabel = framesPerSecond === undefined ? null : formatFrameRate(framesPerSecond);
  if (nodes.length === 0 && frameRateLabel === null) return NO_HEAD_HUD_VIEW;
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
    const frameRateHeight = frameRateLabel === null ? 0 : (2 * HEAD_HUD_CARD_PADDING + HEAD_HUD_LINE_HEIGHT) * textHeightMeters;
    const frameRateOffset = frameRateHeight === 0 ? 0 : frameRateHeight + gap;
      const space: GameUiHeadHudSpace = {
        size_m: [width, height], text_height_m: textHeightMeters,
        top_band_m: [padding + frameRateOffset, Math.max(0, topBandHeight - frameRateOffset)],
        bottom_band_m: [height / 2 - clearBottom + gap, bottomBandHeight]
      };
      const fitted = model === null ? { kind: "fitted" as const, nodes: [] }
        : JSON.parse(game_ui_head_hud_fit_json(JSON.stringify({ layout: model.layout, groups: model.groups }), JSON.stringify(space))) as GameUiHeadHudFit;
      if (fitted.kind === "unavailable") continue;
      const elements: HeadHudElement[] = [];
      if (frameRateLabel !== null) {
        const frameRateWidth = Math.min(cardWidth, (frameRateLabel.length + 2) * textHeightMeters);
        elements.push(Object.freeze({ id: "head-frame-rate", label: "", content: Object.freeze({ kind: "text", value: frameRateLabel, tone: "normal" }),
          instrument_height_em: 0,
          bounds: rect((width - padding - frameRateWidth) / width, padding / height, frameRateWidth / width, frameRateHeight / height) }));
      }
      for (const placement of fitted.nodes) {
        const node = sourceNodes.get(placement.id);
        if (node === undefined) throw new Error(`Shared HUD placement has no source: ${placement.id}`);
        elements.push(Object.freeze({ ...node, instrument_height_em: placement.instrument_height_em,
          bounds: rect(placement.bounds.x, placement.bounds.y, placement.bounds.width, placement.bounds.height) }));
      }
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
  return Object.freeze({ kind: "unavailable", reason: "insufficient-view-area" });
}

export function headHudElementValue(element: Pick<FlightHudNode, "content">): string {
  return element.content.value;
}

export function headHudInstrumentHeight(element: Pick<FlightHudNode, "instrument_height_em">, textHeight: number): number {
  return element.instrument_height_em * textHeight;
}

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
