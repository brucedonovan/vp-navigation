import init, { VpNavigator } from "../../packages/dom/pkg/vp_navigation_wasm.js";
import wasmUrl from "../../packages/dom/pkg/vp_navigation_wasm_bg.wasm?url";
import { DomViewport } from "../../packages/dom/src/index";
import type { NavState } from "../../packages/dom/src/index";
import { galleryConfig, galleryContent } from "./gallery";

function required<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`demo markup is missing ${selector}`);
  return element;
}

const stage = required<HTMLElement>("#stage");
const station = required<HTMLElement>("#station");
const branchBtn = required<HTMLButtonElement>("#branch");
const backBtn = required<HTMLButtonElement>("#back");
const motionBtn = required<HTMLButtonElement>("#motion");

await init({ module_or_path: wasmUrl });
const nav = new VpNavigator(JSON.stringify(galleryConfig()));
const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

const viewport = new DomViewport({
  root: stage,
  navigator: nav,
  content: galleryContent(),
  onChange: (state) => syncChrome(state),
});

let reducedOverride: boolean | null = motionQuery.matches ? true : null;

function motionEnabled() {
  return reducedOverride ?? motionQuery.matches;
}

function syncMotionButton() {
  const reduced = motionEnabled();
  motionBtn.setAttribute("aria-pressed", String(reduced));
  motionBtn.textContent = reduced ? "Play motion" : "Reduce motion";
}

function syncChrome(state: NavState) {
  const index = state.nodes.findIndex((node) => node.id === state.currentNode);
  const number = String(index + 1).padStart(2, "0");
  station.textContent = `${number}   ${state.currentLabel}`;

  const branch = state.availableBranches.find((item) => item.inRange);
  branchBtn.hidden = !branch;
  if (branch) {
    branchBtn.textContent = `Enter ${branch.label}`;
    branchBtn.dataset.branchId = branch.id;
  }

  backBtn.hidden = !state.canReturn;
  backBtn.textContent = state.returnLabel ? `Return to ${state.returnLabel}` : "Return";
}

branchBtn.addEventListener("click", () => {
  const id = branchBtn.dataset.branchId;
  if (!id) return;
  try {
    nav.takeBranch(id);
  } catch {
    /* not attached to this path */
  }
});

backBtn.addEventListener("click", () => {
  try {
    nav.returnToParent();
  } catch {
    /* already on the root path */
  }
});

motionBtn.addEventListener("click", () => {
  reducedOverride = !motionEnabled();
  viewport.setReducedMotion(reducedOverride);
  syncMotionButton();
});

motionQuery.addEventListener("change", () => {
  if (reducedOverride === null) syncMotionButton();
});

syncMotionButton();
if (reducedOverride) viewport.setReducedMotion(true);
viewport.start();
syncChrome(JSON.parse(nav.stateJson()) as NavState);
