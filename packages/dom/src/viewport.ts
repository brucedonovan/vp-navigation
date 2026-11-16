import type { Frame, NavState, NavigatorPort, ProjectedNode, ProjectedView } from "./types";

export interface ViewContent {
  id: string;
  element: HTMLElement;
}

export interface NodeContent {
  views: ViewContent[];
}

export interface ViewportOptions {
  root: HTMLElement;
  navigator: NavigatorPort;
  /** View elements keyed by node id, in column order. */
  content: Record<string, NodeContent>;
  /** Fires when the station, path, or available branch changes. */
  onChange?: (state: NavState) => void;
}

const BASE_CSS = `
.vp-root { position: relative; width: 100%; height: 100%; overflow: hidden; }
.vp-stage {
  position: absolute; inset: 0;
  perspective: 1400px;
  perspective-origin: var(--vx, 50%) var(--vy, 38%);
  transform-style: preserve-3d;
}
.vp-glow, .vp-ribbon, .vp-layers, .vp-hints { position: absolute; inset: 0; }
.vp-ribbon { width: 100%; height: 100%; overflow: visible; pointer-events: none; }
.vp-layers, .vp-hints {
  pointer-events: none;
  perspective: 1400px;
  perspective-origin: var(--vx, 50%) var(--vy, 38%);
}
.vp-node {
  position: absolute; left: 0; top: 0; width: 0; height: 0;
  transform-origin: 0 0;
  transform-style: preserve-3d;
  pointer-events: none;
}
.vp-glow { pointer-events: none; }
.vp-panel, .vp-hint {
  position: absolute; left: 0; top: 0;
  transform-origin: center center;
  pointer-events: auto;
}
.vp-stops { position: absolute; z-index: 5; }
.vp-sr {
  position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px;
  overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0;
}
`;

let baseInstalled = false;

function installBaseCss() {
  if (baseInstalled) return;
  baseInstalled = true;
  const style = document.createElement("style");
  style.id = "vp-navigation-base";
  style.textContent = BASE_CSS;
  document.head.insertBefore(style, document.head.firstChild);
}

/**
 * Renders a {@link NavigatorPort} into the DOM with CSS transforms.
 * Input is wheel, keyboard, touch, and click. The page itself does not scroll.
 */
export class DomViewport {
  private readonly nav: NavigatorPort;
  private readonly onChange?: (state: NavState) => void;
  private readonly root: HTMLElement;
  private readonly stage: HTMLElement;
  private readonly glow: HTMLElement;
  private readonly svg: SVGSVGElement;
  private readonly layers: HTMLElement;
  private readonly hints: HTMLElement;
  private readonly stops: HTMLElement;
  private readonly live: HTMLElement;
  private readonly views = new Map<string, HTMLElement>();
  private readonly groups = new Map<string, HTMLElement>();
  private readonly hintButtons = new Map<string, HTMLButtonElement>();
  private readonly mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  private reducedOverride: boolean | null = null;
  private stopsKey = "";
  private signature = "";
  private raf = 0;
  private width = 1;
  private height = 1;
  private touch: {
    y: number;
    ignore: boolean;
    origin: number;
    accum: number;
    t0: number;
  } | null = null;

  constructor(options: ViewportOptions) {
    installBaseCss();
    this.nav = options.navigator;
    this.onChange = options.onChange;

    this.root = document.createElement("div");
    this.root.className = "vp-root";
    this.stage = document.createElement("div");
    this.stage.className = "vp-stage";
    this.glow = document.createElement("div");
    this.glow.className = "vp-glow";
    this.svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    this.svg.classList.add("vp-ribbon");
    this.svg.setAttribute("aria-hidden", "true");
    this.layers = document.createElement("div");
    this.layers.className = "vp-layers";
    this.hints = document.createElement("div");
    this.hints.className = "vp-hints";
    this.stage.append(this.glow, this.svg, this.layers, this.hints);

    this.stops = document.createElement("nav");
    this.stops.className = "vp-stops";
    this.stops.setAttribute("aria-label", "Stops on this path");

    this.live = document.createElement("p");
    this.live.className = "vp-live vp-sr";
    this.live.setAttribute("aria-live", "polite");

    const help = document.createElement("p");
    help.className = "vp-sr";
    help.textContent =
      "Vanishing-point path. Arrow keys move between stops. Enter takes a branch. Escape returns to the path you left. You can also use the wheel, a swipe, or click a page.";

    this.root.append(this.stage, this.stops, help, this.live);
    options.root.replaceChildren(this.root);

    for (const [id, content] of Object.entries(options.content)) {
      this.mount(id, content);
    }

    this.mq.addEventListener("change", this.onMotionPreference);
    window.addEventListener("wheel", this.onWheel, { passive: false });
    window.addEventListener("keydown", this.onKey);
    this.root.addEventListener("touchstart", this.onTouchStart, { passive: true });
    this.root.addEventListener("touchmove", this.onTouchMove, { passive: false });
    this.root.addEventListener("touchend", this.onTouchEnd);

    const observer = new ResizeObserver(() => this.measure());
    observer.observe(this.root);
    this.measure();
    this.applyReduced();
  }

