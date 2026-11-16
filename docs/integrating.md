# Integrating

vp-navigation is a camera and a graph. A host owns the content. The core replies with a frame of viewport-fraction transforms, and the host draws whatever it wants at those rectangles: HTML, a canvas, a scene.

## The graph

A path is an ordered list of stops. A stop has an id, a label, an orientation, and any number of views. A view has an id and an emphasis. Emphasis is a share of that stop's front. `50` and `50` split it in half. A stop with no views listed is one view at full emphasis.

`orientation` is `horizontal` (columns, opening left and right) or `vertical` (rows, opening up and down). Three or more views travel outward along that axis as you walk past them. How far is the `explode` setting.

A branch joins a child path to one stop on a parent, on the `left` or the `right`. The child path's views stay out of the frame until the traveler turns onto it. What the host shows before the turn is the branch hint: the link's label, side, and screen position.

Stops do not carry HTML, meshes, or copy. The same graph can drive a DOM page, a React tree, or a game level.

```json
{
  "rootPath": "main",
  "paths": [
    {
      "id": "main",
      "nodes": [
        { "id": "intro", "label": "Intro" },
        {
          "id": "fork",
          "label": "Fork",
          "orientation": "horizontal",
          "views": [
            { "id": "left", "emphasis": 60 },
            { "id": "right", "emphasis": 40 }
          ]
        }
      ]
    },
    {
      "id": "side",
      "nodes": [{ "id": "room", "label": "Room" }]
    }
  ],
  "branches": [
    {
      "id": "to-side",
      "label": "Side room",
      "fromPath": "main",
      "fromNode": "fork",
      "toPath": "side",
      "side": "right"
    }
  ]
}
```

## Driving it

Call `scroll(nowMs, delta)` from the wheel. The delta is in stops; the DOM adapter uses `deltaY / 130`, clamped to `±1.5`, and passes `performance.now()` as the clock. A short burst advances one stop. A stream that keeps its speed — a drag, a held wheel — travels smoothly through the stops and lands on the nearest one when it goes quiet. Call `moveBy(delta)` to follow a finger while it is down, then `park(stop)` when it lifts. Call `step(±1)` from the keyboard. Call `activate(nodeId)` when a view is clicked. Call `takeBranch(id)` for a branch link, and `returnToParent()` to leave. Call `tick(dtMs)` once per frame, then read `frameJson()`.

`stateJson()` is the index: which path is active, the current stop, and which branches are in range. It does not describe layout.

## The frame

Coordinates are fractions of the viewport, origin at the top-left. For each view:

| Field | Meaning |
| --- | --- |
| `x`, `y` | Center of the view. |
| `width`, `height` | Size. Already multiplied by `scale`. |
| `opacity` | `0` is hidden, `1` is solid. |
| `rotateX`, `rotateY` | Degrees of tilt. `rotateY` includes the stop's yaw. |
| `zIndex` | Paint order. Nearer stops are higher. |
| `emphasis` | The share the author gave this view, before normalization. |

Each stop also has `x`, `y`, `yaw`, `hingeX`, and `hingeY`. Yaw the stop once around that hinge, and draw each view at `rotateY - yaw`, so the columns stay on one plane. The DOM adapter does this. At a junction the viewer stands in the middle. The path ahead is hinged on the far edge and opens outward: a branch on the right sends that face off to the left, free edge away from the viewer. The chosen face is hinged on the near edge, so it enters from that side and opens onto the camera. Both stay solid. The angle is `branchAngleDeg` (60 is one step of a hexagon).

A view whose node is not in the frame is not on the path the traveler is in. Hide it. The DOM adapter sets `aria-hidden` in that case, and also when `opacity` is below `0.05`.

The frame also carries the floor (`ribbons`), the tick marks (`markers`), and the branch links (`hints`). A hint's `label` is the host's string. Its `inRange` flag is how the UI decides the link can be taken.

## The DOM adapter

`DomViewport` is a reference renderer, not a requirement.

```ts
import init, { VpNavigator } from "@vp-navigation/dom/pkg/vp_navigation_wasm.js";
import { DomViewport } from "@vp-navigation/dom";

await init();
const nav = new VpNavigator(JSON.stringify(config));
const viewport = new DomViewport({
  root: document.querySelector("#stage"),
  navigator: nav,
  content: {
    intro: { views: [{ id: "intro", element: introEl }] },
    fork: {
      views: [
        { id: "left", element: leftEl },
        { id: "right", element: rightEl },
      ],
    },
  },
});
viewport.start();
```

Content is keyed by stop id. Each view's `id` matches the view id in the graph. The element can be any HTML. The adapter positions it. It does not read the element's text.

To use another renderer, keep the wasm object and map `frame.nodes[].views` onto your own nodes. The sketch in the [README](../README.md) is the whole loop: `tick`, parse `frameJson`, write positions.

## What stays with the host

- The words, images, and controls inside a view.
- How a branch is labeled. The core only places the hint.
- Whether to follow `prefers-reduced-motion`. Call `setReducedMotion`.
- The numbers in [configuration](configuration.md). Defaults are a starting point. `setProjection` changes them without rebuilding the graph.
