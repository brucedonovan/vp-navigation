use vp_navigation::{
    BranchSpec, Graph, NavError, Navigator, Node, Orientation, Side, View,
};

fn gallery() -> Navigator {
    let graph = Graph::builder()
        .root("main")
        .path(
            "main",
            &[
                ("arrival", "Arrival"),
                ("idea", "Idea"),
                ("junction", "Junction"),
                ("motion", "Motion"),
                ("horizon", "Horizon"),
            ],
        )
        .path("annex", &[("alcove", "Alcove"), ("wayback", "Way back")])
        .branch(BranchSpec {
            id: "to-annex".into(),
            label: "The Annex".into(),
            from_path: "main".into(),
            from_node: "junction".into(),
            to_path: "annex".into(),
            side: Side::Right,
        })
        .build()
        .unwrap();
    Navigator::new(graph)
}

fn settle(nav: &mut Navigator) {
    for _ in 0..240 {
        nav.tick(16.0);
    }
}

fn place(nav: &mut Navigator, node: &str) {
    let reduced = nav.params().reduced_motion;
    nav.set_reduced_motion(true);
    nav.focus(node).unwrap();
    nav.set_reduced_motion(reduced);
}

#[test]
fn step_moves_one_stop_and_clamps_at_the_ends() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.step(1);
    assert!((nav.state().position - 1.0).abs() < 1e-9);
    nav.step(1);
    assert!((nav.state().position - 2.0).abs() < 1e-9);
    nav.step(-1);
    assert!((nav.state().position - 1.0).abs() < 1e-9);
    nav.step(-8);
    assert!((nav.state().position - 0.0).abs() < 1e-9);
    nav.step(40);
    assert!((nav.state().position - 4.0).abs() < 1e-9);
}

#[test]
fn move_by_eases_toward_the_target_and_settles() {
    let mut nav = gallery();
    nav.move_by(2.5);
    assert!((nav.state().target - 2.5).abs() < 1e-9);
    assert!(nav.state().position < 0.01);

    let mut previous = nav.state().position;
    for _ in 0..30 {
        nav.tick(16.0);
        let position = nav.state().position;
        assert!(position + 1e-9 >= previous);
        previous = position;
    }
    assert!(previous > 0.4);

    settle(&mut nav);
    assert!(
        (nav.state().position - 2.5).abs() < 1e-2,
        "a long scroll finishes where it was aimed, position {}",
        nav.state().position
    );
    assert!(nav.state().at_rest);
}

#[test]
fn a_finished_scroll_settles_onto_a_nearby_stop() {
    let mut nav = gallery();
    nav.move_by(2.12);
    settle(&mut nav);
    assert!(
        (nav.state().position - 2.0).abs() < 0.02,
        "2.12 is close enough to spring back to 2, position {}",
        nav.state().position
    );

    let mut free = gallery();
    free.set_projection_json(r#"{"spring":0}"#).unwrap();
    free.move_by(2.12);
    settle(&mut free);
    assert!(
        (free.state().position - 2.12).abs() < 1e-2,
        "spring 0 leaves the target, position {}",
        free.state().position
    );
}

#[test]
fn the_camera_holds_briefly_on_each_stop_it_passes() {
    let mut nav = gallery();
    assert!((nav.params().dwell_ms - 280.0).abs() < 1e-9);
    nav.focus("motion").unwrap();

    let mut held_at = None;
    for _ in 0..100 {
        nav.tick(16.0);
        let state = nav.state();
        if state.holding && (state.position - 1.0).abs() < 1e-6 {
            held_at = Some(state.position);
            break;
        }
    }
    let at = held_at.expect("holds on stop 1");
    let frames = (nav.params().dwell_ms * 0.55 / 16.0) as i32;
    for _ in 0..frames {
        nav.tick(16.0);
        let state = nav.state();
        assert!(state.holding);
        assert!((state.position - at).abs() < 1e-6);
        assert!(!state.at_rest);
    }
    for _ in 0..90 {
        nav.tick(16.0);
    }
    assert!(nav.state().position > 1.2, "position {}", nav.state().position);

    let mut glide = gallery();
    glide.set_dwell_ms(0.0);
    glide.focus("horizon").unwrap();
    for _ in 0..40 {
        glide.tick(16.0);
        assert!(!glide.state().holding);
    }
    assert!(glide.state().position > 1.5, "position {}", glide.state().position);

    let mut snapped = gallery();
    snapped.set_reduced_motion(true);
    snapped.move_by(2.4);
    assert!((snapped.state().position - 2.4).abs() < 1e-9);
    assert!(!snapped.state().holding);
}

#[test]
fn wheel_backward_from_the_start_stays_put() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.move_by(-3.0);
    assert!((nav.state().position).abs() < 1e-9);
    assert!(!nav.state().can_return);
}

