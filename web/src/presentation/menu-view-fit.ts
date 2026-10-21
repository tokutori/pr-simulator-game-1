import { IDENTITY_POSE } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import type { PanelSize } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { XrMenuViewFit } from "../generated/xr-menu-view-fit.js";
import { xr_menu_view_fit_json } from "../../pkg/birdman_game_wasm.js";

export type MenuViewFit = XrMenuViewFit;

export function fitMenuView(viewer: ViewerFrame, size: PanelSize, menuFromPanel: Pose = IDENTITY_POSE,
  minimumDistanceMeters = 2.4, clipMargin = 0.035): MenuViewFit {
  try {
    return JSON.parse(xr_menu_view_fit_json(JSON.stringify(viewer), JSON.stringify(menuFromPanel),
      size.width, size.height, minimumDistanceMeters, clipMargin)) as MenuViewFit;
  } catch (cause) {
    throw new RangeError(String(cause), { cause });
  }
}
