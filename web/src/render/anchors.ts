import { composePose, IDENTITY_POSE, pose, vec3 } from "./contracts/math.js";
import type { Pose } from "./contracts/math.js";

export type AnchorKind = "world" | "cockpit" | "menu" | "head";

export interface AnchoredPose {
  readonly kind: AnchorKind;
  readonly localPose: Pose;
}

export interface AnchorFrames {
  readonly world: Pose;
  readonly cockpit: Pose;
  readonly menu: Pose;
  readonly head: Pose;
}

export function resolveAnchorPose(anchor: AnchoredPose, frames: AnchorFrames): Pose {
  return composePose(frames[anchor.kind], anchor.localPose);
}

export function placeMenuPanel(headPose: Pose, distanceMeters: number): Pose {
  if (!Number.isFinite(distanceMeters) || distanceMeters <= 0) throw new RangeError("Menu distance must be positive and finite");
  return composePose(headPose, pose(vec3(0, 0, -distanceMeters), IDENTITY_POSE.orientation));
}

export class MenuAnchorPlacement {
  private worldFromMenu: Pose | null = null;

  open(headPose: Pose, distanceMeters: number): Pose {
    if (this.worldFromMenu === null) this.worldFromMenu = placeMenuPanel(headPose, distanceMeters);
    return this.worldFromMenu;
  }

  recenter(headPose: Pose, distanceMeters: number): Pose {
    this.worldFromMenu = placeMenuPanel(headPose, distanceMeters);
    return this.worldFromMenu;
  }

  current(): Pose | null {
    return this.worldFromMenu;
  }
}
