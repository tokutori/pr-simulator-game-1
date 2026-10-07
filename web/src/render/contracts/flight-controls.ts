export type PhysicalFlightControls =
  | Readonly<{ layout: "legacy_three_axis"; rollRadians: number; pitchRadians: number; yawRadians: number }>
  | Readonly<{ layout: "tail_incidence"; physicalIncidence: Readonly<{ horizontalTailRadians: number; verticalTailRadians: number }> }>;

export type LegacyPhysicalFlightControls = Extract<PhysicalFlightControls, { layout: "legacy_three_axis" }>;
export type TailPhysicalFlightControls = Extract<PhysicalFlightControls, { layout: "tail_incidence" }>;