#[test]
fn focus_targets_a_stop_on_the_active_path() {
    let mut nav = gallery();
    nav.focus("motion").unwrap();
    assert!((nav.state().target - 3.0).abs() < 1e-9);
    assert_eq!(
        nav.focus("alcove").unwrap_err(),
        NavError::NotAvailable("'alcove' is not on the active path".into())
    );
    assert_eq!(
        nav.focus("missing").unwrap_err(),
        NavError::UnknownNode("missing".into())
    );
}

#[test]
fn the_stop_you_are_on_fills_the_front_and_distant_stops_shrink() {
    let mut nav = gallery();
    place(&mut nav, "arrival");
    let frame = nav.project();

    let arrival = frame.nodes.iter().find(|n| n.id == "arrival").unwrap();
    assert!(arrival.active);
    assert!(arrival.depth.abs() < 1e-9);
    assert_eq!(arrival.views.len(), 1);
    let view = &arrival.views[0];
    assert!((view.opacity - 1.0).abs() < 1e-9);
    assert!((view.scale - 1.0).abs() < 1e-6);
    assert!((view.x - frame.vanish_x).abs() < 1e-6);
    assert!(view.width > 0.5, "a single view takes the front, width {}", view.width);
    assert!(view.y < nav.params().panel_anchor_y - 0.08);

    let horizon = frame.nodes.iter().find(|n| n.id == "horizon").unwrap();
    let far = &horizon.views[0];
    assert!(far.scale < view.scale * 0.5);
    assert!(far.width < view.width * 0.5);
    assert!((far.y - frame.vanish_y).abs() < (view.y - frame.vanish_y).abs());
    assert!(far.opacity < view.opacity);
    assert!(far.z_index < view.z_index);
}

#[test]
fn emphasis_splits_the_front_and_views_open_as_you_leave() {
    let mut nav = views_path();
    nav.set_reduced_motion(true);
    nav.focus("pair").unwrap();
    let resting = nav.project();
    let pair = resting.nodes.iter().find(|n| n.id == "pair").unwrap();
    assert_eq!(pair.orientation, Orientation::Horizontal);
    let left = pair.views.iter().find(|v| v.id == "l").unwrap();
    let right = pair.views.iter().find(|v| v.id == "r").unwrap();
    assert!((left.width - right.width).abs() < 1e-6, "50 and 50 share equally");
    assert!(left.x < resting.vanish_x && right.x > resting.vanish_x);
    assert!((left.y - right.y).abs() < 1e-6);
    assert!((left.emphasis - 50.0).abs() < 1e-9);

    let wide = resting.nodes.iter().find(|n| n.id == "burst").unwrap();
    let half = wide.views.iter().find(|v| v.id == "a").unwrap();
    let sliver = wide.views.iter().find(|v| v.id == "c").unwrap();
    assert!(half.width > sliver.width * 2.0);

    nav.move_by(0.4);
    let frame = nav.project();
    let leaving = frame.nodes.iter().find(|n| n.id == "pair").unwrap();
    let left_gone = leaving.views.iter().find(|v| v.id == "l").unwrap();
    let right_gone = leaving.views.iter().find(|v| v.id == "r").unwrap();
    assert!(left_gone.x < left.x - 0.05, "{} -> {}", left.x, left_gone.x);
    assert!(right_gone.x > right.x + 0.05, "{} -> {}", right.x, right_gone.x);
    assert!(left_gone.opacity < 0.92 && left_gone.opacity > 0.15, "{}", left_gone.opacity);
    assert!((left_gone.y - right_gone.y).abs() < 0.04);
}

