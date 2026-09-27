import { describe, expect, it } from "vitest";
import { vec3 } from "../../web/src/render/contracts/math.js";
import { selectRayFromXrEvent } from "../../web/src/render/engines/three/xr-select-ray.js";

describe("Three.js WebXR select-ray adapter", () => {
  it("ignores a select event when XRFrame.getPose returns null", () => {
    const frame = { predictedDisplayTime: 15, getPose: () => null };
    expect(selectRayFromXrEvent(frame, inputSource(), referenceSpace())).toBeNull();
  });

  it("converts a resolved target-ray pose into the engine-neutral selection ray", () => {
    const frame = {
      predictedDisplayTime: 25,
      getPose: () => ({
        transform: {
          position: { x: 1, y: 2, z: 3 },
          orientation: { x: 0, y: 0, z: 0, w: 1 }
        }
      })
    };
    expect(selectRayFromXrEvent(frame, inputSource(), referenceSpace())).toEqual({
      origin: vec3(1, 2, 3),
      direction: vec3(0, 0, -1),
      timestampMs: 25
    });
  });
});

function inputSource(): { readonly targetRaySpace: object } {
  return { targetRaySpace: {} };
}

function referenceSpace(): object {
  return {};
}
