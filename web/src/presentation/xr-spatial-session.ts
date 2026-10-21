import { XrSpatialBridge } from "../../pkg/birdman_game_wasm.js";
import type { XrPanelObservation } from "../generated/xr-panel-observation.js";
import type { XrPanelPlacement } from "../generated/xr-panel-placement.js";
import type { XrReferenceFrame } from "../generated/xr-reference-frame.js";
import type { Pose } from "../render/contracts/math.js";
import type { UiPanel, UiViewModel } from "../render/contracts/ui.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";

export class XrSpatialSession {
  private bridge: XrSpatialBridge | null = null;

  place(view: Pick<UiViewModel, "scene" | "activeOverlay">, panel: UiPanel | null, viewer: ViewerFrame, reference: XrReferenceFrame | null): XrPanelPlacement {
    const observation: XrPanelObservation = panel === null ? { kind: "absent" } : { kind: "ready", geometry: {
      scope: { scene: view.scene, overlay: view.activeOverlay, panelId: panel.id }, anchor: panel.anchor,
      localPose: panel.localPose, sizeM: [panel.size.width, panel.size.height]
    } };
    return JSON.parse(this.owner().place_panel_json(JSON.stringify(observation), JSON.stringify(viewer), JSON.stringify(reference))) as XrPanelPlacement;
  }

  closeMenu(): void { this.bridge?.close_menu(); }

  recenterMenu(referenceFromHead: Pose): void {
    this.bridge?.recenter_menu_json(JSON.stringify(referenceFromHead));
  }

  recenterTracking(newReferenceFromPrevious: Pose): void {
    this.bridge?.recenter_tracking_json(JSON.stringify(newReferenceFromPrevious));
  }

  reset(): void {
    const previous = this.bridge;
    this.bridge = null;
    previous?.free();
  }

  private owner(): XrSpatialBridge {
    this.bridge ??= new XrSpatialBridge();
    return this.bridge;
  }
}
