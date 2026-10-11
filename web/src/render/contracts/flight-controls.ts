export type PhysicalFlightControls =
  | Readonly<{ layout: "tail_incidence"; physicalIncidence: Readonly<{ horizontalTailRadians: number; verticalTailRadians: number }> }>;

export type TailPhysicalFlightControls = Extract<PhysicalFlightControls, { layout: "tail_incidence" }>;

export type TailPresentationGeometry =
  Readonly<{ kind: "bpg041_playable_version_two"; horizontalTailArmMeters: 3.6 }>;

export type TailPresentationGeometryAvailability =
  | Readonly<{ kind: "available"; value: TailPresentationGeometry }>
  | Readonly<{ kind: "unavailable"; reason: "unregistered_aircraft_geometry" }>;
