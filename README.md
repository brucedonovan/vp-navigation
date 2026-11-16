# vp-navigation

An experimental navigation framework. Instead of scrolling down a page, you walk a **path** that recedes to a vanishing point. Each stop keeps its page beside the path — a title on the left, a note on the right — and the stops ahead shrink toward a single point of light. A path can fork. You turn onto the branch, and you can walk back off it.

The core is framework-agnostic. It is a Rust library, compiled to WebAssembly for the browser. Rendering is someone else's job: the DOM adapter in this repo is one consumer, and a small enough one that React, Svelte, a `<canvas>`, or a game engine can do the same work.

## Run the demo

You need Rust (1.83+), the `wasm32-unknown-unknown` target, [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/), and Node 20+.

```bash
rustup target add wasm32-unknown-unknown

# from the repo root
cargo test -p vp-navigation

wasm-pack build crates/vp-navigation-wasm --target web --out-dir ../../packages/dom/pkg

cd demo
npm install
npm run dev
```

Open `http://localhost:5173`. The demo is **The Long Room**: six stops and a side branch called The Annex.

| Input | What it does |
| --- | --- |
| Wheel / trackpad | Walk along the path. The page itself does not scroll. |
| Arrow keys, Page Up / Down, Space | Step one stop. |
| Swipe | Same as the wheel. |
| Click a page, or a stop in the index | Walk there. Clicking a page on a branch turns onto it. |
| Enter, or **Enter the Annex** | Take the branch, once you are near the junction. |
| Escape, Backspace, or walking back off the first branch stop | Return to the junction you left. |
| Home / End | First or last stop of the path you are on. |

`prefers-reduced-motion: reduce` snaps travel and turns instead of easing them. The **Reduce motion** button in the demo does the same thing.

## Layout

```
crates/vp-navigation/        pure Rust core (graph, motion, projection)
crates/vp-navigation-wasm/   wasm-bindgen wrapper; JSON in, JSON out
packages/dom/                vanilla TS DOM adapter
demo/                        The Long Room
```

`packages/dom/pkg` is build output from wasm-pack. It is not committed.

## Architecture

```
 graph + input                         Frame (x, y, scale, opacity, z)
┌─────────────────────┐               ┌──────────────────────────┐
│  vp-navigation      │  tick / move  │  your renderer           │
│  Rust core          │ ────────────► │  DOM, React, canvas, …   │
│  (or the wasm pkg)  │   project()   │                          │
└─────────────────────┘               └──────────────────────────┘
```

The core owns:

- **The graph.** Paths are ordered stops. A branch joins a child path to one stop on a parent. Stops have an id and a label. They do not have HTML, meshes, or textures.
- **Navigation state.** `position` is the eased camera, in stops. `target` is where input asked to go. Integer positions stand on a stop. `take_branch` walks you to the junction, then turns; walking back off a branch returns you to that junction.
- **Easing.** `tick(dt)` damps `position` toward `target`, and damps a turn from the parent frame onto the branch. Reduced motion commits both immediately.
- **Projection.** For every visible stop it emits viewport-fraction transforms: position, scale, opacity, z-order, and a small yaw. It also emits the path ribbon (the floor running to the vanishing point) and branch hints. One-point perspective is `scale = focal / (focal + depth)`. A panel at the camera has scale 1; panels ahead slide toward the vanishing point and shrink.

The DOM adapter (`DomViewport`) is deliberately thin. Each frame it reads `frameJson()`, writes CSS transforms, and forwards wheel, keyboard, touch, and clicks back into the core. Screen-reader text is the real DOM, in path order, not a canvas. A live region announces the current stop.

### Using the core from Rust

```rust
use vp_navigation::{BranchSpec, Graph, Navigator, Side};

let graph = Graph::builder()
    .root("main")
    .path("main", &[("intro", "Intro"), ("fork", "Fork")])
    .path("side", &[("room", "Room")])
    .branch(BranchSpec {
        id: "to-side".into(),
        label: "Side room".into(),
        from_path: "main".into(),
        from_node: "fork".into(),
        to_path: "side".into(),
        side: Side::Right,
    })
    .build()?;

let mut nav = Navigator::new(graph);
nav.move_by(0.5);
nav.tick(16.0);
let frame = nav.project();
```

A native renderer or a game engine stops here. It maps `frame.nodes` onto whatever it draws with. No wasm required.

### Using the core from JavaScript

`wasm-pack` emits `VpNavigator` plus TypeScript types. Config and frames are JSON, so the binding stays small and any language that can call the exports can parse the result.

```ts
import init, { VpNavigator } from "@vp-navigation/dom/pkg/vp_navigation_wasm.js";

await init();
const nav = new VpNavigator(JSON.stringify({
  rootPath: "main",
  paths: [
    { id: "main", nodes: [
      { id: "intro", label: "Intro" },
      { id: "fork", label: "Fork" },
    ]},
    { id: "side", nodes: [{ id: "room", label: "Room" }] },
  ],
  branches: [{
    id: "to-side",
    label: "Side room",
    fromPath: "main",
    fromNode: "fork",
    toPath: "side",
    side: "right",
  }],
}));

function frame() {
  nav.tick(16);
  return JSON.parse(nav.frameJson());
}
```

`projection` in that JSON is optional. It overrides focal length, vanishing point, lane width, branch angle, and easing. See `Params` in `crates/vp-navigation/src/params.rs`.

### Wiring another UI framework

React (or Svelte, Vue, a canvas) keeps the wasm object and maps the frame onto its own elements. The DOM adapter is a reference, not a requirement. Sketch of a React hook:

```tsx
function usePath(nav: VpNavigator) {
  const [frame, setFrame] = useState(() => JSON.parse(nav.frameJson()));
  useEffect(() => {
    let last = performance.now();
    let id = 0;
    const loop = (now: number) => {
      nav.tick(now - last);
      last = now;
      setFrame(JSON.parse(nav.frameJson()));
      id = requestAnimationFrame(loop);
    };
    id = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(id);
  }, [nav]);
  return frame;
}

// <article style={{
//   transform: `translate(${panel.x * width}px, ${panel.y * height}px) scale(${panel.scale})`,
//   opacity: panel.opacity,
//   zIndex: panel.zIndex,
// }} />
```

Wheel handling stays in the host: call `moveBy(deltaY / 130)` and `preventDefault()` so the document does not scroll. Call `step(±1)` from the keyboard, `activate(nodeId)` when a page is clicked, `takeBranch(id)` at a junction, and `returnToParent()` to leave. Call `setReducedMotion(true)` when `prefers-reduced-motion: reduce` matches.

Content is not the core's problem. Key your components by node id and let the frame tell you where they sit.

## Tests

```bash
cargo test -p vp-navigation
```

The tests cover stepping and clamping, easing, focus, taking a branch and returning (including by walking backward), reduced motion, and the projection: the stop at the camera is full size, later stops shrink and converge on the vanishing point, the floor narrows with distance, and a right-hand branch sits to the right of the main path.

## Accessibility

- Stops are real text in DOM order, so a screen reader can read the room without walking it.
- Arrow keys, Home, End, Enter, and Escape operate the path. The stop index is a list of buttons.
- A polite live region announces the current stop and an available branch.
- `prefers-reduced-motion` skips the easing. Turns still change which path you are on; they just do not animate.
