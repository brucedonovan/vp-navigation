/** Side of the path a panel or branch sits on. */
export type Side = "left" | "right";

export interface ProjectedPanel {
  side: Side;
  x: number;
  y: number;
  scale: number;
  opacity: number;
  rotateY: number;
  zIndex: number;
}

export interface ProjectedNode {
  id: string;
  label: string;
  pathId: string;
  index: number;
  depth: number;
  active: boolean;
  interactive: boolean;
  panels: ProjectedPanel[];
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
  step(steps: number): void;
  focus(nodeId: string): void;
  activate(nodeId: string): void;
  takeBranch(branchId: string): void;
  returnToParent(): void;
  setReducedMotion(on: boolean): void;
  stateJson(): string;
  frameJson(): string;
}
