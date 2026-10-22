/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
export type FlightRenderPose = import("./flight-render-pose.js").FlightRenderPose;
export type TailSnapshotDocument = import("./tail-snapshot.js").TailSnapshotDocument;
/**
 * One shared frame's admitted work, input takeover and retained live snapshot.
 */
export interface FlightFrameDocument {
    /**
     * Common physical tick budget and interpolation remainder, or delay transition.
     */
    readonly advance: {
        /**
         * Number of tick callbacks invoked, including a final stopped attempt.
         */
        readonly count: number;
        /**
         * Remaining time as a fraction of the common physical tick.
         */
        readonly interpolation_fraction: number;
        readonly kind: "ticks";
        readonly [k: string]: unknown | undefined;
    } | {
        /**
         * Whether the most recent interval exceeds the common frame-gap limit.
         */
        readonly blocked: boolean;
        readonly kind: "processing_delay";
        readonly [k: string]: unknown | undefined;
    };
    /**
     * Selected connection's neutral and pilot-position takeover state.
     */
    readonly gamepad: {
        readonly kind: "unselected";
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity;
        readonly kind: "waiting_neutral";
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity1;
        readonly kind: "waiting_pilot";
        /**
         * Previous normalized axis position used to detect target crossing.
         */
        readonly previous_target: number;
        readonly [k: string]: unknown | undefined;
    } | {
        readonly identity: FlightDeviceIdentity2;
        readonly kind: "active";
        readonly [k: string]: unknown | undefined;
    };
    /**
     * A domain phase or pause-cause transition observed during this frame.
     */
    readonly phase_changed: boolean;
    /**
     * Shared interpolated display pose; paused and terminal frames use their exact committed state.
     */
    readonly render_pose: FlightRenderPose | null;
    /**
     * Live or terminal projection; menus and recorded playback have their own producer.
     */
    readonly snapshot: TailSnapshotDocument | null;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity1 {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
/**
 * Selected physical connection.
 */
export interface FlightDeviceIdentity2 {
    /**
     * Adapter-observed connection generation for this slot.
     */
    readonly generation: number;
    /**
     * High word of a native entity identity; zero for browser slots.
     */
    readonly slot_high: number;
    /**
     * Low word of a browser slot or native entity identity.
     */
    readonly slot_low: number;
}
