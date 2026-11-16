/** Side of the path a branch sits on. */
export type Side = "left" | "right";

/** How a stop's views share the front, and which way they split. */
export type Orientation = "horizontal" | "vertical";

/** One view of a stop. `width` and `height` are viewport fractions and already include `scale`. */
export interface ProjectedView {
  id: string;
  emphasis: number;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  opacity: number;
  rotateX: number;
  rotateY: number;
  zIndex: number;
}

export interface ProjectedNode {
  id: string;
  label: string;
  pathId: string;
  index: number;
  depth: number;
  orientation: Orientation;
  active: boolean;
  interactive: boolean;
  /** Screen center of the stop. Views are laid out around this point. */
  x: number;
  y: number;
  /**
   * Degrees. Apply once to the whole stop, around `hingeX` / `hingeY`.
   * Each view's `rotateY` includes this yaw; subtract it so the columns stay on one plane.
   */
  yaw: number;
  /** Pivot of `yaw`, in viewport fractions. */
  hingeX: number;
  hingeY: number;
  views: ProjectedView[];
}

export interface ProjectedMarker {
  nodeId: string;
  pathId: string;
  x: number;
  y: number;
  halfWidth: number;
  opacity: number;
  scale: number;
  depth: number;
}

export interface RibbonSample {
  centerX: number;
  centerY: number;
  leftX: number;
  leftY: number;
  rightX: number;
  rightY: number;
  opacity: number;
  depth: number;
}

export interface Ribbon {
  pathId: string;
  role: "active" | "branch";
  samples: RibbonSample[];
}

export interface BranchHint {
  id: string;
  label: string;
  side: Side;
  fromNode: string;
  toPath: string;
  distance: number;
  inRange: boolean;
  x: number;
  y: number;
  opacity: number;
  scale: number;
  zIndex: number;
}

/** One projected frame. `x` and `y` are fractions of the viewport. */
export interface Frame {
  pathId: string;
  position: number;
  target: number;
  vanishX: number;
  vanishY: number;
  reducedMotion: boolean;
  nodes: ProjectedNode[];
  markers: ProjectedMarker[];
  ribbons: Ribbon[];
  hints: BranchHint[];
}

export interface StateNode {
  id: string;
  label: string;
  index: number;
}

export interface StateBranch {
  id: string;
  label: string;
  side: Side;
  fromNode: string;
  toPath: string;
  distance: number;
  inRange: boolean;
}

export interface NavState {
  pathId: string;
  position: number;
  target: number;
  atRest: boolean;
  /** The camera is holding on a stop so its views can be read. */
  holding: boolean;
  canReturn: boolean;
  returnLabel: string | null;
  currentNode: string;
  currentLabel: string;
  nodes: StateNode[];
  availableBranches: StateBranch[];
  turning: {
    branchId: string;
    direction: "enter" | "exit";
    align: number;
  } | null;
  pending: { kind: string; id: string } | null;
}

/**
 * The surface a DOM view needs from the core.
 * `VpNavigator` from the wasm package implements this directly.
 * A React or Svelte wrapper can implement the same methods.
 */
export interface NavigatorPort {
  tick(dtMs: number): void;
  moveBy(delta: number): void;
  /** Wheel sample. A flick advances one stop; a sustained stream travels through. */
  scroll(nowMs: number, delta: number): void;
  /** Land on a stop after a drag, or on the one stop a touch flick asked for. */
  park(stop: number): void;
  step(steps: number): void;
  focus(nodeId: string): void;
  activate(nodeId: string): void;
  takeBranch(branchId: string): void;
  returnToParent(): void;
  setReducedMotion(on: boolean): void;
  /** Milliseconds to pause on each stop a scroll would pass through. `0` disables it. */
  setDwellMs(ms: number): void;
  /** Merge a partial projection object. Missing keys keep their current values. */
  setProjection(json: string): void;
  projectionJson(): string;
  stateJson(): string;
  frameJson(): string;
}