  start() {
    let last = performance.now();
    const loop = (now: number) => {
      const dt = Math.min(64, now - last);
      last = now;
      this.nav.tick(dt);
      this.render();
      this.raf = requestAnimationFrame(loop);
    };
    this.render();
    this.raf = requestAnimationFrame(loop);
  }

  /** `null` follows the operating system. `true` / `false` overrides it. */
  setReducedMotion(enabled: boolean | null) {
    this.reducedOverride = enabled;
    this.applyReduced();
  }

  destroy() {
    cancelAnimationFrame(this.raf);
    this.mq.removeEventListener("change", this.onMotionPreference);
    window.removeEventListener("wheel", this.onWheel);
    window.removeEventListener("keydown", this.onKey);
    this.root.removeEventListener("touchstart", this.onTouchStart);
    this.root.removeEventListener("touchmove", this.onTouchMove);
    this.root.removeEventListener("touchend", this.onTouchEnd);
  }

  private mount(id: string, content: NodeContent) {
    const group = document.createElement("div");
    group.className = "vp-node";
    group.dataset.nodeId = id;
    this.layers.append(group);
    this.groups.set(id, group);
    for (const view of content.views) {
      const el = view.element;
      el.classList.add("vp-panel");
      el.dataset.nodeId = id;
      el.dataset.viewId = view.id;
      el.addEventListener("click", () => this.activate(id));
      group.append(el);
      this.views.set(viewKey(id, view.id), el);
    }
  }

  private measure() {
    const rect = this.root.getBoundingClientRect();
    this.width = Math.max(1, rect.width);
    this.height = Math.max(1, rect.height);
    this.svg.setAttribute("viewBox", `0 0 ${this.width} ${this.height}`);
  }

  private render() {
    const frame = JSON.parse(this.nav.frameJson()) as Frame;
    const state = JSON.parse(this.nav.stateJson()) as NavState;
    const w = this.width;
    const h = this.height;

    this.root.style.setProperty("--vx", `${frame.vanishX * 100}%`);
    this.root.style.setProperty("--vy", `${frame.vanishY * 100}%`);
    this.glow.style.left = `${frame.vanishX * 100}%`;
    this.glow.style.top = `${frame.vanishY * 100}%`;

    const seen = new Set<string>();
    for (const node of frame.nodes) {
      const group = this.groups.get(node.id);
      if (group) this.placeGroup(group, node, w, h);
      for (const view of node.views) {
        const key = viewKey(node.id, view.id);
        seen.add(key);
        const el = this.views.get(key);
        if (!el) continue;
        this.placeView(el, view, node, w, h);
      }
    }
    for (const [key, el] of this.views) {
      if (!seen.has(key)) this.hide(el);
    }

    this.svg.innerHTML = this.ribbonMarkup(frame, w, h);
    this.placeHints(frame, w, h);
    this.renderStops(state);
    this.announce(state);
    this.publish(state);
  }

  private placeGroup(group: HTMLElement, node: ProjectedNode, w: number, h: number) {
    const yaw = node.yaw || 0;
    const hingeX = node.hingeX ?? node.x;
    const hingeY = node.hingeY ?? node.y;
    const z = node.views.reduce((highest, view) => Math.max(highest, view.zIndex), 0);
    group.style.left = `${node.x * w}px`;
    group.style.top = `${node.y * h}px`;
    group.style.zIndex = String(z);
    group.style.transformOrigin = `${((hingeX - node.x) * w).toFixed(2)}px ${((hingeY - node.y) * h).toFixed(2)}px`;
    group.style.transform = `rotateY(${yaw.toFixed(2)}deg)`;
  }

