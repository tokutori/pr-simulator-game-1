import { PerspectiveCamera, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import shoreline from "../../../../../assets/biwa-shoreline.json";
import { titleScreenCameraPoseForViewport } from "./three-renderer.js";

describe("title-screen camera composition", () => {
  it("keeps the Takeshima outline just outside the centered panel across viewport ratios", () => {
    const island = shoreline.islands.find((entry) => entry.name === "多景島");
    expect(island).toBeDefined();
    if (island === undefined) throw new TypeError("Missing Takeshima outline");
    const north = island.ringNorthEastMeters.reduce((sum, point) => sum + (point[0] ?? 0), 0) /
      island.ringNorthEastMeters.length;
    const east = island.ringNorthEastMeters.reduce((sum, point) => sum + (point[1] ?? 0), 0) /
      island.ringNorthEastMeters.length;

    for (const [width, height] of [[320, 640], [375, 812], [834, 844], [1280, 720], [1920, 1080]] as const) {
      const aspect = width / height;
      const cameraPose = titleScreenCameraPoseForViewport(width, height);
      const camera = new PerspectiveCamera(60, aspect, 0.05, 100_000);
      camera.position.set(cameraPose.position.x, cameraPose.position.y, cameraPose.position.z);
      camera.quaternion.set(
        cameraPose.orientation.x,
        cameraPose.orientation.y,
        cameraPose.orientation.z,
        cameraPose.orientation.w
      );
      camera.updateMatrixWorld();
      const screenX = new Vector3(east, 12, -north).project(camera).x;
      const panelWidth = Math.min(42 * 16, width - 32);
      const targetX = -Math.min(0.98, panelWidth / width + 24 / width);
      expect(Math.abs(screenX - targetX), `${String(width)}x${String(height)} screen-space error`).toBeLessThan(0.01);
    }
  });

  it("rejects invalid viewport aspect ratios", () => {
    expect(() => titleScreenCameraPoseForViewport(0, 720)).toThrow(RangeError);
    expect(() => titleScreenCameraPoseForViewport(1280, Number.NaN)).toThrow(RangeError);
  });
});
