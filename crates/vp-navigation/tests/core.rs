use vp_navigation::{BranchSpec, Graph, NavError, Navigator, Side};

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
    assert!((nav.state().position - 2.5).abs() < 1e-3);
    assert!(nav.state().at_rest);
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
fn active_stop_is_full_size_and_distant_stops_converge() {
    let mut nav = gallery();
    place(&mut nav, "arrival");
    let frame = nav.project();
    let vanish_x = frame.vanish_x;
    let vanish_y = frame.vanish_y;
    let lane = nav.params().lane;

    let arrival = frame.nodes.iter().find(|n| n.id == "arrival").unwrap();
    assert!(arrival.active);
    assert!(arrival.depth.abs() < 1e-9);
    let left = arrival
        .panels
        .iter()
        .find(|p| p.side == Side::Left)
        .unwrap();
    let right = arrival
        .panels
        .iter()
        .find(|p| p.side == Side::Right)
        .unwrap();
    assert!((left.scale - 1.0).abs() < 1e-9);
    assert!((right.scale - 1.0).abs() < 1e-9);
    assert!((left.opacity - 1.0).abs() < 1e-9);
    assert!((left.x - (vanish_x - lane)).abs() < 1e-9);
    assert!((right.x - (vanish_x + lane)).abs() < 1e-9);
    assert!(left.x < vanish_x && right.x > vanish_x);
    assert!(left.z_index == right.z_index);

    let horizon = frame.nodes.iter().find(|n| n.id == "horizon").unwrap();
    let far = horizon
        .panels
        .iter()
        .find(|p| p.side == Side::Left)
        .unwrap();
    assert!(far.scale < left.scale * 0.5);
    assert!((far.x - vanish_x).abs() < (left.x - vanish_x).abs());
    assert!((far.y - vanish_y).abs() < (left.y - vanish_y).abs());
    assert!(far.opacity < left.opacity);
    assert!(far.z_index < left.z_index);
}

#[test]
fn stops_behind_the_camera_fade_out() {
    let mut nav = gallery();
    place(&mut nav, "horizon");
    let frame = nav.project();
    let arrival = frame.nodes.iter().find(|n| n.id == "arrival").unwrap();
    let horizon = frame.nodes.iter().find(|n| n.id == "horizon").unwrap();
    assert!(arrival.panels.iter().all(|p| p.opacity < 0.05));
    assert!(horizon.panels.iter().all(|p| p.opacity > 0.95));
    assert!(horizon.active);
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
    assert!(hint.x > frame.vanish_x);

    let alcove = frame
        .markers
        .iter()
        .find(|m| m.node_id == "alcove")
        .unwrap();
    let motion = frame
        .markers
        .iter()
        .find(|m| m.node_id == "motion")
        .unwrap();
    assert!(alcove.x > frame.vanish_x + 0.04);
    assert!(alcove.x > motion.x);
    assert!(alcove.scale < 1.0);
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
    assert!(frame["nodes"][0]["panels"][0].get("rotateY").is_some());
    assert!(frame["nodes"][0]["panels"][0].get("zIndex").is_some());

    let state: serde_json::Value = serde_json::from_str(&nav.state_json()).unwrap();
    assert_eq!(state["currentNode"], "b");
    assert_eq!(state["availableBranches"][0]["toPath"], "side");
    assert_eq!(state["availableBranches"][0]["side"], "left");

    let bad = Navigator::from_config_json("{").unwrap_err();
    assert!(matches!(bad, NavError::InvalidConfig(_)));
}
