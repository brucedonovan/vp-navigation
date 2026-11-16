# Configuration

The core does not know what a view contains. It knows the graph, where the camera is, and how to project that into a frame. Everything about the feel of that projection is a setting.

Settings live on one object, `projection`. Pass it inside the config JSON, or change it later. Fields you leave out keep their current value. Names are camelCase.

```json
{
  "rootPath": "main",
  "paths": [],
  "projection": {
    "spring": 0.3,
    "dwellMs": 280,
    "frontWidth": 0.88,
    "turnExit": 2.6
  }
}
```

From JavaScript, after the navigator exists:

```js
nav.setProjection(JSON.stringify({ spring: 0.15, frontWidth: 0.74 }));
const current = JSON.parse(nav.projectionJson());
```

From Rust, build the navigator with `Navigator::with_params`, or call `set_projection_json`.

`prefers-reduced-motion` and `setReducedMotion(true)` skip easing, the hold, and the settle. Travel and turns commit on the next tick.

## Motion

| Field | Default | Range | What it changes |
| --- | --- | --- | --- |
| `stiffness` | `9` | `> 0` | How quickly the camera catches the scroll target, in 1/seconds. Higher is snappier. |
| `turnStiffness` | `5.5` | `> 0` | How quickly a turn onto a branch completes. |
| `dwellMs` | `280` | `0` – `4000` | How long to hold on a stop a scroll would pass through, so that stop can be read. `0` glides through. |
| `spring` | `0.3` | `0` – `1` | After the wheel has been quiet, how strongly a nearby target settles onto a stop. `0` leaves the target where the gesture ended. This does not limit how far one gesture can travel. |
| `springRange` | `0.2` | `0` – `0.5` | A finished scroll only settles when it is at most this many stops from an integer stop. A target at `2.5` with the default range stays at `2.5`. A target at `2.12` settles to `2`. |
| `springIdleMs` | `90` | `0` – `2000` | How long the wheel must be quiet before the settle starts. |
| `reducedMotion` | `false` | boolean | Commit travel and turns immediately. |

The wheel adds to `target` directly. A long flick still crosses several stops. `dwellMs` is the brief pause on each one. `spring` only tidies a gesture that ended close to a stop.

## The front

Sizes are fractions of the viewport. The stop you are standing on is laid out from these, then perspective scales the stops ahead. `width` and `height` in the frame already include that scale.

| Field | Default | Range | What it changes |
| --- | --- | --- | --- |
| `foregroundY` | `0.48` | `0.2` – `0.95` | Vertical center of the stop you are on. |
| `frontWidth` | `0.88` | `0.3` – `1.05` | Width of that stop. A single view uses all of it. Columns share it by emphasis. |
| `pageHeight` | `0.58` | `0.15` – `0.9` | Height of a single view. |
| `columnHeight` | `0.54` | `0.12` – `0.9` | Height of each view in a grid that has only one row. |
| `frontHeight` | `0.66` | `0.2` – `0.95` | Height shared by the rows of a grid that has more than one. |
| `viewGap` | `0.016` | `0` – `0.08` | Gap between cells of the same stop. |
| `explode` | `0.46` | `0` – `1.2` | How far a view at the edge of its grid travels, in viewport fractions, as you walk past the stop. Every view moves outward from the center in proportion to where it sits, and fades. A single view is the center, so it only fades. |

Emphasis is not a projection setting. It belongs to each view in the graph. Shares are normalized. A spanning cell divides its emphasis across the tracks it covers, and each track takes the largest share that lands on it. `50` and `50` still split a row in half. How the cells are arranged — one row, one column, or any other grid — is part of the stop, described in [integrating](integrating.md).

## Turns

| Field | Default | Range | What it changes |
| --- | --- | --- | --- |
| `branchAngleDeg` | `60` | `8` – `75` | How far, in degrees, the faces swing at a junction. 60 is one step of a hexagon. The side (left or right) comes from the branch. The face ahead opens outward to the far side; the chosen face enters from the near side. |
| `turnExit` | `2.6` | `0` – `6` | Kept for hosts that already set it. The swing angle is `branchAngleDeg`. |
| `turnStartDistance` | `0.22` | `> 0` | How close, in stops, the camera must be to a junction before the turn begins. |
| `uiRange` | `0.85` | `> 0` | How close a junction must be before the branch link is reported in range. |

A branch's views are not in the frame while you are on the parent. Only the link (the branch hint) is. The turn brings those views in.

## Camera

Screen positions are fractions of the viewport. `(0, 0)` is the top-left, `(1, 1)` is the bottom-right. One-point perspective is `scale = focal / (focal + depth)`.

| Field | Default | Range | What it changes |
| --- | --- | --- | --- |
| `focal` | `1.25` | `> 0.2` | Focal length, in stops. Larger values flatten the recession. |
| `lane` | `0.29` | `0.02` – `0.8` | How far a world unit of lateral offset lands from the vanishing point, at the camera plane. |
| `vanishX` | `0.5` | `0` – `1` | Horizontal vanishing point. |
| `vanishY` | `0.38` | `0` – `1` | Vertical vanishing point. |
| `panelAnchorY` | `0.64` | `0` – `1.5` | Vertical anchor of a view when it is still in perspective, before it opens into the front. |
| `roadAnchorY` | `0.8` | `0` – `1.5` | Vertical anchor of the path centerline at the camera plane. |
| `roadHalfWidth` | `0.145` | `> 0` | Half-width of the floor, as a viewport fraction, at the camera plane. |
| `maxDepth` | `10` | `> 0.5` | Stops further ahead than this are fully faded. |
| `behindLimit` | `-0.8` | `< 0` | Stops further behind than this are fully faded. |

A value outside its range is rejected. The navigator keeps the previous settings.