  private placeView(
    el: HTMLElement,
    view: ProjectedView,
    node: ProjectedNode,
    w: number,
    h: number,
  ) {
    const yaw = node.yaw || 0;
    // The group owns the camera yaw, so the columns stay on one plane.
    const spinY = view.rotateY - yaw;
    el.style.left = `${(view.x - node.x) * w}px`;
    el.style.top = `${(view.y - node.y) * h}px`;
    el.style.width = `${Math.max(view.width, 0) * w}px`;
    el.style.height = `${Math.max(view.height, 0) * h}px`;
    el.style.opacity = view.opacity.toFixed(3);
    el.style.zIndex = String(view.zIndex);
    el.style.transform = `translate(-50%, -50%) rotateX(${view.rotateX.toFixed(2)}deg) rotateY(${spinY.toFixed(2)}deg)`;
    el.style.pointerEvents = node.interactive && view.opacity > 0.4 ? "auto" : "none";
    el.classList.toggle("is-active", node.active);
    // Content the host mounted for another path is omitted from the frame.
    // Anything this faint is not part of the room a reader is in yet.
    if (view.opacity < 0.05) el.setAttribute("aria-hidden", "true");
    else el.removeAttribute("aria-hidden");
  }

  private hide(el: HTMLElement) {
    el.style.opacity = "0";
    el.style.pointerEvents = "none";
    el.setAttribute("aria-hidden", "true");
  }

  private ribbonMarkup(frame: Frame, w: number, h: number): string {
    const vanishY = frame.vanishY * h;
    const parts = [
      `<defs>
        <linearGradient id="vp-floor" gradientUnits="userSpaceOnUse" x1="0" y1="${h}" x2="0" y2="${vanishY}">
          <stop offset="0%" stop-color="#d7c4a4" stop-opacity="0.78"/>
          <stop offset="62%" stop-color="#e6d3b4" stop-opacity="0.22"/>
          <stop offset="100%" stop-color="#fff4df" stop-opacity="0"/>
        </linearGradient>
        <linearGradient id="vp-floor-branch" gradientUnits="userSpaceOnUse" x1="0" y1="${h}" x2="0" y2="${vanishY}">
          <stop offset="0%" stop-color="#b7cfc6" stop-opacity="0.7"/>
          <stop offset="70%" stop-color="#d5e4de" stop-opacity="0.16"/>
          <stop offset="100%" stop-color="#f3fffb" stop-opacity="0"/>
        </linearGradient>
        <linearGradient id="vp-brass" gradientUnits="userSpaceOnUse" x1="0" y1="${h}" x2="0" y2="${vanishY}">
          <stop offset="0%" stop-color="#f0d7a2" stop-opacity="0.95"/>
          <stop offset="100%" stop-color="#fff6e6" stop-opacity="0"/>
        </linearGradient>
      </defs>`,
    ];

    const ribbons = [...frame.ribbons].sort(
      (a, b) => Number(a.role === "branch") - Number(b.role === "branch"),
    );
    for (const ribbon of ribbons) {
      const poly = roadPolygon(ribbon.samples, w, h, frame.vanishX, ribbon.role === "active");
      if (!poly) continue;
      const fill = ribbon.role === "branch" ? "url(#vp-floor-branch)" : "url(#vp-floor)";
      parts.push(`<polygon points="${poly.points}" fill="${fill}"/>`);
      parts.push(
        `<polyline points="${poly.center}" fill="none" stroke="url(#vp-brass)" stroke-width="1.6" stroke-linecap="round"/>`,
      );
    }

    for (const marker of frame.markers) {
      if (marker.opacity < 0.08) continue;
      const x = marker.x * w;
      const y = marker.y * h;
      const hw = Math.max(4, marker.halfWidth * w * 0.62);
      parts.push(
        `<line x1="${x - hw}" y1="${y}" x2="${x + hw}" y2="${y}" stroke="#f3e2c2" stroke-opacity="${marker.opacity.toFixed(3)}" stroke-width="1.25"/>`,
      );
    }
    return parts.join("");
  }

