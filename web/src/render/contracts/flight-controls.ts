export type PhysicalFlightControls =
  | Readonly<{ layout: "legacy_three_axis"; rollRadians: number; pitchRadians: number; yawRadians: number }>
  | Readonly<{ layout: "tail_incidence"; physicalIncidence: Readonly<{ horizontalTailRadians: number; verticalTailRadians: number }> }>;

export type LegacyPhysicalFlightControls = Extract<PhysicalFlightControls, { layout: "legacy_three_axis" }>;
export type TailPhysicalFlightControls = Extract<PhysicalFlightControls, { layout: "tail_incidence" }>;

export type TailPresentationGeometry =
  | Readonly<{ kind: "bpg041_version_one"; horizontalTailArmMeters: 1.8 }>
  | Readonly<{ kind: "bpg041_playable_version_two"; horizontalTailArmMeters: 3.6 }>;

export type TailPresentationGeometryAvailability =
  | Readonly<{ kind: "available"; value: TailPresentationGeometry }>
  | Readonly<{ kind: "unavailable"; reason: "unregistered_aircraft_geometry" }>;
