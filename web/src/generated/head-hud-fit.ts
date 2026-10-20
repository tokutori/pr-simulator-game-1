/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
/**
 * Exhaustive head-layout result; insufficient space never omits an element.
 */
export type GameUiHeadHudFit = {
    readonly kind: "fitted";
    /**
     * Complete source-node placement in the original shared order.
     */
    readonly nodes: readonly GameUiHeadHudNode[];
} | {
    readonly kind: "unavailable";
};
/**
 * Shared placement of one retained element in the head-fixed plane.
 */
export interface GameUiHeadHudNode {
    readonly bounds: GameUiRect;
    /**
     * Identity of the unchanged source node.
     */
    readonly id: string;
    /**
     * Shared instrument height in multiples of the required text height.
     */
    readonly instrument_height_em: number;
}
/**
 * Normalized left/top rectangle within the complete HUD plane.
 */
export interface GameUiRect {
    /**
     * Height.
     */
    readonly height: number;
    /**
     * Width.
     */
    readonly width: number;
    /**
     * Left edge.
     */
    readonly x: number;
    /**
     * Top edge.
     */
    readonly y: number;
}
