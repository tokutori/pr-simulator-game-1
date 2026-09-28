export interface PilotIntent {
  readonly roll: number;
  readonly pitch: number;
  readonly yaw: number;
  readonly pilotPositionMeters: number;
}

export interface KeyboardBindings {
  readonly rollPositive: string;
  readonly rollNegative: string;
  readonly pitchPositive: string;
  readonly pitchNegative: string;
  readonly yawPositive: string;
  readonly yawNegative: string;
  readonly pilotPositionIncrease: string;
  readonly pilotPositionDecrease: string;
}

export const DEFAULT_KEYBOARD_BINDINGS: KeyboardBindings = Object.freeze({
  rollPositive: "KeyD",
  rollNegative: "KeyA",
  pitchPositive: "ArrowUp",
  pitchNegative: "ArrowDown",
  yawPositive: "ArrowRight",
  yawNegative: "ArrowLeft",
  pilotPositionIncrease: "KeyL",
  pilotPositionDecrease: "KeyJ"
});

export function keyboardIntent(
  pressed: ReadonlySet<string>,
  previousPilotPositionMeters = 0,
  physicsHz = 100,
  bindings: KeyboardBindings = DEFAULT_KEYBOARD_BINDINGS,
  pilotPositionRangeMeters = 0.4,
  pilotPositionRateMetersPerSecond = 0.4
): PilotIntent {
  const movementDirection = axis(pressed, bindings.pilotPositionIncrease, bindings.pilotPositionDecrease);
  const pilotPositionMeters = Math.max(
    -pilotPositionRangeMeters,
    Math.min(pilotPositionRangeMeters, previousPilotPositionMeters + movementDirection * pilotPositionRateMetersPerSecond / physicsHz)
  );
  return Object.freeze({
    roll: axis(pressed, bindings.rollPositive, bindings.rollNegative),
    pitch: axis(pressed, bindings.pitchPositive, bindings.pitchNegative),
    yaw: axis(pressed, bindings.yawPositive, bindings.yawNegative),
    pilotPositionMeters
  });
}

function axis(pressed: ReadonlySet<string>, positive: string, negative: string): number {
  return Number(pressed.has(positive)) - Number(pressed.has(negative));
}