#[test]
fn a_vertical_pair_splits_up_and_down() {
    let mut nav = views_path();
    nav.set_reduced_motion(true);
    nav.focus("stack").unwrap();
    let resting = nav.project();
    let stack = resting.nodes.iter().find(|n| n.id == "stack").unwrap();
    let top = stack.views.iter().find(|v| v.id == "t").unwrap();
    let bottom = stack.views.iter().find(|v| v.id == "b").unwrap();
    assert!(top.y < bottom.y);
    assert!((top.x - bottom.x).abs() < 1e-6);
    assert!((top.height - bottom.height).abs() < 1e-6);

    nav.move_by(0.45);
    let frame = nav.project();
    let leaving = frame.nodes.iter().find(|n| n.id == "stack").unwrap();
    let top_gone = leaving.views.iter().find(|v| v.id == "t").unwrap();
    let bottom_gone = leaving.views.iter().find(|v| v.id == "b").unwrap();
    assert!(top_gone.y < top.y - 0.04, "{} -> {}", top.y, top_gone.y);
    assert!(bottom_gone.y > bottom.y + 0.04, "{} -> {}", bottom.y, bottom_gone.y);
    assert!((top_gone.x - bottom_gone.x).abs() < 0.04);
}

#[test]
fn three_views_explode_away_from_the_center() {
    let mut nav = views_path();
    nav.set_reduced_motion(true);
    nav.focus("burst").unwrap();
    let resting = nav.project();
    let burst = resting.nodes.iter().find(|n| n.id == "burst").unwrap();
    let left = burst.views.iter().find(|v| v.id == "a").unwrap();
    let middle = burst.views.iter().find(|v| v.id == "m").unwrap();
    let right = burst.views.iter().find(|v| v.id == "c").unwrap();

    nav.move_by(0.45);
    let frame = nav.project();
    let leaving = frame.nodes.iter().find(|n| n.id == "burst").unwrap();
    let left_gone = leaving.views.iter().find(|v| v.id == "a").unwrap();
    let middle_gone = leaving.views.iter().find(|v| v.id == "m").unwrap();
    let right_gone = leaving.views.iter().find(|v| v.id == "c").unwrap();
    let left_travel = left.x - left_gone.x;
    let right_travel = right_gone.x - right.x;
    let middle_travel = (middle_gone.x - middle.x).abs();
    assert!(left_travel > middle_travel + 0.04, "left {left_travel} middle {middle_travel}");
    assert!(right_travel > middle_travel + 0.04, "right {right_travel} middle {middle_travel}");
    assert!(left_gone.opacity < 0.9);
}

#[test]
fn a_single_view_stays_centered_while_it_fades() {
    let mut nav = views_path();
    nav.set_reduced_motion(true);
    let resting = nav.project().nodes.into_iter().find(|n| n.id == "one").unwrap();
    let home = resting.views[0].x;
    nav.move_by(0.4);
    let leaving = nav.project().nodes.into_iter().find(|n| n.id == "one").unwrap();
    let view = &leaving.views[0];
    assert!((view.x - home).abs() < 0.02, "{}", view.x);
    assert!(view.opacity < 0.9 && view.opacity > 0.1, "{}", view.opacity);
}

#[test]
fn stops_behind_the_camera_fade_out() {
    let mut nav = gallery();
    place(&mut nav, "horizon");
    let frame = nav.project();
    let arrival = frame.nodes.iter().find(|n| n.id == "arrival").unwrap();
    let horizon = frame.nodes.iter().find(|n| n.id == "horizon").unwrap();
    assert!(arrival.views.iter().all(|v| v.opacity < 0.05));
    assert!(horizon.views.iter().all(|v| v.opacity > 0.95));
    assert!(horizon.active);
}

