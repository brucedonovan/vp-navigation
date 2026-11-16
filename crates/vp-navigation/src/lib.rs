//! Vanishing-point navigation.
//!
//! The core owns a graph of paths, the traveler's position on that graph, and
//! the perspective projection that turns each stop into a screen transform.
//! It does not render. A DOM adapter, a React binding, or a game engine
//! consumes [`Frame`] and applies it however it likes.
//!
//! Build a graph with [`Graph::builder`](graph::Graph::builder) or, from a
//! host that only has JSON, [`Navigator::from_config_json`]. Drive it with
//! [`Navigator::move_by`], [`Navigator::step`], [`Navigator::take_branch`],
//! and [`Navigator::tick`], then read [`Navigator::project`].

mod config;
mod error;
mod graph;
mod navigate;
mod params;
mod project;

pub use error::NavError;
pub use graph::{Branch, BranchSpec, Graph, GraphBuilder, Node, Orientation, Path, Side, View};
pub use navigate::{
    NavState, Navigator, PendingView, StateBranch, StateNode, TurnDirection, TurnView,
};
pub use params::Params;
pub use project::{
    BranchHint, Frame, ProjectedMarker, ProjectedNode, ProjectedView, Ribbon, RibbonRole,
    RibbonSample,
};
