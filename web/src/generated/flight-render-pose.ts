/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Control layout accepted by the current live session boundary.
 */
export type TailControlLayoutDocument = "tail_incidence";
/**
 * Explicit geometry availability for current and imported model identities.
 */
export type RenderTailGeometry = {
    readonly kind: "available";
    readonly value: RenderTailGeometryValue;
} | {
    readonly kind: "unavailable";
    readonly reason: RenderGeometryUnavailable;
};
/**
 * Geometry lookup failure carried to presentation adapters.
 */
export type RenderGeometryUnavailable = "unregistered_aircraft_geometry";
/**
 * Structural input supplied by the owning flight or canonical record query.
 */
export type RenderWingDeformation = {
    /**
     * Dimensionless bending coefficient held over its physical interval.
     */
    readonly factor: number;
    readonly kind: "accepted";
} | {
    readonly kind: "unavailable";
    readonly reason: RenderWingDeformationUnavailable;
};
/**
 * Structural input availability for saved and live sources.
 */
export type RenderWingDeformationUnavailable = "unrecorded" | "unregistered_aircraft_geometry";
/**
 * Engine-independent presentation of one committed or recorded aircraft state.
 */
export interface FlightRenderPose {
    /**
     * Observed airspeed at this physical or recorded state.
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
    readonly tailGeometry: RenderTailGeometry;
    /**
     * Ambient wind at the composite centre of gravity.
     */
    readonly windVelocityNedMetersPerSecond: RenderNedVector | null;
    readonly wingDeformation: RenderWingDeformation;
}
/**
 * Scalar-first orientation components used by rendering APIs.
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
 * Current two-tail layout and held incidence.
 */
export interface RenderTailControls {
    readonly layout: TailControlLayoutDocument;
    readonly physicalIncidence: RenderTailIncidence;
}
/**
 * Physical tail incidence in the renderer's public units.
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
 * Named NED vector used by rendering APIs.
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
 * Registered tail presentation parameters.
 */
export interface RenderTailGeometryValue {
    /**
     * Aft distance to the horizontal tail's physical quarter-chord pivot.
     */
    readonly horizontalTailArmMeters: number;
}