fn views_path() -> Navigator {
    let graph = Graph::builder()
        .stops(
            "main",
            vec![
                Node::single("one", "One"),
                Node {
                    id: "pair".into(),
                    label: "Pair".into(),
                    orientation: Orientation::Horizontal,
                    views: vec![
                        View { id: "l".into(), emphasis: 50.0 },
                        View { id: "r".into(), emphasis: 50.0 },
                    ],
                },
                Node {
                    id: "stack".into(),
                    label: "Stack".into(),
                    orientation: Orientation::Vertical,
                    views: vec![
                        View { id: "t".into(), emphasis: 50.0 },
                        View { id: "b".into(), emphasis: 50.0 },
                    ],
                },
                Node {
                    id: "burst".into(),
                    label: "Burst".into(),
                    orientation: Orientation::Horizontal,
                    views: vec![
                        View { id: "a".into(), emphasis: 50.0 },
                        View { id: "m".into(), emphasis: 30.0 },
                        View { id: "c".into(), emphasis: 20.0 },
                    ],
                },
                Node::single("after", "After"),
            ],
        )
        .build()
        .unwrap();
    Navigator::new(graph)
}

#[test]
fn the_path_narrows_toward_the_vanishing_point() {
    let nav = gallery();
    let frame = nav.project();
    let ribbon = frame
        .ribbons
        .iter()
        .find(|r| r.role == vp_navigation::RibbonRole::Active)
        .unwrap();
    assert!(ribbon.samples.len() >= 4);
    let near = ribbon
        .samples
        .iter()
        .min_by(|a, b| a.depth.total_cmp(&b.depth))
        .unwrap();
    let far = ribbon
        .samples
        .iter()
        .max_by(|a, b| a.depth.total_cmp(&b.depth))
        .unwrap();
    let near_width = near.right_x - near.left_x;
    let far_width = far.right_x - far.left_x;
    assert!(near.center_y > far.center_y);
    assert!(near_width > far_width * 1.8);
    assert!((far.center_x - frame.vanish_x).abs() < (near.center_x - frame.vanish_x).abs() + 0.02);
}

#[test]
fn a_branch_preview_leaves_toward_its_side() {
    let mut nav = gallery();
    place(&mut nav, "junction");
    let frame = nav.project();
    let hint = frame.hints.iter().find(|h| h.id == "to-annex").unwrap();
    assert_eq!(hint.side, Side::Right);
    assert!(hint.in_range);
    assert!(
        hint.x > frame.vanish_x + 0.25,
        "the link sits beside the front, x {}",
        hint.x
    );

    assert!(
        frame.nodes.iter().all(|node| node.path_id != "annex"),
        "a branch's views stay off the path you are on"
    );
    let motion = frame
        .markers
        .iter()
        .find(|m| m.node_id == "motion")
        .unwrap();
    assert!(hint.x > motion.x);
}

#[test]
fn taking_a_branch_walks_to_the_junction_then_turns() {
    let mut nav = gallery();
    nav.take_branch("to-annex").unwrap();
    let state = nav.state();
    assert_eq!(state.path_id, "main");
    assert!((state.target - 2.0).abs() < 1e-9);
    assert!(state.pending.is_some());

    settle(&mut nav);
    let state = nav.state();
    assert_eq!(state.path_id, "annex");
    assert!(state.position.abs() < 0.05, "position {}", state.position);
    assert!(state.can_return);
    assert_eq!(state.return_label.as_deref(), Some("Junction"));
    assert!(state.turning.is_none());
}

