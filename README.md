# vp-navigation

An experimental navigation framework. Instead of scrolling down a page, you walk a **path** that recedes to a vanishing point. Each stop is a tick on that path, and it can hold as many views as you give it. One view takes the whole front. Any other arrangement is a grid: a row, a column, or whatever columns, rows, and spans you give it. Emphasis is each cell's share of that front. Walk through the grid and its views explode outward from the center and fade. The stops ahead shrink toward a single point of light. A path can fork. You turn onto the branch, and you can walk back off it.

The core is framework-agnostic. It is a Rust library, compiled to WebAssembly for the browser. Rendering is someone else's job: the DOM adapter in this repo is one consumer, and a small enough one that React, Svelte, a `<canvas>`, or a game engine can do the same work. Motion, view size, and the turn are settings. See [docs/configuration.md](docs/configuration.md).

Licensed under [MIT](LICENSE).

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

Open `http://localhost:5173`. The demo is **The Long Room**: seven stops and a side path called The Alley. The Alley's views stay off the main path. The link is what you see, and taking it turns you in.

| Input | What it does |
| --- | --- |
| Wheel / trackpad | A flick advances one stop, like a key press. Keep scrolling, or hold the wheel, and you travel smoothly through the stops. The page itself does not scroll. |
| Arrow keys, Page Up / Down, Space | Step one stop. |
| Swipe | A short flick advances one stop. A drag follows your finger through the stops and lands on the nearest one. |
| Click a page, or a stop in the index | Walk there. Clicking a page on a branch turns onto it. |
| Enter, or **Enter the Alley** | Take the branch, once you are near the junction. |
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

- **The graph.** Paths are ordered stops. A branch joins a child path to one stop on a parent. A stop has an id, a label, and any number of views, arranged as a grid. With no grid of its own the stop is one row or one column (`horizontal` or `vertical`). `grid.columns` and `grid.rows` make any other arrangement, and a view can pin itself to a cell and span. Each view has an id and an emphasis, a percentage share of that stop's front. Shares are normalized, so 50 and 50 split a row in half. A stop with no views listed is one view at full emphasis. Stops do not have HTML, meshes, or textures.
- **Navigation state.** `position` is the eased camera, in stops. `target` is where input asked to go. Integer positions stand on a stop. `take_branch` walks you to the junction, then turns; walking back off a branch returns you to that junction.
- **Easing.** `tick(dt)` damps `position` toward `target`, and damps a turn from the parent frame onto the branch. `scroll(now, delta)` treats a short impulse as one stop and a sustained stream as smooth travel through later stops. `dwellMs` (default 280) holds briefly on each stop a sustained travel passes. After the wheel goes quiet, `spring` settles a nearby target onto its stop. Set `spring` or `dwellMs` to `0` to turn that behavior off. Reduced motion commits travel and turns immediately. At a junction the viewer stands in the middle. Taking a branch on the right opens the path ahead outward to the left, and the chosen path enters from the right. Both faces stay solid. The swing is `branchAngleDeg` (60 is one step of a hexagon).
- **Projection.** For every visible view it emits viewport-fraction transforms: position, width, height, scale, opacity, z-order, and a small tilt. It also emits the path ribbon (the floor running to the vanishing point) and branch hints. One-point perspective is `scale = focal / (focal + depth)`. The stop you are standing on opens across most of the frame, divided by emphasis into whatever grid the stop declared. Walking through that grid throws every view outward from the center, in proportion to where it sits, and fades it. A single view is the center, so it fades in place. Stops ahead shrink toward the vanishing point. `width` and `height` already include `scale`. A branch's views are not in the frame while you are on the parent — only its hint, the link the host labeled. Turning swings that path onto the camera and brings those views in. What a view contains is entirely the host's.

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

`projection` in that JSON is optional. Any field you omit keeps its default, and the same object can be applied later with `setProjection`. The full list, with defaults and ranges, is in [docs/configuration.md](docs/configuration.md). How a host supplies its own views is in [docs/integrating.md](docs/integrating.md).

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
//   left: view.x * width,
//   top: view.y * height,
//   width: view.width * width,
//   height: view.height * height,
//   opacity: view.opacity,
//   zIndex: view.zIndex,
// }} />
```

Wheel handling stays in the host: call `scroll(performance.now(), deltaY / 130)` and `preventDefault()` so the document does not scroll. A flick then advances one stop; keep sending samples while the wheel is held and the traveler moves through the stops. Call `step(±1)` from the keyboard, `activate(nodeId)` when a page is clicked, `takeBranch(id)` at a junction, and `returnToParent()` to leave. Call `setReducedMotion(true)` when `prefers-reduced-motion: reduce` matches.

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
