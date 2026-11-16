use serde::{Deserialize, Serialize};

use crate::error::NavError;

/// Tunables for easing and the perspective camera.
///
/// Screen positions are fractions of the viewport: `(0, 0)` is the top-left,
/// `(1, 1)` is the bottom-right. The camera looks down the path toward
/// [`vanish_x`](Params::vanish_x) / [`vanish_y`](Params::vanish_y).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Params {
    /// Perspective focal length, in stops. Larger values flatten the recession.
    pub focal: f64,
    /// Viewport half-separation of a left/right panel at the camera plane.
    /// A panel at world `x = ±1` lands at `vanish_x ± lane`.
    pub lane: f64,
    pub vanish_x: f64,
    pub vanish_y: f64,
    /// Vertical anchor of a panel center when it is at the camera (`depth = 0`).
    pub panel_anchor_y: f64,
    /// Vertical anchor of the path centerline at the camera plane.
    pub road_anchor_y: f64,
    /// Half-width of the path, as a viewport fraction, at the camera plane.
    pub road_half_width: f64,
    /// Angle, in degrees, between adjacent faces of a junction. The viewer
    /// stands in the middle of that room; 60 is one step of a hexagon. Right
    /// is positive. Turning rotates the room by this angle.
    pub branch_angle_deg: f64,
    /// Exponential smoothing rate for travel along a path (1/seconds).
    pub stiffness: f64,
    /// Exponential smoothing rate for turns onto a branch.
    pub turn_stiffness: f64,
    /// Nodes farther ahead than this are fully faded.
    pub max_depth: f64,
    /// Nodes farther behind than this (negative) are fully faded.
    pub behind_limit: f64,
    /// How close, in stops, the camera must be to a junction before a turn begins.
    pub turn_start_distance: f64,
    /// How close, in stops, a junction must be before the UI calls it in range.
    pub ui_range: f64,
    /// Skip easing and commit turns immediately.
    pub reduced_motion: bool,
    /// How long, in milliseconds, to hold the camera on a stop it would
    /// otherwise pass through, so that stop's views sit still. `0` disables
    /// the hold. Reduced motion ignores it.
    pub dwell_ms: f64,
    /// How strongly a finished scroll settles onto a nearby stop.
    /// `0` leaves the target where the gesture ended. `1` settles quickly.
    /// Scrolling itself is not gated by this.
    pub spring: f64,
    /// A finished scroll only settles when it is at most this many stops
    /// from an integer stop. Further away, the target stays where it is.
    pub spring_range: f64,
    /// Milliseconds after the last wheel event before the settle begins.
    pub spring_idle_ms: f64,
    /// Vertical center of the stop you are standing on, as a viewport fraction.
    pub foreground_y: f64,
    /// Width of that stop's front, as a viewport fraction. Views share it by emphasis.
    pub front_width: f64,
    /// Height of a single view.
    pub page_height: f64,
    /// Height of each column in a horizontal stop.
    pub column_height: f64,
    /// Height shared by the rows of a vertical stop.
    pub front_height: f64,
    /// Gap between views of the same stop, as a viewport fraction.
    pub view_gap: f64,
    /// How far, in viewport fractions, a view at the edge of its grid travels
    /// as you walk past the stop. Every view moves outward from the center
    /// in proportion to where it sits, and fades. A single view is the center.
    pub explode: f64,
    /// Retained so existing projection JSON still loads. The junction swing
    /// is [`branch_angle_deg`](Params::branch_angle_deg).
    pub turn_exit: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            focal: 1.25,
            lane: 0.29,
            vanish_x: 0.5,
            vanish_y: 0.38,
            panel_anchor_y: 0.64,
            road_anchor_y: 0.8,
            road_half_width: 0.145,
            branch_angle_deg: 60.0,
            stiffness: 9.0,
            turn_stiffness: 5.5,
            max_depth: 10.0,
            behind_limit: -0.8,
            turn_start_distance: 0.22,
            ui_range: 0.85,
            reduced_motion: false,
            dwell_ms: 280.0,
            spring: 0.3,
            spring_range: 0.2,
            spring_idle_ms: 90.0,
            foreground_y: 0.48,
            front_width: 0.88,
            page_height: 0.58,
            column_height: 0.54,
            front_height: 0.66,
            view_gap: 0.016,
            explode: 0.46,
            turn_exit: 2.6,
        }
    }
}

impl Params {
    pub fn validate(&self) -> Result<(), NavError> {
        let ok = self.focal > 0.2
            && self.lane > 0.02
            && self.lane < 0.8
            && (0.0..=1.0).contains(&self.vanish_x)
            && (0.0..=1.0).contains(&self.vanish_y)
            && (0.0..=1.5).contains(&self.panel_anchor_y)
            && (0.0..=1.5).contains(&self.road_anchor_y)
            && self.road_half_width > 0.0
            && (8.0..=75.0).contains(&self.branch_angle_deg)
            && self.stiffness > 0.0
            && self.turn_stiffness > 0.0
            && self.max_depth > 0.5
            && self.behind_limit < 0.0
            && self.turn_start_distance > 0.0
            && self.ui_range > 0.0
            && self.dwell_ms >= 0.0
            && self.dwell_ms <= 4_000.0
            && (0.0..=1.0).contains(&self.spring)
            && (0.0..=0.5).contains(&self.spring_range)
            && (0.0..=2_000.0).contains(&self.spring_idle_ms)
            && (0.2..=0.95).contains(&self.foreground_y)
            && (0.3..=1.05).contains(&self.front_width)
            && (0.15..=0.9).contains(&self.page_height)
            && (0.12..=0.9).contains(&self.column_height)
            && (0.2..=0.95).contains(&self.front_height)
            && (0.0..=0.08).contains(&self.view_gap)
            && (0.0..=1.2).contains(&self.explode)
            && (0.0..=6.0).contains(&self.turn_exit);
        if ok {
            Ok(())
        } else {
            Err(NavError::InvalidConfig(
                "projection parameters are out of range".into(),
            ))
        }
    }

    pub(crate) fn branch_angle_rad(&self) -> f64 {
        self.branch_angle_deg.to_radians()
    }
}
