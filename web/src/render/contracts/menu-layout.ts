import type { MenuLayoutReply } from "../../generated/menu-layout-reply.js";
export type { MenuScrollScope, MenuScrollContext, MenuScrollState, MenuScrollIntent } from "./ui.js";
export type { MenuRect } from "../../generated/menu-layout-reply.js";
export type { MenuPoint, MenuFont as MenuFontIdentity } from "../../generated/surface-paint-item.js";
export type MenuViewport = Extract<MenuLayoutReply, { kind: "ready" }>["projection"];
export type MenuLayoutUnavailableReason = Extract<MenuLayoutReply, { kind: "unavailable" }>["reason"];
export type MenuFontMeasurer = (value: string, fontMeters: number, weight: number, widthMeters: number, heightMeters: number) => Float64Array;
