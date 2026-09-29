export interface SyntheticMapPoint {
  readonly northMeters: number;
  readonly eastMeters: number;
}

export interface SyntheticMapLine {
  readonly id: string;
  readonly label: string;
  readonly color: string;
  readonly points: readonly SyntheticMapPoint[];
}

export interface SyntheticMapLandmark {
  readonly id: string;
  readonly label: string;
  readonly color: string;
  readonly point: SyntheticMapPoint;
}

export interface SyntheticVenueMap {
  readonly assetId: "synthetic-training-basin";
  readonly version: 1;
  readonly origin: "scenario-launch-composite-cg";
  readonly sourceNote: "schematic-only-not-geographic-data";
  readonly lines: readonly SyntheticMapLine[];
  readonly landmarks: readonly SyntheticMapLandmark[];
  readonly cameraPoints: readonly FixedCameraPoint[];
  readonly cameraPointsSha256: string;
}

const SYNTHETIC_TRAINING_BASIN: SyntheticVenueMap = Object.freeze({
  assetId: "synthetic-training-basin",
  version: 1,
  origin: "scenario-launch-composite-cg",
  sourceNote: "schematic-only-not-geographic-data",
  lines: Object.freeze([
    Object.freeze({
      id: "shoreline",
      label: "Synthetic shoreline (schematic; not geographic data)",
      color: "#70a9c3",
      points: Object.freeze([
        Object.freeze({ northMeters: -34, eastMeters: -60 }),
        Object.freeze({ northMeters: -26, eastMeters: -35 }),
        Object.freeze({ northMeters: -22, eastMeters: -10 }),
        Object.freeze({ northMeters: -22, eastMeters: 10 }),
        Object.freeze({ northMeters: -26, eastMeters: 35 }),
        Object.freeze({ northMeters: -34, eastMeters: 60 })
      ])
    }),
    Object.freeze({
      id: "launch-platform",
      label: "Synthetic platform (schematic; not geographic data)",
      color: "#e7c27b",
      points: Object.freeze([
        Object.freeze({ northMeters: -5, eastMeters: -14 }),
        Object.freeze({ northMeters: -5, eastMeters: 14 })
      ])
    })
  ]),
  landmarks: Object.freeze([
    Object.freeze({
      id: "training-islet",
      label: "Synthetic islet (schematic; not geographic data)",
      color: "#a9c986",
      point: Object.freeze({ northMeters: 120, eastMeters: 26 })
    })
  ]),
  cameraPoints: Object.freeze([
    Object.freeze({ id: "platform", northMeters: -16, eastMeters: -18, altitudeMeters: 14 }),
    Object.freeze({ id: "shore", northMeters: 85, eastMeters: -92, altitudeMeters: 26 }),
    Object.freeze({ id: "telephoto", northMeters: 230, eastMeters: 82, altitudeMeters: 35 })
  ]),
  cameraPointsSha256: "8a276ef8eb32b206fff17ba7079bec04ed4695c1f00374087824bdef18510abd"
});

export function syntheticVenueMapForScenario(scenarioId: number): SyntheticVenueMap | null {
  return Number.isInteger(scenarioId) && scenarioId >= 1 && scenarioId <= 5
    ? SYNTHETIC_TRAINING_BASIN
    : null;
}
import type { FixedCameraPoint } from "../render/contracts/camera.js";
