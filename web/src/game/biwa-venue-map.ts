import analysisMap from "../../../assets/biwa-analysis-map.json";
import venueFeatures from "../../../assets/biwa-venue-features.json";
import type { FixedCameraPoint, FixedCameraPointId } from "../render/contracts/camera.js";
import { LAUNCH_PLATFORM } from "../render/contracts/launch-venue.js";
import type { SyntheticMapLandmark, SyntheticMapLine } from "./synthetic-venue-map.js";

export interface VenueMapDescriptor {
  readonly assetId: string;
  readonly version: number;
  readonly origin: string;
  readonly sourceNote: string;
  readonly lines: readonly SyntheticMapLine[];
  readonly landmarks: readonly SyntheticMapLandmark[];
  readonly cameraPoints: readonly FixedCameraPoint[];
  readonly cameraPointsSha256: string;
}

const launchBearing = LAUNCH_PLATFORM.launchBearingDegrees * Math.PI / 180;
const forwardNorth = Math.cos(launchBearing);
const forwardEast = Math.sin(launchBearing);
const rightNorth = -forwardEast;
const rightEast = forwardNorth;
const launchCorners = [
  { along: 0, across: -1 },
  { along: 0, across: 1 },
  { along: -LAUNCH_PLATFORM.lengthMeters, across: 1 },
  { along: -LAUNCH_PLATFORM.lengthMeters, across: -1 },
  { along: 0, across: -1 }
].map(({ along, across }) => Object.freeze({
  northMeters: along * forwardNorth + across * LAUNCH_PLATFORM.widthMeters / 2 * rightNorth,
  eastMeters: along * forwardEast + across * LAUNCH_PLATFORM.widthMeters / 2 * rightEast
}));

const launchLine: SyntheticMapLine = Object.freeze({
  id: "launch-platform",
  label: "発進台",
  color: "#e7c27b",
  points: Object.freeze(launchCorners)
});

const shorelineLines: SyntheticMapLine[] = analysisMap.shorelinesNorthEastMeters.map((line, index) => Object.freeze({
  id: `hikone-shoreline-${String(index + 1)}`,
  label: index === 0 ? "彦根 湖岸線（OSM）" : "彦根 湖岸線（OSM・続き）",
  color: "#77b6c8",
  points: Object.freeze(line.map((point) => {
    const northMeters = point[0];
    const eastMeters = point[1];
    if (northMeters === undefined || eastMeters === undefined) throw new TypeError("Invalid Analysis shoreline point");
    return Object.freeze({ northMeters, eastMeters });
  }))
}));

const venueFeatureLines: SyntheticMapLine[] = venueFeatures.features
  .filter((feature) => feature.kind === "pier" || feature.kind === "quay" || feature.kind === "breakwater" ||
    (feature.kind === "beach" && feature.name === "松原水泳場"))
  .map((feature) => Object.freeze({
    id: feature.id,
    label: feature.name ?? featureLabel(feature.kind),
    color: feature.kind === "beach" ? "#d8c89b" : "#a08f70",
    points: Object.freeze(feature.northEastMeters.map((point) => {
      const northMeters = point[0];
      const eastMeters = point[1];
      if (northMeters === undefined || eastMeters === undefined) throw new TypeError("Invalid venue feature point");
      return Object.freeze({ northMeters, eastMeters });
    }))
  }));

function featureLabel(kind: typeof venueFeatures.features[number]["kind"]): string {
  switch (kind) {
    case "beach": return "砂浜（OSM）";
    case "woodland": return "樹林（OSM）";
    case "tree-row": return "並木（OSM）";
    case "pier": return "桟橋（OSM）";
    case "quay": return "岸壁（OSM）";
    case "breakwater": return "防波堤（OSM）";
    default: throw new TypeError("Unknown OSM venue feature kind");
  }
}

const cameraPoints: readonly FixedCameraPoint[] = Object.freeze(
  analysisMap.cameraPoints.map((point) => {
    if (!isFixedCameraPointId(point.id)) throw new TypeError("Invalid Lake Biwa camera point ID");
    return Object.freeze({ ...point, id: point.id });
  })
);

function isFixedCameraPointId(value: string): value is FixedCameraPointId {
  return value === "platform" || value === "shore" || value === "telephoto";
}

const BIWA_VENUE_MAP: VenueMapDescriptor = Object.freeze({
  assetId: "biwa-hikone-launch-venue",
  version: analysisMap.sourceRelationVersion,
  origin: "launch-origin-wgs84-35.294075-136.254448",
  sourceNote: "OpenStreetMap contributors, ODbL-1.0; locally clipped Lake Biwa shoreline and Matsubara beach/pier/quay/breakwater features.",
  lines: Object.freeze([...shorelineLines, ...venueFeatureLines, launchLine]),
  landmarks: Object.freeze(venueFeatures.features.filter((feature) => feature.kind === "beach" && feature.name !== null)
    .map((feature) => {
      const northMeters = feature.northEastMeters.reduce((sum, point) => sum + (point[0] ?? 0), 0) / feature.northEastMeters.length;
      const eastMeters = feature.northEastMeters.reduce((sum, point) => sum + (point[1] ?? 0), 0) / feature.northEastMeters.length;
      return Object.freeze({ id: feature.id, label: feature.name ?? "砂浜", color: "#d8c89b", point: Object.freeze({ northMeters, eastMeters }) });
    })),
  cameraPoints,
  cameraPointsSha256: analysisMap.cameraPointsSha256
});

export function venueMapForScenario(scenarioId: number): VenueMapDescriptor | null {
  return Number.isInteger(scenarioId) && scenarioId >= 1 && scenarioId <= 5 ? BIWA_VENUE_MAP : null;
}
