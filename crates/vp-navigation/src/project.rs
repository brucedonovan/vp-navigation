use serde::Serialize;

use crate::graph::Side;
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
    pub active: bool,
    pub interactive: bool,
    pub panels: Vec<ProjectedPanel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedPanel {
    pub side: Side,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub opacity: f64,
    /// Degrees. Positive rotateY turns the panel's right edge away from the viewer.
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
        let parent_fade = (1.0 - turn.align).clamp(0.0, 1.0);
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
                        fade: 1.0,
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
    role: RibbonRole,
    sink: &mut Sink<'_>,
) {
    let Some(path) = nav.graph.path(path_id) else {
        return;
    };
    let params = &nav.params;
    let swing = -signed_angle * align;

    for (index, node) in path.nodes.iter().enumerate() {
        let depth = index as f64 - camera;
        let (cx, cz) = rotate_xz(0.0, depth, swing);
        let mut panels = Vec::with_capacity(2);
        let mut interactive = false;
        for (side, sign) in [(Side::Left, -1.0), (Side::Right, 1.0)] {
            let (x, z) = rotate_xz(sign, depth, swing);
            let panel = panel_from_world(params, side, sign, x, z, swing, fade);
            interactive = interactive || panel_interactive(&panel, z);
            panels.push(panel);
        }
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
            active: false,
            interactive,
            panels,
        });
    }

    let mut samples = Vec::new();
    let mut z = 0.02;
    let far = (path.nodes.len() as f64 - camera + 1.6).clamp(2.4, params.max_depth);
    while z <= far {
        if let Some(sample) = road_sample(params, z, swing, fade) {
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
}

struct Angled {
    align: f64,
    signed_angle: f64,
    junction_depth: f64,
    fade: f64,
    role: RibbonRole,
}

fn push_angled(nav: &Navigator, path_id: &str, angled: Angled, sink: &mut Sink<'_>) {
    let Some(path) = nav.graph.path(path_id) else {
        return;
    };
    let params = &nav.params;
    let remain = angled.signed_angle * (1.0 - angled.align);

    for (index, node) in path.nodes.iter().enumerate() {
        let distance = index as f64 + 1.0;
        let (cx, cz) = branch_xz(distance, 0.0, &angled);
        let mut panels = Vec::with_capacity(2);
        let mut interactive = false;
        for (side, sign) in [(Side::Left, -1.0), (Side::Right, 1.0)] {
            let (x, z) = branch_xz(distance, sign, &angled);
            let panel = panel_from_world(params, side, sign, x, z, remain, angled.fade);
            interactive = interactive || panel_interactive(&panel, z);
            panels.push(panel);
        }
        if let Some(center) = project(params, cx, cz, params.road_anchor_y) {
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
            active: false,
            interactive,
            panels,
        });
    }

    let mut samples = Vec::new();
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
    if samples.len() >= 2 {
        sink.ribbons.push(Ribbon {
            path_id: path_id.to_string(),
            role: angled.role,
            samples,
        });
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
        role: RibbonRole::Branch,
    };
    let (x, z) = branch_xz(0.55, 0.0, &angled);
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
    Some(BranchHint {
        id: branch.id.clone(),
        label: branch.label.clone(),
        side: branch.side,
        from_node: branch.from_node.clone(),
        to_path: branch.to_path.clone(),
        distance,
        in_range,
        x: screen.x,
        y: screen.y,
        opacity,
        scale: screen.scale,
        z_index: z_index_for(z) + 40,
    })
}

fn road_sample(params: &Params, depth: f64, swing: f64, fade: f64) -> Option<RibbonSample> {
    let edge = params.road_half_width / params.lane;
    let (cx, cz) = rotate_xz(0.0, depth, swing);
    let (lx, lz) = rotate_xz(-edge, depth, swing);
    let (rx, rz) = rotate_xz(edge, depth, swing);
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

fn panel_from_world(
    params: &Params,
    side: Side,
    side_sign: f64,
    x: f64,
    z: f64,
    path_angle: f64,
    fade: f64,
) -> ProjectedPanel {
    if let Some(screen) = project(params, x, z, params.panel_anchor_y) {
        ProjectedPanel {
            side,
            x: screen.x,
            y: screen.y,
            scale: screen.scale,
            opacity: (depth_opacity(z, params) * fade).clamp(0.0, 1.0),
            rotate_y: -side_sign * 14.0 - path_angle.to_degrees() * 0.45,
            z_index: z_index_for(z),
        }
    } else {
        ProjectedPanel {
            side,
            x: params.vanish_x,
            y: params.vanish_y,
            scale: 0.02,
            opacity: 0.0,
            rotate_y: 0.0,
            z_index: 0,
        }
    }
}

fn panel_interactive(panel: &ProjectedPanel, z: f64) -> bool {
    panel.opacity > 0.4 && panel.scale > 0.22 && (-0.02..3.8).contains(&z)
}

fn mark_active(nodes: &mut [ProjectedNode]) {
    let mut best: Option<(usize, f64)> = None;
    for (index, node) in nodes.iter().enumerate() {
        let opacity = node
            .panels
            .iter()
            .map(|panel| panel.opacity)
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
