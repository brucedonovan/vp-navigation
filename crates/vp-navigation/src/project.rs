use serde::Serialize;

use crate::graph::{place_views, Node, Orientation, Placement, Side};
use crate::navigate::{Navigator, TurnKind};
use crate::params::Params;

/// One projected frame. Coordinates are viewport fractions, origin at the top-left.
///
/// The renderer multiplies `x` and `y` by the stage size and applies `scale`,
/// `opacity`, and `rotate_y`. The core does not know about CSS, canvases, or meshes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub path_id: String,
    pub position: f64,
    pub target: f64,
    pub vanish_x: f64,
    pub vanish_y: f64,
    pub reduced_motion: bool,
    pub nodes: Vec<ProjectedNode>,
    pub markers: Vec<ProjectedMarker>,
    pub ribbons: Vec<Ribbon>,
    pub hints: Vec<BranchHint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedNode {
    pub id: String,
    pub label: String,
    pub path_id: String,
    pub index: usize,
    /// Depth of the stop's center along the camera axis, in stops.
    /// Negative values are behind the camera.
    pub depth: f64,
    pub orientation: Orientation,
    pub active: bool,
    pub interactive: bool,
    /// Screen center of the stop. Views are laid out around this point.
    pub x: f64,
    pub y: f64,
    /// Degrees. Yaw the whole stop by this, around [`hinge_x`](ProjectedNode::hinge_x)
    /// / [`hinge_y`](ProjectedNode::hinge_y). Use `rotateY - yaw` on each view
    /// so the columns stay on that one plane.
    pub yaw: f64,
    /// Pivot of [`yaw`](ProjectedNode::yaw), in viewport fractions. The pivot
    /// is the stop center, except during a turn: the viewer stands in the
    /// middle, the face ahead is hinged on the far edge and opens outward, and
    /// the face being entered is hinged on the near edge.
    pub hinge_x: f64,
    pub hinge_y: f64,
    pub views: Vec<ProjectedView>,
}

