/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Current Web replay camera candidates in their shared selection order.
 */
export type GameUiReplayCamera = "auto" | "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";
export type FlightRenderPose = import("./flight-render-pose.js").FlightRenderPose;
export type RecordedPlaybackSample = import("./record-sample.js").RecordedPlaybackSample;
/**
 * Recorded camera projection; unavailable fixed points retain the existing Chase fallback.
 */
export interface ReplayCameraView {
    readonly camera: GameUiReplayCamera;
    readonly cinematic: CinematicCameraView | null;
    readonly render_pose: FlightRenderPose;
    readonly sample: RecordedPlaybackSample;
}
/**
 * Absolute pose in render axes `(east, up, -north)`, with local camera forward `-Z`.
 */
export interface CinematicCameraView {
    /**
     * @minItems 4
     * @maxItems 4
     */
    readonly orientation_wxyz: readonly [
        number,
        number,
        number,
        number
    ];
    /**
     * @minItems 3
     * @maxItems 3
     */
    readonly position_m: readonly [
        number,
        number,
        number
    ];
    readonly vertical_field_of_view_degrees: number;
}