#[test]
fn a_turn_swings_the_branch_toward_center() {
    let mut nav = gallery();
    place(&mut nav, "junction");
    nav.take_branch("to-annex").unwrap();
    let before = nav.project();
    let x0 = before
        .markers
        .iter()
        .find(|m| m.node_id == "alcove")
        .unwrap()
        .x;
    assert!(nav.state().turning.unwrap().align < 0.001);

    nav.tick(80.0);
    let mid = nav.state().turning.expect("still turning");
    assert!(mid.align > 0.05);
    let after = nav.project();
    let x1 = after
        .markers
        .iter()
        .find(|m| m.node_id == "alcove")
        .unwrap()
        .x;
    assert!(
        x1 < x0,
        "branch marker should swing toward the axis, {x0} -> {x1}"
    );

    let parent_node = after.nodes.iter().find(|n| n.id == "junction").unwrap();
    let parent_after = parent_node.views[0].clone();
    let step = 60.0;
    assert!(
        parent_node.hinge_x < parent_node.x - 0.15,
        "the face ahead hinges on the left, hinge {} center {}",
        parent_node.hinge_x,
        parent_node.x
    );
    assert!(
        parent_node.yaw > 8.0,
        "the face ahead opens outward to the left, yaw {}",
        parent_node.yaw
    );
    assert!(
        parent_after.opacity > 0.85,
        "the path you leave stays solid while it opens ({})",
        parent_after.opacity
    );
    assert!(
        (parent_node.yaw - parent_after.rotate_y).abs() < 0.01,
        "the stop opens as one face"
    );

    let hidden = before.nodes.iter().find(|n| n.id == "alcove").unwrap();
    let arriving = after.nodes.iter().find(|n| n.id == "alcove").unwrap();
    assert!(
        hidden.hinge_x > hidden.x + 0.15,
        "the alley hinges on the right, hinge {} center {}",
        hidden.hinge_x,
        hidden.x
    );
    assert!(
        hidden.yaw < -(step - 1.0),
        "the alley starts folded at the right, yaw {} step {}",
        hidden.yaw,
        step
    );
    assert!(
        arriving.yaw > hidden.yaw + 4.0,
        "the alley opens in from the right, {} -> {}",
        hidden.yaw,
        arriving.yaw
    );
    assert!(
        arriving.views[0].opacity > 0.85,
        "the alley stays solid while it opens ({})",
        arriving.views[0].opacity
    );
    assert!(
        (arriving.yaw - arriving.views[0].rotate_y).abs() < 0.01,
        "the alley opens as one face"
    );
}

#[test]
fn commands_are_rejected_while_turning() {
    let mut nav = gallery();
    place(&mut nav, "junction");
    nav.take_branch("to-annex").unwrap();
    let target = nav.state().target;
    nav.move_by(1.0);
    nav.step(1);
    assert!((nav.state().target - target).abs() < 1e-9);
    assert_eq!(nav.focus("idea").unwrap_err(), NavError::Busy);
    assert_eq!(nav.return_to_parent().unwrap_err(), NavError::Busy);
}

#[test]
fn reduced_motion_snaps_travel_and_branches() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.move_by(1.7);
    assert!((nav.state().position - 1.7).abs() < 1e-9);

    nav.take_branch("to-annex").unwrap();
    let state = nav.state();
    assert_eq!(state.path_id, "annex");
    assert!(state.position.abs() < 1e-9);
    assert!(state.turning.is_none());
    assert!(nav.project().reduced_motion);

    nav.return_to_parent().unwrap();
    let state = nav.state();
    assert_eq!(state.path_id, "main");
    assert!((state.position - 2.0).abs() < 1e-9);
}

#[test]
fn walking_backward_off_a_branch_returns_to_the_junction() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.take_branch("to-annex").unwrap();
    nav.set_reduced_motion(false);
    nav.move_by(-1.0);
    settle(&mut nav);
    let state = nav.state();
    assert_eq!(state.path_id, "main");
    assert!(
        (state.position - 2.0).abs() < 0.05,
        "position {}",
        state.position
    );
}

#[test]
fn stepping_back_from_the_first_branch_stop_returns() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.take_branch("to-annex").unwrap();
    nav.step(1);
    assert!((nav.state().position - 1.0).abs() < 1e-9);
    nav.step(-1);
    assert_eq!(nav.state().path_id, "annex");
    assert!(nav.state().position.abs() < 1e-9);
    nav.step(-1);
    assert_eq!(nav.state().path_id, "main");
    assert!((nav.state().position - 2.0).abs() < 1e-9);
}

#[test]
fn activate_enters_a_branch_stop_and_can_leave_again() {
    let mut nav = gallery();
    nav.set_reduced_motion(true);
    nav.activate("wayback").unwrap();
    let state = nav.state();
    assert_eq!(state.path_id, "annex");
    assert!((state.position - 1.0).abs() < 1e-9);
    assert_eq!(state.current_node, "wayback");

    nav.activate("idea").unwrap();
    let state = nav.state();
    assert_eq!(state.path_id, "main");
    assert!((state.position - 1.0).abs() < 1e-9);
}

