import type { GameUiView } from "../generated/game-ui-view.js";

type SharedHud = NonNullable<GameUiView["hud"]>;

export interface FlightHudModel {
  readonly layout: SharedHud["layout"];
  readonly geometry: SharedHud["geometry"];
  readonly theme: GameUiView["theme"];
  readonly labels: Readonly<{
    heading: SharedHud["heading_label"];
    pilotPosition: SharedHud["pilot_position_label"];
    wind: SharedHud["wind_label"];
    angleOfAttack: SharedHud["angle_of_attack_label"];
  }>;
  readonly status: SharedHud["status"];
  readonly attitude: Readonly<{ rollDegrees: number; pitchDegrees: number }> | null;
  readonly flightPathAngleDegrees: SharedHud["flight_path_angle_degrees"];
  readonly warning: SharedHud["warning"];
  readonly headingDegrees: SharedHud["heading_degrees"];
  readonly windDirectionDegrees: SharedHud["wind_direction_degrees"];
  readonly angleOfAttackDegrees: SharedHud["angle_of_attack_degrees"];
  readonly readouts: SharedHud["readouts"];
  readonly heading: SharedHud["heading"];
  readonly pilotPosition: SharedHud["pilot_position"];
  readonly wind: SharedHud["wind"];
  readonly angleOfAttack: SharedHud["angle_of_attack"];
  readonly telemetry: SharedHud["telemetry"];
  readonly location: SharedHud["location"];
  readonly mapAttribution: SharedHud["map_attribution"];
  readonly supplementaryReadouts: SharedHud["supplementary_readouts"];
  readonly controlsDescription: SharedHud["controls_description"];
}

export function projectSharedFlightHudModel(shared: SharedHud, theme: GameUiView["theme"]): FlightHudModel {
  return Object.freeze({
    layout: shared.layout,
    geometry: shared.geometry,
    theme,
    labels: Object.freeze({ heading: shared.heading_label, pilotPosition: shared.pilot_position_label,
      wind: shared.wind_label, angleOfAttack: shared.angle_of_attack_label }),
    status: shared.status,
    attitude: shared.attitude === null ? null : Object.freeze({ rollDegrees: shared.attitude.roll_degrees, pitchDegrees: shared.attitude.pitch_degrees }),
    flightPathAngleDegrees: shared.flight_path_angle_degrees,
    warning: shared.warning,
    headingDegrees: shared.heading_degrees,
    windDirectionDegrees: shared.wind_direction_degrees,
    angleOfAttackDegrees: shared.angle_of_attack_degrees,
    readouts: shared.readouts,
    heading: shared.heading,
    pilotPosition: shared.pilot_position,
    wind: shared.wind,
    angleOfAttack: shared.angle_of_attack,
    telemetry: shared.telemetry,
    location: shared.location,
    mapAttribution: shared.map_attribution,
    supplementaryReadouts: shared.supplementary_readouts,
    controlsDescription: shared.controls_description
  });
}