  private placeHints(frame: Frame, w: number, h: number) {
    const seen = new Set<string>();
    for (const hint of frame.hints) {
      seen.add(hint.id);
      let button = this.hintButtons.get(hint.id);
      if (!button) {
        button = document.createElement("button");
        button.type = "button";
        button.className = "vp-hint";
        button.addEventListener("click", () => this.take(hint.id));
        this.hints.append(button);
        this.hintButtons.set(hint.id, button);
      }
      button.textContent = hint.label;
      button.setAttribute("aria-label", `Take branch ${hint.label}`);
      button.classList.toggle("in-range", hint.inRange);
      button.style.left = `${hint.x * w}px`;
      button.style.top = `${hint.y * h}px`;
      button.style.opacity = hint.opacity.toFixed(3);
      button.style.zIndex = String(hint.zIndex);
      button.style.transform = `translate(-50%, -50%) scale(${Math.max(hint.scale, 0.35).toFixed(4)})`;
      button.style.pointerEvents = hint.opacity > 0.25 ? "auto" : "none";
    }
    for (const [id, button] of this.hintButtons) {
      if (!seen.has(id)) {
        button.remove();
        this.hintButtons.delete(id);
      }
    }
  }

  private renderStops(state: NavState) {
    const key = `${state.pathId}:${state.nodes.map((node) => node.id).join(",")}`;
    if (key !== this.stopsKey) {
      this.stopsKey = key;
      this.stops.replaceChildren();
      for (const node of state.nodes) {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.nodeId = node.id;
        const index = document.createElement("span");
        index.className = "vp-stop-index";
        index.textContent = String(node.index + 1).padStart(2, "0");
        const label = document.createElement("span");
        label.className = "vp-stop-label";
        label.textContent = node.label;
        button.append(index, label);
        button.addEventListener("click", () => this.activate(node.id));
        this.stops.append(button);
      }
    }
    for (const button of this.stops.querySelectorAll("button")) {
      const current = button.dataset.nodeId === state.currentNode;
      button.classList.toggle("is-current", current);
      if (current) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    }
  }

  private announce(state: NavState) {
    const branch = state.availableBranches.find((item) => item.inRange);
    let message = state.currentLabel;
    if (branch) message += `. Branch available: ${branch.label}. Press Enter to take it.`;
    if (state.canReturn) {
      message += state.returnLabel
        ? `. Press Escape to return to ${state.returnLabel}.`
        : ". Press Escape to return.";
    }
    if (this.live.textContent !== message) this.live.textContent = message;
  }

  private publish(state: NavState) {
    if (!this.onChange) return;
    const signature = [
      state.pathId,
      state.currentNode,
      state.canReturn ? "1" : "0",
      state.availableBranches.map((branch) => `${branch.id}:${branch.inRange ? 1 : 0}`).join(","),
      state.turning?.direction ?? "",
      state.pending?.kind ?? "",
    ].join("|");
    if (signature === this.signature) return;
    this.signature = signature;
    this.onChange(state);
  }

  private applyReduced() {
    const on = this.reducedOverride ?? this.mq.matches;
    this.nav.setReducedMotion(on);
    this.root.classList.toggle("is-reduced", on);
  }

  private onMotionPreference = () => {
    if (this.reducedOverride === null) this.applyReduced();
  };

  private onWheel = (event: WheelEvent) => {
    if (event.ctrlKey) return;
    event.preventDefault();
    let dy = event.deltaY;
    if (event.deltaMode === 1) dy *= 16;
    else if (event.deltaMode === 2) dy *= window.innerHeight;
    const delta = clamp(dy / 130, -1.5, 1.5);
    if (delta !== 0) this.nav.scroll(performance.now(), delta);
  };

