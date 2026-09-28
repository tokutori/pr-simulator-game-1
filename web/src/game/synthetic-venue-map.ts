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
  ])
});

export function syntheticVenueMapForScenario(scenarioId: number): SyntheticVenueMap | null {
  return Number.isInteger(scenarioId) && scenarioId >= 1 && scenarioId <= 5
    ? SYNTHETIC_TRAINING_BASIN
    : null;
}
