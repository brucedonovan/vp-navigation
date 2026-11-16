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
    /// Angle, in degrees, at which a branch leaves its parent. Right is positive.
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
            branch_angle_deg: 34.0,
            stiffness: 9.0,
            turn_stiffness: 5.5,
            max_depth: 10.0,
            behind_limit: -0.8,
            turn_start_distance: 0.22,
            ui_range: 0.85,
            reduced_motion: false,
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
            && self.ui_range > 0.0;
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
