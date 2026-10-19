/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Engine-independent presentation of one committed or recorded aircraft state.
 */
export interface FlightRenderPose {
    /**
     * Airspeed used by the visual wing-flex model.
     */
    readonly airspeedMetersPerSecond: number | null;
    readonly attitudeBodyToNed: RenderAttitude;
    readonly controls: RenderTailControls;
    readonly datumPositionNed: RenderNedVector;
    /**
     * Pilot position at the beginning of this flight or record.
     */
    readonly initialPilotPositionMeters: number;
    /**
     * Current longitudinal pilot position.
     */
    readonly pilotPositionMeters: number;
    /**
     * Shared simulation time for environmental animation.
     */
    readonly simulationTimeSeconds: number;
    /**
     * Geometry resolved from the recorded model identity.
     */
    readonly tailGeometry: {
        readonly kind: "available";
        readonly value: RenderTailGeometryValue;
    } | {
        readonly kind: "unavailable";
        /**
         * Reason for the absent geometry.
         */
        readonly reason: "unregistered_aircraft_geometry";
    };
    /**
     * Ambient wind at the composite centre of gravity.
     */
    readonly windVelocityNedMetersPerSecond: RenderNedVector1 | null;
}
/**
 * Scalar-first body-to-NED orientation.
 */
export interface RenderAttitude {
    /**
     * Scalar component.
     */
    readonly w: number;
    /**
     * Body x component.
     */
    readonly x: number;
    /**
     * Body y component.
     */
    readonly y: number;
    /**
     * Body z component.
     */
    readonly z: number;
}
/**
 * Held physical control positions.
 */
export interface RenderTailControls {
    /**
     * Physical layout.
     */
    readonly layout: "tail_incidence";
    readonly physicalIncidence: RenderTailIncidence;
}
/**
 * Held physical incidence.
 */
export interface RenderTailIncidence {
    /**
     * Horizontal tail angle in radians.
     */
    readonly horizontalTailRadians: number;
    /**
     * Vertical tail angle in radians.
     */
    readonly verticalTailRadians: number;
}
/**
 * Fixed aircraft datum in NED metres.
 */
export interface RenderNedVector {
    /**
     * Down component.
     */
    readonly down: number;
    /**
     * East component.
     */
    readonly east: number;
    /**
     * North component.
     */
    readonly north: number;
}
/**
 * Model-derived presentation parameters.
 */
export interface RenderTailGeometryValue {
    /**
     * Aft distance to the horizontal tail's physical quarter-chord pivot.
     */
    readonly horizontalTailArmMeters: number;
}
/**
 * Named NED vector used by rendering APIs.
 */
export interface RenderNedVector1 {
    /**
     * Down component.
     */
    readonly down: number;
    /**
     * East component.
     */
    readonly east: number;
    /**
     * North component.
     */
    readonly north: number;
}
