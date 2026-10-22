/* Generated from birdman-game-presentation::game_ui. Edit the Rust contract and regenerate. */
/**
 * Resource observations that jointly determine presentation admission.
 */
export type PresentationObservation = {
    readonly kind: "backend";
    /**
     * Whether the selected backend is ready to present the application.
     */
    readonly ready: boolean;
} | {
    readonly kind: "xr";
    /**
     * Retained observation; None releases the adapter's XR session.
     */
    readonly observation?: XrAvailability | null;
};
/**
 * Runtime visibility observed independently of input focus and tracking.
 */
export type XrVisibility = "visible" | "blurred" | "hidden";
/**
 * Same-session XR resource observations supplied by a platform adapter.
 */
export interface XrAvailability {
    /**
     * Whether the runtime grants application input focus.
     */
    readonly input_focused: boolean;
    /**
     * Whether the current frame has a usable head pose.
     */
    readonly tracking_valid: boolean;
    readonly visibility: XrVisibility;
}
