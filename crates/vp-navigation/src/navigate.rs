use serde::Serialize;

use crate::error::NavError;
use crate::graph::{Graph, Side};
use crate::params::Params;
use crate::project::{self, Frame};

/// Where the traveler is, which stops are on the active path, and which forks are open.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavState {
    pub path_id: String,
    pub position: f64,
    pub target: f64,
    pub at_rest: bool,
    /// The camera is holding on a stop so its views can be read.
    pub holding: bool,
    pub can_return: bool,
    /// Label of the parent stop a return would land on.
    pub return_label: Option<String>,
    pub current_node: String,
    pub current_label: String,
    pub nodes: Vec<StateNode>,
    pub available_branches: Vec<StateBranch>,
    pub turning: Option<TurnView>,
    pub pending: Option<PendingView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateNode {
    pub id: String,
    pub label: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBranch {
    pub id: String,
    pub label: String,
    pub side: Side,
    pub from_node: String,
    pub to_path: String,
    /// Signed stops from the camera to the junction. Positive means the junction is ahead.
    pub distance: f64,
    pub in_range: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnView {
    pub branch_id: String,
    pub direction: TurnDirection,
    /// 0 is the parent frame, 1 is fully turned onto the branch.
    pub align: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnDirection {
    Enter,
    Exit,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingView {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone)]
enum Pending {
    Enter {
        branch_id: String,
        then_index: Option<usize>,
    },
    Exit {
        then_index: Option<usize>,
    },
}

/// A wheel gesture. A short impulse steps once. A stream that is still pushing
/// after that impulse becomes a drag and travels through stops.
#[derive(Debug, Clone)]
enum Wheel {
    Idle,
    Impulse {
        started_ms: f64,
        origin: f64,
        sign: f64,
        accum: f64,
        samples: Vec<(f64, f64)>,
    },
    Drag {
        origin: f64,
        sign: f64,
        accum: f64,
    },
    /// The impulse was a flick. Later deltas from the same burst are inertia.
    Swallow {
        sign: f64,
    },
}

const WHEEL_QUIET_S: f64 = 0.14;
const WHEEL_DECIDE_MS: f64 = 380.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnKind {
    Enter,
    Exit,
}

#[derive(Debug, Clone)]
pub(crate) struct Turn {
    pub kind: TurnKind,
    pub align: f64,
    pub align_target: f64,
    pub branch_id: String,
    pub parent_path: String,
    pub child_path: String,
    pub junction_index: usize,
    pub dest_index: usize,
    pub signed_angle: f64,
}

/// Owns the path graph and the camera that travels it.
///
/// `position` is the eased camera, in stops along the active path.
/// `target` is where input asked to go. [`Navigator::tick`] closes the gap.
/// Integer positions sit on a stop; fractional positions are between stops.
#[derive(Debug)]
pub struct Navigator {
    pub(crate) graph: Graph,
    pub(crate) params: Params,
    pub(crate) path_id: String,
    pub(crate) position: f64,
    pub(crate) target: f64,
    pub(crate) turn: Option<Turn>,
    /// Seconds left in the hold on [`hold_at`](Navigator::hold_at).
    dwell_left: f64,
    hold_at: f64,
    /// Seconds since the last wheel event. The light settle waits on this.
    scroll_idle: f64,
    wheel: Wheel,
    pending: Option<Pending>,
}

impl Navigator {
    pub fn new(graph: Graph) -> Self {
        Self::with_params(graph, Params::default()).expect("default params are valid")
    }

    pub fn with_params(graph: Graph, params: Params) -> Result<Self, NavError> {
        params.validate()?;
        let path_id = graph.root().to_string();
        Ok(Self {
            graph,
            params,
            path_id,
            position: 0.0,
            target: 0.0,
            turn: None,
            dwell_left: 0.0,
            hold_at: 0.0,
            scroll_idle: 10.0,
            wheel: Wheel::Idle,
            pending: None,
        })
    }

    pub fn from_config_json(json: &str) -> Result<Self, NavError> {
        let (graph, params) = crate::config::parse_config(json)?;
        Self::with_params(graph, params)
    }

    pub fn params(&self) -> &Params {
        &self.params
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Advance easing. `dt_ms` is milliseconds since the previous call.
    pub fn tick(&mut self, dt_ms: f64) {
        if !dt_ms.is_finite() {
            return;
        }
        let dt = (dt_ms.max(0.0) / 1000.0).min(0.1);
        self.scroll_idle = (self.scroll_idle + dt).min(30.0);
        self.expire_wheel();
        if self.params.reduced_motion {
            self.snap();
            return;
        }
        if self.turn.is_none() {
            self.settle_spring(dt);
        }

        if self.dwell_left > 0.0 {
            self.dwell_left = (self.dwell_left - dt).max(0.0);
            self.position = self.hold_at;
            if self.dwell_left > 0.0 {
                return;
            }
        }

        let kind = self.turn.as_ref().map(|turn| turn.kind);
        // Exit parks the child-path position at the threshold. Enter keeps
        // walking up to the junction while the heading swings.
        if kind != Some(TurnKind::Exit) {
            let next = damp(self.position, self.target, self.params.stiffness, dt);
            if self.turn.is_none() && !matches!(self.wheel, Wheel::Drag { .. }) {
                if let Some(stop) = self.gate(self.position, next) {
                    self.position = stop;
                    self.hold_at = stop;
                    self.dwell_left = self.params.dwell_ms / 1000.0;
                } else {
                    self.position = next;
                    if (self.position - self.target).abs() < 5e-4 {
                        self.position = self.target;
                    }
                }
            } else {
                self.position = next;
                if (self.position - self.target).abs() < 5e-4 {
                    self.position = self.target;
                }
            }
        }

        if self.turn.is_none() {
            self.try_begin_pending();
        }

        let mut finished = false;
        if let Some(turn) = self.turn.as_mut() {
            turn.align = damp(
                turn.align,
                turn.align_target,
                self.params.turn_stiffness,
                dt,
            );
            finished = match turn.kind {
                TurnKind::Enter => {
                    let junction = turn.junction_index as f64;
                    (turn.align - 1.0).abs() < 0.012 && (self.position - junction).abs() < 0.03
                }
                TurnKind::Exit => turn.align < 0.012,
            };
        }
        if finished {
            if let Some(turn) = self.turn.take() {
                self.commit_turn(turn, false);
            }
        }
    }

    /// Move the target by a fractional number of stops. Positive walks forward,
    /// toward the vanishing point. The wheel is free: a gesture may cross
    /// several stops. After it goes quiet, a light spring settles the target
    /// onto a stop it landed near. On a branch, a backward move past the
    /// threshold arms a return to the parent.
    pub fn move_by(&mut self, delta: f64) {
        if self.turn.is_some() || !delta.is_finite() || delta == 0.0 {
            return;
        }
        self.wheel = Wheel::Idle;
        self.scroll_idle = 0.0;
        self.scroll_by(delta);
    }

    /// Wheel or trackpad sample. `now_ms` is a monotonic clock in milliseconds.
    ///
    /// The first impulse advances exactly one stop, the same as a key press.
    /// If the stream is still pushing after that — a held wheel, or a drag
    /// that keeps its speed — travel follows the gesture through later stops.
    /// A flick's decaying tail does not.
    pub fn scroll(&mut self, now_ms: f64, delta: f64) {
        if self.turn.is_some() || !now_ms.is_finite() || !delta.is_finite() || delta == 0.0 {
            return;
        }
        self.scroll_idle = 0.0;
        let sign = delta.signum();
        if let Wheel::Swallow { sign: swallowed } = self.wheel {
            if sign == swallowed {
                return;
            }
            self.wheel = Wheel::Idle;
        }
        if self.wheel_sign() != Some(sign) {
            self.wheel = Wheel::Idle;
        }
        match self.wheel {
            Wheel::Idle => self.begin_impulse(now_ms, delta),
            Wheel::Impulse { .. } => self.continue_impulse(now_ms, delta),
            Wheel::Drag { .. } => self.continue_drag(delta),
            Wheel::Swallow { .. } => {}
        }
    }

    /// Put the target on `stop`. A finished drag lands here, and so does a
    /// touch flick that should cover exactly one step.
    pub fn park(&mut self, stop: f64) {
        if self.turn.is_some() || !stop.is_finite() {
            return;
        }
        self.wheel = Wheel::Idle;
        self.dwell_left = 0.0;
        self.scroll_idle = 0.0;
        self.aim(stop);
    }

    fn scroll_by(&mut self, delta: f64) {
        if self.dwell_left > 0.0 {
            let going_forward = self.target > self.hold_at + 1e-6;
            let same_way = if going_forward {
                delta > 0.0
            } else {
                delta < 0.0
            };
            if same_way {
                let max = self.path_max(&self.path_id);
                self.target = (self.target + delta).clamp(0.0, max);
                if self.params.reduced_motion {
                    self.snap();
                }
                return;
            }
            self.dwell_left = 0.0;
            self.target = self.hold_at;
        }
        if delta > 0.0 && matches!(self.pending, Some(Pending::Exit { .. })) {
            self.pending = None;
        }
        if matches!(self.pending, Some(Pending::Enter { .. })) {
            self.pending = None;
        }

        let desired = self.target + delta;
        let on_branch = self.graph.parent_of(&self.path_id).is_some();
        let near_threshold = self.position <= self.params.turn_start_distance.max(0.35);
        // Half a stop past the threshold, so a small trackpad jitter does not
        // kick the traveler out of a branch they meant to stay on.
        if on_branch && desired < -0.45 && near_threshold {
            self.target = 0.0;
            self.pending = Some(Pending::Exit { then_index: None });
            self.try_begin_pending();
        } else {
            self.target = desired.clamp(0.0, self.path_max(&self.path_id));
        }
        if self.params.reduced_motion {
            self.snap();
        }
    }

    fn begin_impulse(&mut self, now_ms: f64, delta: f64) {
        let origin = self.target;
        let sign = delta.signum();
        self.step(sign as i32);
        self.wheel = Wheel::Impulse {
            started_ms: now_ms,
            origin,
            sign,
            accum: delta,
            samples: vec![(now_ms, delta)],
        };
        self.scroll_idle = 0.0;
    }

    fn continue_impulse(&mut self, now_ms: f64, delta: f64) {
        let Wheel::Impulse {
            started_ms,
            origin,
            sign,
            mut accum,
            mut samples,
        } = std::mem::replace(&mut self.wheel, Wheel::Idle)
        else {
            return;
        };
        accum += delta;
        samples.push((now_ms, delta));
        if samples.len() > 80 {
            let drop = samples.len() - 80;
            samples.drain(0..drop);
        }
        let age = now_ms - started_ms;
        if age < WHEEL_DECIDE_MS {
            self.wheel = Wheel::Impulse {
                started_ms,
                origin,
                sign,
                accum,
                samples,
            };
            return;
        }
        let early: f64 = samples
            .iter()
            .filter(|(t, _)| *t - started_ms <= 120.0)
            .map(|(_, d)| d.abs())
            .sum();
        let late: f64 = samples
            .iter()
            .filter(|(t, _)| now_ms - *t <= 120.0)
            .map(|(_, d)| d.abs())
            .sum();
        if late >= 0.08 && late + 1e-9 >= early * 0.5 {
            self.wheel = Wheel::Drag {
                origin,
                sign,
                accum,
            };
            self.dwell_left = 0.0;
            self.aim(origin + accum);
        } else if late < 0.08 || late + 1e-9 < early * 0.35 {
            self.wheel = Wheel::Swallow { sign };
        } else {
            self.wheel = Wheel::Impulse {
                started_ms,
                origin,
                sign,
                accum,
                samples,
            };
        }
    }

    fn continue_drag(&mut self, delta: f64) {
        let Wheel::Drag {
            origin,
            sign,
            accum,
        } = self.wheel
        else {
            return;
        };
        let accum = accum + delta;
        self.wheel = Wheel::Drag {
            origin,
            sign,
            accum,
        };
        self.dwell_left = 0.0;
        self.aim(origin + accum);
    }

    fn wheel_sign(&self) -> Option<f64> {
        match self.wheel {
            Wheel::Idle => None,
            Wheel::Impulse { sign, .. } | Wheel::Drag { sign, .. } | Wheel::Swallow { sign } => {
                Some(sign)
            }
        }
    }

    fn expire_wheel(&mut self) {
        if self.scroll_idle < WHEEL_QUIET_S {
            return;
        }
        match self.wheel {
            Wheel::Drag { origin, accum, .. } => {
                self.wheel = Wheel::Idle;
                if self.pending.is_none() && self.turn.is_none() {
                    let max = self.path_max(&self.path_id);
                    self.target = (origin + accum).round().clamp(0.0, max);
                }
            }
            Wheel::Impulse { .. } | Wheel::Swallow { .. } => self.wheel = Wheel::Idle,
            Wheel::Idle => {}
        }
    }

    /// Set the target, including the backward step off a branch.
    fn aim(&mut self, desired: f64) {
        if desired > 0.0 && matches!(self.pending, Some(Pending::Exit { .. })) {
            self.pending = None;
        }
        if matches!(self.pending, Some(Pending::Enter { .. })) {
            self.pending = None;
        }
        let on_branch = self.graph.parent_of(&self.path_id).is_some();
        let near_threshold = self.position <= self.params.turn_start_distance.max(0.35);
        if on_branch && desired < -0.45 && near_threshold {
            self.target = 0.0;
            self.pending = Some(Pending::Exit { then_index: None });
            self.try_begin_pending();
        } else {
            self.target = desired.clamp(0.0, self.path_max(&self.path_id));
        }
        if self.params.reduced_motion {
            self.snap();
        }
    }

    /// After the wheel has been quiet, ease a nearby target onto its stop.
    /// A target that is not close to a stop is left alone, so a long scroll
    /// still finishes where it was aimed.
    fn settle_spring(&mut self, dt: f64) {
        if self.params.spring <= 0.0 {
            return;
        }
        if self.scroll_idle < self.params.spring_idle_ms / 1000.0 {
            return;
        }
        let max = self.path_max(&self.path_id);
        let nearest = self.target.round().clamp(0.0, max);
        if (self.target - nearest).abs() > self.params.spring_range {
            return;
        }
        let rate = 3.0 + self.params.spring * 9.0;
        self.target = damp(self.target, nearest, rate, dt);
    }

    /// Step to the next or previous stop. `steps` is negative to go back.
    /// Stepping back off the first stop of a branch returns to the parent.
    pub fn step(&mut self, steps: i32) {
        if self.turn.is_some() || steps == 0 {
            return;
        }
        self.wheel = Wheel::Idle;
        if self.dwell_left > 0.0 {
            self.dwell_left = 0.0;
            self.target = self.hold_at;
        }
        if steps > 0 && matches!(self.pending, Some(Pending::Exit { .. })) {
            self.pending = None;
        }
        if matches!(self.pending, Some(Pending::Enter { .. })) {
            self.pending = None;
        }

        let next = if steps > 0 {
            (self.target + 1e-9).floor() + f64::from(steps)
        } else {
            (self.target - 1e-9).ceil() + f64::from(steps)
        };
        if self.graph.parent_of(&self.path_id).is_some() && next < -0.5 {
            self.target = 0.0;
            self.pending = Some(Pending::Exit { then_index: None });
            self.try_begin_pending();
        } else {
            self.target = next.clamp(0.0, self.path_max(&self.path_id));
        }
        if self.params.reduced_motion {
            self.snap();
        }
    }

    /// Animate to a stop on the active path.
    pub fn focus(&mut self, node_id: &str) -> Result<(), NavError> {
        if self.turn.is_some() {
            return Err(NavError::Busy);
        }
        let (path_id, index) = self.locate(node_id)?;
        if path_id != self.path_id {
            return Err(NavError::NotAvailable(format!(
                "'{node_id}' is not on the active path"
            )));
        }
        self.pending = None;
        self.wheel = Wheel::Idle;
        self.dwell_left = 0.0;
        self.target = index as f64;
        if self.params.reduced_motion {
            self.position = self.target;
        }
        Ok(())
    }

    /// Walk to `node_id`. If it sits on a branch connected to the active path,
    /// turn onto that branch first. If it sits on the parent, return first.
    pub fn activate(&mut self, node_id: &str) -> Result<(), NavError> {
        if self.turn.is_some() {
            return Err(NavError::Busy);
        }
        self.dwell_left = 0.0;
        self.wheel = Wheel::Idle;
        let (path_id, index) = self.locate(node_id)?;
        if path_id == self.path_id {
            self.pending = None;
            self.dwell_left = 0.0;
            self.target = index as f64;
            if self.params.reduced_motion {
                self.position = self.target;
            }
            return Ok(());
        }

        if let Some(branch_id) = self.branch_id_into(&path_id) {
            let from_path = self
                .graph
                .branch(&branch_id)
                .map(|b| b.from_path.clone())
                .unwrap_or_default();
            if from_path == self.path_id {
                let junction = self
                    .graph
                    .branch(&branch_id)
                    .map(|b| b.from_index)
                    .unwrap_or(0);
                self.pending = Some(Pending::Enter {
                    branch_id,
                    then_index: Some(index),
                });
                self.target = junction as f64;
                self.try_begin_pending();
                if self.params.reduced_motion {
                    self.snap();
                }
                return Ok(());
            }
        }

        if let Some(parent) = self.graph.parent_of(&self.path_id) {
            if parent.path_id == path_id {
                self.pending = Some(Pending::Exit {
                    then_index: Some(index),
                });
                self.target = 0.0;
                self.try_begin_pending();
                if self.params.reduced_motion {
                    self.snap();
                }
                return Ok(());
            }
        }

        Err(NavError::NotAvailable(format!(
            "'{node_id}' is not on the active path or a path connected to it"
        )))
    }

    /// Turn onto a branch that leaves the active path.
    ///
    /// If the camera is far from the junction, it walks there first and turns
    /// on arrival. With reduced motion the switch is immediate.
    pub fn take_branch(&mut self, branch_id: &str) -> Result<(), NavError> {
        if self.turn.is_some() {
            return Err(NavError::Busy);
        }
        let branch = self
            .graph
            .branch(branch_id)
            .ok_or_else(|| NavError::UnknownBranch(branch_id.to_string()))?;
        if branch.from_path != self.path_id {
            return Err(NavError::NotAvailable(format!(
                "branch '{branch_id}' is not attached to the active path"
            )));
        }
        let junction = branch.from_index as f64;
        self.dwell_left = 0.0;
        self.wheel = Wheel::Idle;
        self.pending = Some(Pending::Enter {
            branch_id: branch_id.to_string(),
            then_index: None,
        });
        self.target = junction;
        self.try_begin_pending();
        if self.params.reduced_motion {
            self.snap();
        }
        Ok(())
    }

    /// Leave the current branch and stand again on the parent junction.
    pub fn return_to_parent(&mut self) -> Result<(), NavError> {
        if self.turn.is_some() {
            return Err(NavError::Busy);
        }
        if self.graph.parent_of(&self.path_id).is_none() {
            return Err(NavError::NoParent);
        }
        self.dwell_left = 0.0;
        self.wheel = Wheel::Idle;
        self.pending = Some(Pending::Exit { then_index: None });
        self.target = 0.0;
        self.try_begin_pending();
        if self.params.reduced_motion {
            self.snap();
        }
        Ok(())
    }

    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.params.reduced_motion = enabled;
        if enabled {
            self.snap();
        }
    }

    /// Replace any projection fields present in `json`. Missing fields keep
    /// their current values, so a host can change `spring` without restating
    /// the camera. The same object is accepted as `projection` in the config.
    pub fn set_projection_json(&mut self, json: &str) -> Result<(), NavError> {
        let patch: serde_json::Value =
            serde_json::from_str(json).map_err(|err| NavError::InvalidConfig(err.to_string()))?;
        let serde_json::Value::Object(patch) = patch else {
            return Err(NavError::InvalidConfig(
                "projection settings must be a JSON object".into(),
            ));
        };
        let mut current = serde_json::to_value(&self.params)
            .map_err(|err| NavError::InvalidConfig(err.to_string()))?;
        let Some(object) = current.as_object_mut() else {
            return Err(NavError::InvalidConfig(
                "projection settings are unreadable".into(),
            ));
        };
        for (key, value) in patch {
            object.insert(key, value);
        }
        let params: Params = serde_json::from_value(current)
            .map_err(|err| NavError::InvalidConfig(err.to_string()))?;
        params.validate()?;
        let reduced = self.params.reduced_motion;
        self.params = params;
        if self.params.dwell_ms == 0.0 {
            self.dwell_left = 0.0;
        }
        if self.params.reduced_motion && !reduced {
            self.snap();
        }
        Ok(())
    }

    pub fn projection_json(&self) -> String {
        serde_json::to_string(&self.params).unwrap_or_else(|_| "{}".into())
    }

    /// How long to hold on a stop the camera would pass through.
    /// `0` turns the hold off. Values are clamped to `0..=4000` milliseconds.
    pub fn set_dwell_ms(&mut self, ms: f64) {
        self.params.dwell_ms = if ms.is_finite() {
            ms.clamp(0.0, 4_000.0)
        } else {
            0.0
        };
        if self.params.dwell_ms == 0.0 {
            self.dwell_left = 0.0;
        }
    }

    pub fn state(&self) -> NavState {
        let path = self
            .graph
            .path(&self.path_id)
            .expect("active path is part of the graph");
        let nodes = path
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| StateNode {
                id: node.id.clone(),
                label: node.label.clone(),
                index,
            })
            .collect();
        let current_index = nearest_index(self.position, path.nodes.len());
        let current = &path.nodes[current_index];

        let available_branches = self
            .graph
            .branches_from(&self.path_id)
            .map(|branch| {
                let distance = branch.from_index as f64 - self.position;
                StateBranch {
                    id: branch.id.clone(),
                    label: branch.label.clone(),
                    side: branch.side,
                    from_node: branch.from_node.clone(),
                    to_path: branch.to_path.clone(),
                    distance,
                    in_range: self.turn.is_none() && distance.abs() <= self.params.ui_range,
                }
            })
            .collect();

        let parent = self.graph.parent_of(&self.path_id);
        let return_label = parent.and_then(|parent| {
            self.graph
                .node(&parent.node_id)
                .map(|node| node.label.clone())
        });

        NavState {
            path_id: self.path_id.clone(),
            position: self.position,
            target: self.target,
            at_rest: self.turn.is_none()
                && self.pending.is_none()
                && self.dwell_left <= 0.0
                && (self.position - self.target).abs() < 0.004,
            holding: self.dwell_left > 0.0,
            can_return: parent.is_some(),
            return_label,
            current_node: current.id.clone(),
            current_label: current.label.clone(),
            nodes,
            available_branches,
            turning: self.turn.as_ref().map(|turn| TurnView {
                branch_id: turn.branch_id.clone(),
                direction: match turn.kind {
                    TurnKind::Enter => TurnDirection::Enter,
                    TurnKind::Exit => TurnDirection::Exit,
                },
                align: turn.align,
            }),
            pending: self.pending.as_ref().map(|pending| match pending {
                Pending::Enter { branch_id, .. } => PendingView {
                    kind: "enter".into(),
                    id: branch_id.clone(),
                },
                Pending::Exit { .. } => PendingView {
                    kind: "exit".into(),
                    id: String::new(),
                },
            }),
        }
    }

    pub fn project(&self) -> Frame {
        project::build(self)
    }

    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.state()).unwrap_or_else(|_| "{}".into())
    }

    pub fn frame_json(&self) -> String {
        serde_json::to_string(&self.project()).unwrap_or_else(|_| "{}".into())
    }

    fn locate(&self, node_id: &str) -> Result<(String, usize), NavError> {
        self.graph
            .node(node_id)
            .map(|loc| (loc.path_id.clone(), loc.index))
            .ok_or_else(|| NavError::UnknownNode(node_id.to_string()))
    }

    fn branch_id_into(&self, path_id: &str) -> Option<String> {
        self.graph
            .branch_into(path_id)
            .map(|branch| branch.id.clone())
    }

    fn path_max(&self, path_id: &str) -> f64 {
        self.graph
            .path(path_id)
            .map(|path| path.nodes.len().saturating_sub(1) as f64)
            .unwrap_or(0.0)
    }

    fn try_begin_pending(&mut self) {
        if self.turn.is_some() {
            return;
        }
        let Some(pending) = self.pending.clone() else {
            return;
        };
        match pending {
            Pending::Enter {
                branch_id,
                then_index,
            } => {
                let Some(branch) = self.graph.branch(&branch_id).cloned() else {
                    self.pending = None;
                    return;
                };
                if (self.position - branch.from_index as f64).abs()
                    > self.params.turn_start_distance
                {
                    return;
                }
                let child_len = self
                    .graph
                    .path(&branch.to_path)
                    .map(|path| path.nodes.len())
                    .unwrap_or(1);
                let dest = then_index.unwrap_or(0).min(child_len.saturating_sub(1));
                let signed_angle = branch.signed_angle(self.params.branch_angle_rad());
                let junction = branch.from_index;
                self.turn = Some(Turn {
                    kind: TurnKind::Enter,
                    align: 0.0,
                    align_target: 1.0,
                    branch_id,
                    parent_path: branch.from_path,
                    child_path: branch.to_path,
                    junction_index: junction,
                    dest_index: dest,
                    signed_angle,
                });
                self.target = junction as f64;
                self.pending = None;
            }
            Pending::Exit { then_index } => {
                if self.position > 0.07 {
                    return;
                }
                let Some(parent) = self.graph.parent_of(&self.path_id).cloned() else {
                    self.pending = None;
                    return;
                };
                let Some(branch) = self.graph.branch(&parent.branch_id).cloned() else {
                    self.pending = None;
                    return;
                };
                let parent_len = self
                    .graph
                    .path(&parent.path_id)
                    .map(|path| path.nodes.len())
                    .unwrap_or(1);
                let dest = then_index
                    .unwrap_or(parent.node_index)
                    .min(parent_len.saturating_sub(1));
                self.position = 0.0;
                self.target = 0.0;
                self.turn = Some(Turn {
                    kind: TurnKind::Exit,
                    align: 1.0,
                    align_target: 0.0,
                    branch_id: parent.branch_id,
                    parent_path: parent.path_id,
                    child_path: self.path_id.clone(),
                    junction_index: parent.node_index,
                    dest_index: dest,
                    signed_angle: branch.signed_angle(self.params.branch_angle_rad()),
                });
                self.pending = None;
            }
        }
    }

    fn commit_turn(&mut self, turn: Turn, instant: bool) {
        match turn.kind {
            TurnKind::Enter => {
                self.path_id = turn.child_path;
                let dest = (turn.dest_index as f64).clamp(0.0, self.path_max(&self.path_id));
                self.position = if instant { dest } else { 0.0 };
                self.target = dest;
            }
            TurnKind::Exit => {
                self.path_id = turn.parent_path;
                let junction = turn.junction_index as f64;
                let dest = (turn.dest_index as f64).clamp(0.0, self.path_max(&self.path_id));
                self.position = if instant { dest } else { junction };
                self.target = dest;
            }
        }
    }

    /// The next integer stop `from` → `to` would pass, when that stop is not
    /// the place the traveler asked to finish.
    fn gate(&self, from: f64, to: f64) -> Option<f64> {
        if self.params.dwell_ms <= 0.0 || (to - from).abs() < 1e-6 {
            return None;
        }
        let stop = if to > from {
            let stop = from.floor() + 1.0;
            if stop <= to + 1e-8 {
                stop
            } else {
                return None;
            }
        } else {
            let stop = from.ceil() - 1.0;
            if stop + 1e-8 >= to {
                stop
            } else {
                return None;
            }
        };
        if (self.target - stop).abs() < 0.02 {
            return None;
        }
        let max = self.path_max(&self.path_id);
        if !(0.0..=max).contains(&stop) {
            return None;
        }
        Some(stop)
    }

    fn snap(&mut self) {
        self.dwell_left = 0.0;
        if let Some(pending) = self.pending.take() {
            self.turn = None;
            self.apply_pending_instant(pending);
            return;
        }
        if let Some(turn) = self.turn.take() {
            self.commit_turn(turn, true);
            return;
        }
        self.position = self.target;
    }

    fn apply_pending_instant(&mut self, pending: Pending) {
        match pending {
            Pending::Enter {
                branch_id,
                then_index,
            } => {
                let Some(branch) = self.graph.branch(&branch_id).cloned() else {
                    return;
                };
                self.path_id = branch.to_path;
                let dest = then_index.unwrap_or(0) as f64;
                let dest = dest.clamp(0.0, self.path_max(&self.path_id));
                self.position = dest;
                self.target = dest;
            }
            Pending::Exit { then_index } => {
                let Some(parent) = self.graph.parent_of(&self.path_id).cloned() else {
                    return;
                };
                let dest = then_index.unwrap_or(parent.node_index) as f64;
                self.path_id = parent.path_id;
                let dest = dest.clamp(0.0, self.path_max(&self.path_id));
                self.position = dest;
                self.target = dest;
            }
        }
    }
}

fn damp(current: f64, target: f64, stiffness: f64, dt: f64) -> f64 {
    let k = 1.0 - (-stiffness * dt).exp();
    current + (target - current) * k
}

fn nearest_index(position: f64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let max = (len - 1) as f64;
    position.round().clamp(0.0, max) as usize
}
