/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Shared measured-panel reservation below the forward view.
 */
export interface XrFlightMenuGeometry {
    /**
     * Plane's head-local centre height in metres.
     */
    readonly centerY: number;
    /**
     * Plane distance in metres.
     */
    readonly distanceMeters: number;
    /**
     * Plane height in metres.
     */
    readonly height: number;
    /**
     * Plane width in metres.
     */
    readonly width: number;
}

export const XR_FLIGHT_MENU_GEOMETRY = Object.freeze({
  "centerY": -1.12,
  "distanceMeters": 2.4,
  "height": 0.25,
  "width": 0.62
} as const);
