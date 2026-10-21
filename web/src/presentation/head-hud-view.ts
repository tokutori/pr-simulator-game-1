import type { HeadHudElement, HeadHudRect, VisibleHeadHud } from "../render/contracts/head-hud.js";
import type { ViewerFrame, ViewerGeometryUnavailableReason } from "../render/contracts/viewer-frame.js";
import type { FlightHudModel, FlightHudNode } from "./flight-hud-model.js";
import { formatFrameRate } from "./frame-rate.js";
import { xr_head_hud_projection_json } from "../../pkg/birdman_game_wasm.js";
import { DEFAULT_XR_HEAD_HUD_PROFILE } from "../generated/xr-head-hud-profile.js";
import type { XrHeadHudProfile } from "../generated/xr-head-hud-profile.js";
import type { XrHeadHudProjection } from "../generated/xr-head-hud-projection.js";

export { DEFAULT_XR_HEAD_HUD_PROFILE as DEFAULT_HEAD_HUD_PROFILE };
export type HeadHudLayoutProfile = XrHeadHudProfile;
export type HeadHudUnavailableReason = ViewerGeometryUnavailableReason | "insufficient-view-area" | "text-overflow";
export type HeadHudView =
  | { readonly kind: "absent" }
  | { readonly kind: "unavailable"; readonly reason: HeadHudUnavailableReason }
  | { readonly kind: "visible"; readonly layer: VisibleHeadHud; readonly textHeightMeters: number };

export const NO_HEAD_HUD_VIEW = Object.freeze({ kind: "absent" } as const);
export const HEAD_HUD_LINE_HEIGHT = 1.5;
export const HEAD_HUD_CARD_PADDING = 0.5;
export const HEAD_HUD_CARD_GAP = 0.6;

export function createHeadHudView(model: FlightHudModel | null, viewer: ViewerFrame, locale: string,
  profile: HeadHudLayoutProfile = DEFAULT_XR_HEAD_HUD_PROFILE, framesPerSecond?: number | null,
  sharedTheme: FlightHudModel["theme"] | null = model?.theme ?? null): HeadHudView {
  if (locale.trim() === "") throw new RangeError("Head HUD display locale must be explicit");
  const nodes = model?.groups.flatMap((group) => group.nodes) ?? [];
  const frameRateLabel = framesPerSecond === undefined ? null : formatFrameRate(framesPerSecond);
  if (nodes.length === 0 && frameRateLabel === null || sharedTheme === null) return NO_HEAD_HUD_VIEW;
  let fit: XrHeadHudProjection;
  try {
    fit = JSON.parse(xr_head_hud_projection_json(JSON.stringify(model === null ? null : { layout: model.layout, groups: model.groups }),
      JSON.stringify(viewer), JSON.stringify(profile), frameRateLabel)) as XrHeadHudProjection;
  } catch (cause) {
    throw new RangeError(String(cause), { cause });
  }
  if (fit.kind === "unavailable") return fit;
  const sourceNodes = new Map(nodes.map((node) => [node.id, node]));
  const elements: HeadHudElement[] = [];
  if (fit.frameRateBounds !== null && frameRateLabel !== null) {
    const bounds = fit.frameRateBounds;
    elements.push({ id: "head-frame-rate", label: "", content: { kind: "text", value: frameRateLabel, tone: "normal" }, instrument_height_em: 0,
      bounds: rect(bounds.x, bounds.y, bounds.width, bounds.height) });
  }
  for (const placement of fit.nodes) {
    const node = sourceNodes.get(placement.id);
    if (node === undefined) throw new Error(`Shared HUD placement has no source: ${placement.id}`);
    elements.push({ ...node, instrument_height_em: placement.instrument_height_em,
      bounds: rect(placement.bounds.x, placement.bounds.y, placement.bounds.width, placement.bounds.height) });
  }
  return { kind: "visible", textHeightMeters: fit.textHeightMeters,
    layer: { kind: "visible", anchor: "head", localPose: fit.localPose, locale,
      size: { width: fit.sizeM[0], height: fit.sizeM[1] },
      clearRegion: rect(fit.clearRegion.x, fit.clearRegion.y, fit.clearRegion.width, fit.clearRegion.height),
      backgroundAlpha: model?.layout.opacity ?? sharedTheme.hud_opacity, foregroundAlpha: profile.foregroundAlpha,
      theme: sharedTheme, elements: Object.freeze(elements) } };
}

export function headHudElementValue(element: Pick<FlightHudNode, "content">): string { return element.content.value; }
export function headHudInstrumentHeight(element: Pick<FlightHudNode, "instrument_height_em">, textHeight: number): number { return element.instrument_height_em * textHeight; }
function rect(left: number, top: number, width: number, height: number): HeadHudRect { return { left, top, width, height }; }
