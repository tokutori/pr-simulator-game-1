import type { Pose } from "./math.js";
import type { HeadHudPaintProjection } from "../../generated/head-hud-paint-reply.js";

export type VisibleHeadHud = HeadHudPaintProjection & { readonly kind: "visible" };
export type HeadHudLayer = { readonly kind: "absent" } | VisibleHeadHud;

export type HeadHudFrame =
  | { readonly kind: "absent" }
  | { readonly kind: "visible"; readonly trackingFromHead: Pose; readonly view: VisibleHeadHud };

export const NO_HEAD_HUD = { kind: "absent" } as const;

export function resolveHeadHudFrame(layer: HeadHudLayer, trackingFromHead: Pose | null): HeadHudFrame {
  return layer.kind === "absent" || trackingFromHead === null
    ? NO_HEAD_HUD
    : { kind: "visible", trackingFromHead, view: layer };
}