  private onKey = (event: KeyboardEvent) => {
    const target = event.target instanceof HTMLElement ? event.target : null;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
      return;
    }
    const inControl = Boolean(target?.closest("button, a"));
    switch (event.key) {
      case "ArrowDown":
      case "ArrowRight":
      case "PageDown":
        event.preventDefault();
        this.nav.step(1);
        break;
      case "ArrowUp":
      case "ArrowLeft":
      case "PageUp":
        event.preventDefault();
        this.nav.step(-1);
        break;
      case " ":
        if (inControl) return;
        event.preventDefault();
        this.nav.step(1);
        break;
      case "Home":
        event.preventDefault();
        this.focusEdge(true);
        break;
      case "End":
        event.preventDefault();
        this.focusEdge(false);
        break;
      case "Enter":
        if (inControl) return;
        event.preventDefault();
        this.takeReadyBranch();
        break;
      case "Escape":
      case "Backspace":
        event.preventDefault();
        this.tryReturn();
        break;
      default:
        break;
    }
  };

  private onTouchStart = (event: TouchEvent) => {
    if (event.touches.length !== 1) return;
    const target = event.target instanceof Element ? event.target : null;
    const state = JSON.parse(this.nav.stateJson()) as NavState;
    this.touch = {
      y: event.touches[0].clientY,
      ignore: Boolean(target?.closest("button, a, .vp-stops")),
      origin: state.target,
      accum: 0,
      t0: performance.now(),
    };
  };

  private onTouchMove = (event: TouchEvent) => {
    if (!this.touch || this.touch.ignore || event.touches.length !== 1) return;
    event.preventDefault();
    const y = event.touches[0].clientY;
    const dy = this.touch.y - y;
    this.touch.y = y;
    const delta = clamp(dy / 170, -1.2, 1.2);
    if (Math.abs(delta) > 0.001) {
      this.touch.accum += delta;
      this.nav.moveBy(delta);
    }
  };

  private onTouchEnd = () => {
    const drag = this.touch;
    this.touch = null;
    if (!drag || drag.ignore) return;
    const distance = drag.accum;
    if (Math.abs(distance) < 0.04) {
      this.nav.park(drag.origin);
      return;
    }
    const flick = performance.now() - drag.t0 < 260 && Math.abs(distance) < 1.15;
    this.nav.park(flick ? drag.origin + Math.sign(distance) : Math.round(drag.origin + distance));
  };

  private focusEdge(start: boolean) {
    const state = JSON.parse(this.nav.stateJson()) as NavState;
    const node = start ? state.nodes[0] : state.nodes[state.nodes.length - 1];
    if (node) this.tryCall(() => this.nav.focus(node.id));
  }

  private takeReadyBranch() {
    const state = JSON.parse(this.nav.stateJson()) as NavState;
    const branch = state.availableBranches.find((item) => item.inRange) ?? state.availableBranches[0];
    if (branch) this.take(branch.id);
  }

  private tryReturn() {
    const state = JSON.parse(this.nav.stateJson()) as NavState;
    if (state.canReturn) this.tryCall(() => this.nav.returnToParent());
  }

  private activate(id: string) {
    this.tryCall(() => this.nav.activate(id));
  }

  private take(id: string) {
    this.tryCall(() => this.nav.takeBranch(id));
  }

  private tryCall(fn: () => void) {
    try {
      fn();
    } catch {
      // The core rejects commands that do not apply (busy turn, no parent).
    }
  }
}

function viewKey(nodeId: string, viewId: string) {
  return `${nodeId}:${viewId}`;
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function roadPolygon(
  samples: Frame["ribbons"][number]["samples"],
  w: number,
  h: number,
  vanishX: number,
  extendToFloor: boolean,
): { points: string; center: string } | null {
  if (samples.length < 2) return null;
  const nearToFar = [...samples].sort((a, b) => a.depth - b.depth);
  const near = nearToFar[0];
  const left = nearToFar.map((sample) => `${sample.leftX * w},${sample.leftY * h}`);
  const right = [...nearToFar].reverse().map((sample) => `${sample.rightX * w},${sample.rightY * h}`);
  const center = nearToFar.map((sample) => `${sample.centerX * w},${sample.centerY * h}`);
  const extend = extendToFloor && near.centerY > 0.68 && Math.abs(near.centerX - vanishX) < 0.28;
  if (extend) {
    const hw = ((near.rightX - near.leftX) * w) / 2;
    const cx = near.centerX * w;
    const extra = hw * 0.7;
    left.unshift(`${cx - hw - extra},${h + 4}`);
    right.push(`${cx + hw + extra},${h + 4}`);
    center.unshift(`${cx},${h + 2}`);
  }
  return { points: [...left, ...right].join(" "), center: center.join(" ") };
}