/// One view of a stop, already laid out. `width` and `height` are viewport
/// fractions and already include `scale`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedView {
    pub id: String,
    /// The emphasis the author gave this view, before shares were normalized.
    pub emphasis: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub opacity: f64,
    /// Degrees. Positive rotateX tips the top edge away from the viewer.
    pub rotate_x: f64,
    /// Degrees. Positive rotateY turns the right edge away from the viewer.
    pub rotate_y: f64,
    pub z_index: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedMarker {
    pub node_id: String,
    pub path_id: String,
    pub x: f64,
    pub y: f64,
    pub half_width: f64,
    pub opacity: f64,
    pub scale: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RibbonRole {
    Active,
    Branch,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ribbon {
    pub path_id: String,
    pub role: RibbonRole,
    pub samples: Vec<RibbonSample>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RibbonSample {
    pub center_x: f64,
    pub center_y: f64,
    pub left_x: f64,
    pub left_y: f64,
    pub right_x: f64,
    pub right_y: f64,
    pub opacity: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchHint {
    pub id: String,
    pub label: String,
    pub side: Side,
    pub from_node: String,
    pub to_path: String,
    pub distance: f64,
    pub in_range: bool,
    pub x: f64,
    pub y: f64,
    pub opacity: f64,
    pub scale: f64,
    pub z_index: i32,
}

struct Screen {
    x: f64,
    y: f64,
    scale: f64,
}

struct Sink<'a> {
    nodes: &'a mut Vec<ProjectedNode>,
    markers: &'a mut Vec<ProjectedMarker>,
    ribbons: &'a mut Vec<Ribbon>,
}

pub(crate) fn build(nav: &Navigator) -> Frame {
    let mut nodes = Vec::new();
    let mut markers = Vec::new();
    let mut ribbons = Vec::new();
    let mut hints = Vec::new();
    let params = &nav.params;

    if let Some(turn) = &nav.turn {
        let parent_camera = match turn.kind {
            TurnKind::Enter => nav.position,
            TurnKind::Exit => turn.junction_index as f64,
        };
        // The junction is a room of faces around the viewer. Turning rotates
        // that room until the chosen face is the one in front.
        let parent_fade = 1.0;
        {
            let mut sink = Sink {
                nodes: &mut nodes,
                markers: &mut markers,
                ribbons: &mut ribbons,
            };
            push_straight(
                nav,
                &turn.parent_path,
                parent_camera,
                turn.align,
                turn.signed_angle,
                parent_fade,
                true,
                RibbonRole::Active,
                &mut sink,
            );
        }
        let junction_depth = match turn.kind {
            TurnKind::Enter => turn.junction_index as f64 - nav.position,
            TurnKind::Exit => 0.0,
        };
        let mut sink = Sink {
            nodes: &mut nodes,
            markers: &mut markers,
            ribbons: &mut ribbons,
        };
        push_angled(
            nav,
            &turn.child_path,
            Angled {
                align: turn.align,
                signed_angle: turn.signed_angle,
                junction_depth,
                fade: 1.0,
                // The chosen face starts one step around the room and rotates
                // forward with it. Opacity is not how the turn is shown.
                include_views: true,
                reveal: 1.0,
                role: RibbonRole::Branch,
            },
            &mut sink,
        );
    } else {
        {
            let mut sink = Sink {
                nodes: &mut nodes,
                markers: &mut markers,
                ribbons: &mut ribbons,
            };
            push_straight(
                nav,
                &nav.path_id,
                nav.position,
                0.0,
                0.0,
                1.0,
                false,
                RibbonRole::Active,
                &mut sink,
            );
        }
        let branches: Vec<_> = nav.graph.branches_from(&nav.path_id).cloned().collect();
        for branch in branches {
            let junction_depth = branch.from_index as f64 - nav.position;
            {
                let mut sink = Sink {
                    nodes: &mut nodes,
                    markers: &mut markers,
                    ribbons: &mut ribbons,
                };
                push_angled(
                    nav,
                    &branch.to_path,
                    Angled {
                        align: 0.0,
                        signed_angle: branch.signed_angle(params.branch_angle_rad()),
                        junction_depth,
                        fade: 0.45,
                        include_views: false,
                        reveal: 0.0,
                        role: RibbonRole::Branch,
                    },
                    &mut sink,
                );
            }
            if let Some(hint) = hint_for(nav, &branch, junction_depth) {
                hints.push(hint);
            }
        }
    }

    mark_active(&mut nodes);

    Frame {
        path_id: nav.path_id.clone(),
        position: nav.position,
        target: nav.target,
        vanish_x: params.vanish_x,
        vanish_y: params.vanish_y,
        reduced_motion: params.reduced_motion,
        nodes,
        markers,
        ribbons,
        hints,
    }
}

fn push_straight(
    nav: &Navigator,
    path_id: &str,
    camera: f64,
    align: f64,
    signed_angle: f64,
    fade: f64,
    // The camera is yawing away from this path. The stop it is leaving stays
    // solid and turns as one plane; it is not the walk-past dissolve.
    turning: bool,
    role: RibbonRole,
    sink: &mut Sink<'_>,
) {
    let Some(path) = nav.graph.path(path_id) else {
        return;
    };
    let params = &nav.params;
    // Laid out facing the camera. A turn then carries the whole face around
    // the room, instead of dissolving the corridor.
    let facing = 0.0;

    for (index, node) in path.nodes.iter().enumerate() {
        let depth = index as f64 - camera;
        let (cx, mut cz) = (0.0, depth);
        // A stop already behind the camera stays out. The yaw would otherwise
        // swing it back into the frame and dissolve it across the turn.
        if turning && depth < -0.5 {
            cz = -SLIDE - 1.0;
        }
        // Only the page underfoot stays solid. Stops further along the path
        // keep their ordinary depth, so the turn does not stamp a full-size
        // copy of every stop you have already passed.
        let layout = project_views(
            params,
            node,
            cx,
            cz,
            fade,
            facing,
            turning && depth.abs() < 0.45,
            0,
        );
        if let Some(center) = project(params, cx, cz, params.road_anchor_y) {
            let opacity = depth_opacity(cz, params) * fade;
            if opacity > 0.02 {
                sink.markers.push(ProjectedMarker {
                    node_id: node.id.clone(),
                    path_id: path_id.to_string(),
                    x: center.x,
                    y: center.y,
                    half_width: params.road_half_width * center.scale,
                    opacity,
                    scale: center.scale,
                    depth: cz,
                });
            }
        }
        sink.nodes.push(ProjectedNode {
            id: node.id.clone(),
            label: node.label.clone(),
            path_id: path_id.to_string(),
            index,
            depth: cz,
            orientation: node.orientation,
            active: false,
            interactive: layout.interactive,
            x: layout.x,
            y: layout.y,
            yaw: layout.yaw,
            hinge_x: layout.x,
            hinge_y: layout.y,
            views: layout.views,
        });
    }

    let mut samples = Vec::new();
    let mut z = 0.02;
    let far = (path.nodes.len() as f64 - camera + 1.6).clamp(2.4, params.max_depth);
    while z <= far {
        if let Some(sample) = road_sample(params, z, 0.0, 0.0, fade) {
            samples.push(sample);
        }
        z += 0.26;
    }
    if samples.len() >= 2 {
        sink.ribbons.push(Ribbon {
            path_id: path_id.to_string(),
            role,
            samples,
        });
    }
    if turning {
        let sweep = signed_angle.abs().to_degrees();
        let sign = signed_angle.signum();
        // The viewer is inside the room. A branch on the right hinges this
        // face on its left edge and opens it outward — the free edge swings
        // away as the face turns off to the left.
        // Slide toward the hinge so the face clears the middle, where the viewer stands.
        let slide = -sign * params.front_width * 0.34 * align;
        hang_door(
            params,
            path_id,
            sign * sweep * align,
            sign < 0.0,
            slide,
            sink,
        );
    }
}

struct Angled {
    align: f64,
    signed_angle: f64,
    junction_depth: f64,
    /// Ribbon and floor markers. Independent of whether the views are shown.
    fade: f64,
    /// When false, the child path contributes a floor only. Its views stay out
    /// of the frame until a turn onto that path begins.
    include_views: bool,
    /// Multiplier for the child views during a turn. 0 at the doorway.
    reveal: f64,
    role: RibbonRole,
}

fn push_angled(nav: &Navigator, path_id: &str, angled: Angled, sink: &mut Sink<'_>) {
    let Some(path) = nav.graph.path(path_id) else {
        return;
    };
    let params = &nav.params;

    // A branch the traveler has not turned into keeps its floor, so the
    // opening reads as a way off, and withholds its views. Those views are
    // the host's content — pages, scenes, whatever they mounted — and they
    // belong to the child path. Once the turn starts, the branch is the next
    // face of the room and rotates forward with it.
    if angled.include_views {
        for (index, node) in path.nodes.iter().enumerate() {
            let cz = index as f64;
            let layout = project_views(
                params,
                node,
                0.0,
                cz,
                angled.fade * angled.reveal,
                0.0,
                index == 0,
                0,
            );
            if let Some(center) = project(params, 0.0, cz, params.road_anchor_y) {
                let opacity = depth_opacity(cz, params) * angled.fade;
                if opacity > 0.02 {
                    sink.markers.push(ProjectedMarker {
                        node_id: node.id.clone(),
                        path_id: path_id.to_string(),
                        x: center.x,
                        y: center.y,
                        half_width: params.road_half_width * center.scale,
                        opacity,
                        scale: center.scale,
                        depth: cz,
                    });
                }
            }
            sink.nodes.push(ProjectedNode {
                id: node.id.clone(),
                label: node.label.clone(),
                path_id: path_id.to_string(),
                index,
                depth: cz,
                orientation: node.orientation,
                active: false,
                interactive: layout.interactive,
                x: layout.x,
                y: layout.y,
                yaw: layout.yaw,
                hinge_x: layout.x,
                hinge_y: layout.y,
                views: layout.views,
            });
        }
    }

    let mut samples = Vec::new();
    if angled.include_views {
        let mut z = 0.02;
        let far = (path.nodes.len() as f64 + 1.6).clamp(2.4, params.max_depth);
        while z <= far {
            if let Some(sample) = road_sample(params, z, 0.0, 0.0, angled.fade) {
                samples.push(sample);
            }
            z += 0.26;
        }
    } else {
        let max_s = path.nodes.len() as f64 + 0.2;
        let mut s = 0.0;
        let edge = params.road_half_width / params.lane;
        while s <= max_s {
            let (x, z) = branch_xz(s, 0.0, &angled);
            let (lx, lz) = branch_xz(s, -edge, &angled);
            let (rx, rz) = branch_xz(s, edge, &angled);
            if let (Some(center), Some(left), Some(right)) = (
                project(params, x, z, params.road_anchor_y),
                project(params, lx, lz, params.road_anchor_y),
                project(params, rx, rz, params.road_anchor_y),
            ) {
                let opacity = road_opacity(z, params) * angled.fade;
                if opacity > 0.02 {
                    samples.push(RibbonSample {
                        center_x: center.x,
                        center_y: center.y,
                        left_x: left.x,
                        left_y: left.y,
                        right_x: right.x,
                        right_y: right.y,
                        opacity,
                        depth: z,
                    });
                }
            }
            s += 0.26;
        }
    }
    if samples.len() >= 2 {
        sink.ribbons.push(Ribbon {
            path_id: path_id.to_string(),
            role: angled.role,
            samples,
        });
    }
    if angled.include_views {
        let sweep = angled.signed_angle.abs().to_degrees();
        let sign = angled.signed_angle.signum();
        // Hinged on the near edge, so a right-hand alley starts folded at the
        // right and opens inward onto the camera.
        // Start out at the near side and arrive in front. The middle stays the viewer's.
        let slide = sign * params.front_width * 0.55 * (1.0 - angled.align);
        hang_door(
            params,
            path_id,
            -sign * sweep * (1.0 - angled.align),
            sign > 0.0,
            slide,
            sink,
        );
    }
}

fn hint_for(
    nav: &Navigator,
    branch: &crate::graph::Branch,
    junction_depth: f64,
) -> Option<BranchHint> {
    let params = &nav.params;
    let angled = Angled {
        align: 0.0,
        signed_angle: branch.signed_angle(params.branch_angle_rad()),
        junction_depth,
        fade: 1.0,
        include_views: false,
        reveal: 0.0,
        role: RibbonRole::Branch,
    };
    let (x, z) = branch_xz(0.7, 0.0, &angled);
    let screen = project(params, x, z, params.panel_anchor_y)?;
    let distance = branch.from_index as f64 - nav.position;
    let in_range = nav.turn.is_none() && distance.abs() <= params.ui_range;
    let mut opacity = depth_opacity(z, params);
    if !in_range {
        opacity *= 0.55;
    }
    if opacity < 0.04 {
        return None;
    }
    // Far away, the link recedes with the junction. Close up, it steps out
    // to the side of the front composition so it stays a way through, not a
    // label sitting on the host's views.
    let near = smoothstep(1.0 - (junction_depth.abs() / params.ui_range.max(0.2)).clamp(0.0, 1.0));
    let edge = (params.vanish_x + branch.side.sign() * (params.front_width * 0.5 + 0.045))
        .clamp(0.06, 0.94);
    Some(BranchHint {
        id: branch.id.clone(),
        label: branch.label.clone(),
        side: branch.side,
        from_node: branch.from_node.clone(),
        to_path: branch.to_path.clone(),
        distance,
        in_range,
        x: lerp(screen.x, edge, near),
        y: lerp(screen.y, params.foreground_y, near * 0.85),
        opacity,
        scale: lerp(screen.scale, 1.0, near),
        z_index: z_index_for(z) + 40,
    })
}

fn road_sample(
    params: &Params,
    depth: f64,
    swing: f64,
    slide: f64,
    fade: f64,
) -> Option<RibbonSample> {
    let edge = params.road_half_width / params.lane;
    let (mut cx, cz) = rotate_xz(0.0, depth, swing);
    let (mut lx, lz) = rotate_xz(-edge, depth, swing);
    let (mut rx, rz) = rotate_xz(edge, depth, swing);
    cx += slide;
    lx += slide;
    rx += slide;
    let center = project(params, cx, cz, params.road_anchor_y)?;
    let left = project(params, lx, lz, params.road_anchor_y)?;
    let right = project(params, rx, rz, params.road_anchor_y)?;
    let opacity = road_opacity(cz, params) * fade;
    if opacity < 0.02 {
        return None;
    }
    Some(RibbonSample {
        center_x: center.x,
        center_y: center.y,
        left_x: left.x,
        left_y: left.y,
        right_x: right.x,
        right_y: right.y,
        opacity,
        depth: cz,
    })
}

/// How far, in stops, a page ahead of the camera eases open into the front layout.
const APPROACH: f64 = 0.85;
/// How far, in stops, the page you walk past keeps moving before it is gone.
const SLIDE: f64 = 1.0;
const SLIDE_GROW: f64 = 0.1;

struct Slot {
    id: String,
    emphasis: f64,
    dx: f64,
    dy: f64,
    width: f64,
    height: f64,
    /// Signed offset from the grid center, about -1 at one edge and 1 at the other.
    axis_x: f64,
    axis_y: f64,
}

struct GroupPose {
    x: f64,
    y: f64,
    scale: f64,
    opacity: f64,
    /// 0 while the views are still collapsed in the distance, 1 when the columns are open.
    open: f64,
    /// 0 at the camera, 1 when the views have finished leaving.
    explode: f64,
    z_index: i32,
    yaw: f64,
}

struct Layout {
    views: Vec<ProjectedView>,
    interactive: bool,
    x: f64,
    y: f64,
    yaw: f64,
}

fn project_views(
    params: &Params,
    node: &Node,
    world_x: f64,
    z: f64,
    fade: f64,
    path_angle: f64,
    // Keep this page solid. Used for the stop a turn is leaving and the stop
    // a turn is arriving on, so the yaw reads as a rotation rather than a fade.
    solid: bool,
    z_bias: i32,
) -> Layout {
    let slots = slots_for(params, node);
    let Some(pose) = group_pose(params, world_x, z, fade, path_angle, solid, z_bias) else {
        return Layout {
            views: slots.iter().map(|slot| hidden_view(slot, params)).collect(),
            interactive: false,
            x: params.vanish_x,
            y: params.vanish_y,
            yaw: 0.0,
        };
    };
    let mut interactive = false;
    let views = slots
        .iter()
        .map(|slot| {
            // Every grid leaves the same way: each cell travels out from the
            // center in proportion to where it sits, and the stop fades.
            let ox = slot.dx * pose.open + slot.axis_x * params.explode * pose.explode;
            let oy = slot.dy * pose.open + slot.axis_y * params.explode * pose.explode;
            let fit = pose.scale;
            let opacity = pose.opacity;
            if opacity > 0.4 && fit > 0.22 && (-0.05..3.8).contains(&z) {
                interactive = true;
            }
            ProjectedView {
                id: slot.id.clone(),
                emphasis: slot.emphasis,
                x: pose.x + ox * fit,
                y: pose.y + oy * fit,
                width: slot.width * fit,
                height: slot.height * fit,
                scale: fit,
                opacity,
                rotate_x: slot.axis_y * 14.0 * pose.explode,
                rotate_y: slot.axis_x * 14.0 * pose.explode + pose.yaw,
                z_index: pose.z_index,
            }
        })
        .collect();
    Layout {
        views,
        interactive,
        x: pose.x,
        y: pose.y,
        yaw: pose.yaw,
    }
}

struct Track {
    center: f64,
    size: f64,
}

fn slots_for(params: &Params, node: &Node) -> Vec<Slot> {
    // `Graph` rejects a stop whose views do not fit, so this is the layout
    // those checks already accepted.
    let placement = place_views(node).unwrap_or_else(|_| Placement {
        columns: 1,
        rows: 1,
        cells: Vec::new(),
    });
    if placement.columns <= 1 && placement.rows <= 1 {
        let view = node.views.first();
        return vec![Slot {
            id: view.map(|view| view.id.clone()).unwrap_or_default(),
            emphasis: view.map(|view| view.emphasis).unwrap_or(100.0),
            dx: 0.0,
            dy: 0.0,
            width: params.front_width,
            height: params.page_height,
            axis_x: 0.0,
            axis_y: 0.0,
        }];
    }

    let columns = tracks(
        &track_weights(&placement, true),
        params.front_width,
        params.view_gap,
    );
    let row_span = if placement.rows <= 1 {
        params.column_height
    } else {
        params.front_height
    };
    let rows = if placement.rows <= 1 {
        vec![Track {
            center: 0.0,
            size: row_span,
        }]
    } else {
        tracks(&track_weights(&placement, false), row_span, params.view_gap)
    };
    let half_x = params.front_width / 2.0;
    let half_y = row_span / 2.0;

    placement
        .cells
        .iter()
        .map(|cell| {
            let (dx, width) = cover(&columns, cell.column, cell.col_span);
            let (dy, height) = cover(&rows, cell.row, cell.row_span);
            Slot {
                id: cell.id.clone(),
                emphasis: cell.emphasis,
                dx,
                dy,
                width,
                height,
                axis_x: if half_x > 0.001 { dx / half_x } else { 0.0 },
                axis_y: if placement.rows <= 1 || half_y <= 0.001 {
                    0.0
                } else {
                    dy / half_y
                },
            }
        })
        .collect()
}

/// Track weight is the largest share contributed by a view that covers it.
/// A spanning view divides its emphasis across the tracks it covers, so the
/// cell's total still matches the share the author wrote.
fn track_weights(placement: &Placement, columns: bool) -> Vec<f64> {
    let count = if columns {
        placement.columns
    } else {
        placement.rows
    } as usize;
    let mut weight = vec![0.0; count];
    for cell in &placement.cells {
        let (start, span) = if columns {
            (cell.column, cell.col_span)
        } else {
            (cell.row, cell.row_span)
        };
        let share = cell.emphasis / span.max(1) as f64;
        let end = ((start + span) as usize).min(count);
        for slot in weight.iter_mut().take(end).skip(start as usize) {
            if share > *slot {
                *slot = share;
            }
        }
    }
    let known: Vec<f64> = weight
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect();
    let fill = if known.is_empty() {
        1.0
    } else {
        known.iter().sum::<f64>() / known.len() as f64
    };
    for slot in &mut weight {
        if *slot <= 0.0 {
            *slot = fill;
        }
    }
    weight
}

fn tracks(weights: &[f64], span: f64, gap: f64) -> Vec<Track> {
    if weights.is_empty() {
        return Vec::new();
    }
    if weights.len() == 1 {
        return vec![Track {
            center: 0.0,
            size: span,
        }];
    }
    let sum = weights.iter().sum::<f64>().max(0.001);
    let inner = (span - gap * (weights.len() as f64 - 1.0)).max(0.0);
    let mut cursor = -span / 2.0;
    weights
        .iter()
        .map(|weight| {
            let size = inner * (weight / sum);
            let center = cursor + size / 2.0;
            cursor += size + gap;
            Track { center, size }
        })
        .collect()
}

fn cover(tracks: &[Track], start: u32, span: u32) -> (f64, f64) {
    let Some(first) = tracks.get(start as usize) else {
        return (0.0, 0.0);
    };
    let last = tracks
        .get((start + span.saturating_sub(1)) as usize)
        .unwrap_or(first);
    let left = first.center - first.size / 2.0;
    let right = last.center + last.size / 2.0;
    ((left + right) / 2.0, (right - left).max(0.0))
}

fn group_pose(
    params: &Params,
    world_x: f64,
    z: f64,
    fade: f64,
    path_angle: f64,
    solid: bool,
    z_bias: i32,
) -> Option<GroupPose> {
    if z < -SLIDE - 0.05 || z > params.max_depth + 1.5 {
        return None;
    }
    // `path_angle` is the heading still left between this path and the camera.
    // The whole stop yaws by that angle, so a turn reads as the camera rotating
    // onto the branch rather than the pages fading in place.
    let yaw = -path_angle.to_degrees();
    if z < 0.0 {
        let t = (-z / SLIDE).clamp(0.0, 1.0);
        let travel = smoothstep(t);
        let held = project(params, world_x, 0.05, params.panel_anchor_y);
        let on_axis = world_x.abs() < 0.25;
        Some(GroupPose {
            x: held
                .as_ref()
                .map(|screen| screen.x)
                .unwrap_or(params.vanish_x),
            y: if on_axis {
                params.foreground_y + 0.04 * travel
            } else {
                held.map(|screen| screen.y).unwrap_or(params.foreground_y)
            },
            scale: 1.0 + SLIDE_GROW * travel,
            opacity: if solid {
                fade.clamp(0.0, 1.0)
            } else {
                ((1.0 - smoothstep(t)) * fade).clamp(0.0, 1.0)
            },
            open: 1.0,
            explode: if solid { 0.0 } else { travel },
            z_index: 2400 - (travel * 200.0).round() as i32 + z_bias,
            yaw,
        })
    } else {
        let screen = project(params, world_x, z, params.panel_anchor_y)?;
        let open = smoothstep(1.0 - (z / APPROACH).clamp(0.0, 1.0));
        Some(GroupPose {
            x: screen.x,
            y: lerp(screen.y, params.foreground_y, open),
            scale: lerp(screen.scale, 1.0, open),
            opacity: if solid {
                fade.clamp(0.0, 1.0)
            } else {
                (depth_opacity(z, params) * fade).clamp(0.0, 1.0)
            },
            open,
            explode: 0.0,
            z_index: z_index_for(z) + z_bias,
            yaw,
        })
    }
}

fn hidden_view(slot: &Slot, params: &Params) -> ProjectedView {
    ProjectedView {
        id: slot.id.clone(),
        emphasis: slot.emphasis,
        x: params.vanish_x,
        y: params.vanish_y,
        width: 0.02,
        height: 0.02,
        scale: 0.02,
        opacity: 0.0,
        rotate_x: 0.0,
        rotate_y: 0.0,
        z_index: 0,
    }
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mark_active(nodes: &mut [ProjectedNode]) {
    let mut best: Option<(usize, f64)> = None;
    for (index, node) in nodes.iter().enumerate() {
        let opacity = node
            .views
            .iter()
            .map(|view| view.opacity)
            .fold(0.0_f64, f64::max);
        if opacity < 0.15 {
            continue;
        }
        let score = node.depth.abs();
        if best.map(|(_, previous)| score < previous).unwrap_or(true) {
            best = Some((index, score));
        }
    }
    if let Some((index, _)) = best {
        nodes[index].active = true;
    }
}

/// Hang one face of the junction on an edge and yaw it, in degrees.
///
/// The viewer stands in the middle of the room. `hinge_right` picks the edge.
/// A negative yaw brings the free edge toward the viewer, so the face opens
/// inward. Stops that are not the face itself stay out of the turn.
fn hang_door(
    params: &Params,
    path_id: &str,
    yaw: f64,
    hinge_right: bool,
    slide: f64,
    sink: &mut Sink<'_>,
) {
    if yaw.abs() < 0.4 && slide.abs() < 1e-4 {
        return;
    }
    for node in sink.nodes.iter_mut().filter(|node| node.path_id == path_id) {
        node.x += slide;
        for view in &mut node.views {
            view.x += slide;
        }
    }
    for marker in sink
        .markers
        .iter_mut()
        .filter(|marker| marker.path_id == path_id)
    {
        marker.x += slide;
    }
    for ribbon in sink
        .ribbons
        .iter_mut()
        .filter(|ribbon| ribbon.path_id == path_id)
    {
        for sample in &mut ribbon.samples {
            sample.center_x += slide;
            sample.left_x += slide;
            sample.right_x += slide;
        }
    }
    let Some(front) = sink
        .nodes
        .iter()
        .filter(|node| node.path_id == path_id && node.depth.abs() < 0.55)
        .min_by(|a, b| {
            a.depth
                .abs()
                .partial_cmp(&b.depth.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return;
    };
    let scale = front.views.first().map(|view| view.scale).unwrap_or(1.0);
    let half = params.front_width * scale * 0.5;
    let hinge_x = front.x + if hinge_right { half } else { -half };
    let hinge_y = front.y;
    // The face that still presents itself to the viewer paints in front.
    let bias = ((1.0 - (yaw.abs() / 90.0).clamp(0.0, 1.0)) * 520.0).round() as i32;
    for node in sink.nodes.iter_mut().filter(|node| node.path_id == path_id) {
        if node.depth.abs() > 0.55 {
            for view in &mut node.views {
                view.opacity = 0.0;
            }
            continue;
        }
        node.hinge_x = hinge_x;
        node.hinge_y = hinge_y;
        node.yaw = yaw;
        for view in &mut node.views {
            view.rotate_y += yaw;
            view.z_index += bias;
        }
    }
    for marker in sink
        .markers
        .iter_mut()
        .filter(|marker| marker.path_id == path_id)
    {
        marker.x = fold_x(marker.x, hinge_x, yaw);
    }
    for ribbon in sink
        .ribbons
        .iter_mut()
        .filter(|ribbon| ribbon.path_id == path_id)
    {
        for sample in &mut ribbon.samples {
            sample.center_x = fold_x(sample.center_x, hinge_x, yaw);
            sample.left_x = fold_x(sample.left_x, hinge_x, yaw);
            sample.right_x = fold_x(sample.right_x, hinge_x, yaw);
        }
    }
}

/// Draw a screen x in toward `hinge` as a door yaws. `yaw` is degrees.
fn fold_x(x: f64, hinge: f64, yaw: f64) -> f64 {
    let c = yaw.to_radians().cos().abs().max(0.12);
    hinge + (x - hinge) * c
}

/// Rotate a camera-space point around the camera. `angle` is yaw toward +x.
fn rotate_xz(x: f64, z: f64, angle: f64) -> (f64, f64) {
    let (s, c) = angle.sin_cos();
    (x * c + z * s, -x * s + z * c)
}

/// Point on a branch. `distance` is stops from the junction along the branch.
/// `align` 0 is the preview pose; `align` 1 has swung the branch onto the camera axis
/// and walked one stop onto it, so branch node 0 sits at the camera.
fn branch_xz(distance: f64, side: f64, angled: &Angled) -> (f64, f64) {
    let angle = angled.signed_angle * (1.0 - angled.align);
    let straight_z = distance - angled.align;
    let (x, z_rot) = rotate_xz(side, straight_z, angle);
    (x, z_rot + angled.junction_depth * (1.0 - angled.align))
}

fn project(params: &Params, x_world: f64, z: f64, anchor_y: f64) -> Option<Screen> {
    if z < params.behind_limit - 0.4 || z > params.max_depth + 1.5 {
        return None;
    }
    let denom = params.focal + z;
    if denom < 0.12 {
        return None;
    }
    let scale = params.focal / denom;
    if !(0.02..=2.8).contains(&scale) {
        return None;
    }
    Some(Screen {
        x: params.vanish_x + x_world * params.lane * scale,
        y: params.vanish_y + (anchor_y - params.vanish_y) * scale,
        scale,
    })
}

fn depth_opacity(z: f64, params: &Params) -> f64 {
    if z < params.behind_limit || z > params.max_depth {
        return 0.0;
    }
    if z < 0.0 {
        (1.0 - z / params.behind_limit).clamp(0.0, 1.0)
    } else {
        (1.0 / (1.0 + z.powf(1.28) * 0.42)).clamp(0.0, 1.0)
    }
}

fn road_opacity(z: f64, params: &Params) -> f64 {
    if !(-0.05..=params.max_depth + 2.0).contains(&z) {
        return 0.0;
    }
    (1.0 / (1.0 + z.max(0.0) * 0.11)).clamp(0.0, 1.0)
}

fn z_index_for(z: f64) -> i32 {
    if z < 0.0 {
        40
    } else {
        (1600.0 - z * 100.0).round() as i32
    }
}