#[test]
fn return_on_the_root_and_unknown_branch_fail() {
    let mut nav = gallery();
    assert_eq!(nav.return_to_parent().unwrap_err(), NavError::NoParent);
    assert_eq!(
        nav.take_branch("nope").unwrap_err(),
        NavError::UnknownBranch("nope".into())
    );
}

#[test]
fn graph_rejects_broken_links() {
    let duplicate = Graph::builder()
        .path("main", &[("a", "A"), ("a", "Again")])
        .build();
    assert!(matches!(duplicate, Err(NavError::InvalidConfig(_))));

    let missing = Graph::builder()
        .root("main")
        .path("main", &[("a", "A")])
        .path("side", &[("b", "B")])
        .branch(BranchSpec {
            id: "fork".into(),
            label: "Fork".into(),
            from_path: "main".into(),
            from_node: "missing".into(),
            to_path: "side".into(),
            side: Side::Left,
        })
        .build();
    assert!(matches!(missing, Err(NavError::InvalidConfig(_))));

    let empty = Graph::builder().path("main", &[]).build();
    assert!(matches!(empty, Err(NavError::InvalidConfig(_))));

    let emphasis = Graph::builder()
        .stops(
            "main",
            vec![Node {
                id: "a".into(),
                label: "A".into(),
                orientation: Orientation::Horizontal,
                views: vec![View { id: "a".into(), emphasis: 0.0 }],
            }],
        )
        .build();
    assert!(matches!(emphasis, Err(NavError::InvalidConfig(_))));
}

#[test]
fn config_json_round_trips_into_a_camel_case_frame() {
    let json = r#"{
        "rootPath": "main",
        "paths": [
            { "id": "main", "nodes": [
                { "id": "a", "label": "A" },
                { "id": "b", "label": "B" }
            ]},
            { "id": "side", "nodes": [ { "id": "c", "label": "C" } ] }
        ],
        "branches": [{
            "id": "fork",
            "label": "Side",
            "fromPath": "main",
            "fromNode": "b",
            "toPath": "side",
            "side": "left"
        }],
        "projection": { "focal": 1.5, "reducedMotion": true }
    }"#;
    let mut nav = Navigator::from_config_json(json).unwrap();
    nav.focus("b").unwrap();
    assert!((nav.state().position - 1.0).abs() < 1e-9);
    assert!((nav.params().focal - 1.5).abs() < 1e-9);

    let frame: serde_json::Value = serde_json::from_str(&nav.frame_json()).unwrap();
    assert!(frame.get("pathId").is_some());
    assert!(frame.get("vanishX").is_some());
    assert!(frame["nodes"][0].get("pathId").is_some());
    assert!(frame["nodes"][0]["views"][0].get("rotateY").is_some());
    assert!(frame["nodes"][0]["views"][0].get("width").is_some());
    assert!(frame["nodes"][0]["views"][0].get("zIndex").is_some());

    let state: serde_json::Value = serde_json::from_str(&nav.state_json()).unwrap();
    assert_eq!(state["currentNode"], "b");
    assert_eq!(state["availableBranches"][0]["toPath"], "side");
    assert_eq!(state["availableBranches"][0]["side"], "left");

    let bad = Navigator::from_config_json("{").unwrap_err();
    assert!(matches!(bad, NavError::InvalidConfig(_)));
}

#[test]
fn a_flick_advances_exactly_one_stop() {
    let mut nav = gallery();
    nav.scroll(0.0, 3.0);
    nav.scroll(40.0, 1.4);
    nav.scroll(90.0, 0.5);
    nav.scroll(400.0, 0.04);
    assert!(
        (nav.state().target - 1.0).abs() < 1e-9,
        "a flick stops at the next stop, target {}",
        nav.state().target
    );
    settle(&mut nav);
    assert!((nav.state().position - 1.0).abs() < 0.02);
}

#[test]
fn a_sustained_scroll_travels_through_stops() {
    let mut nav = gallery();
    let mut t = 0.0;
    while t <= 640.0 {
        nav.scroll(t, 0.12);
        nav.tick(16.0);
        t += 16.0;
    }
    assert!(
        nav.state().target > 2.0,
        "holding the scroll walks past the next stop, target {}",
        nav.state().target
    );
}
