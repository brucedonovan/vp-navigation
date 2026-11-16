use serde::Deserialize;

use crate::error::NavError;
use crate::graph::{BranchSpec, Graph, GraphBuilder, Node, Orientation, Side, View};
use crate::params::Params;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigFile {
    root_path: String,
    paths: Vec<PathConfig>,
    #[serde(default)]
    branches: Vec<BranchConfig>,
    #[serde(default)]
    projection: Params,
}

#[derive(Debug, Deserialize)]
struct PathConfig {
    id: String,
    nodes: Vec<NodeConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeConfig {
    id: String,
    label: String,
    #[serde(default)]
    orientation: Orientation,
    #[serde(default)]
    views: Vec<ViewConfig>,
}

#[derive(Debug, Deserialize)]
struct ViewConfig {
    id: String,
    emphasis: f64,
}

impl NodeConfig {
    fn into_node(self) -> Node {
        if self.views.is_empty() {
            return Node::single(self.id, self.label);
        }
        Node {
            id: self.id,
            label: self.label,
            orientation: self.orientation,
            views: self
                .views
                .into_iter()
                .map(|view| View {
                    id: view.id,
                    emphasis: view.emphasis,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchConfig {
    id: String,
    label: String,
    from_path: String,
    from_node: String,
    to_path: String,
    side: Side,
}

pub fn parse_config(json: &str) -> Result<(Graph, Params), NavError> {
    let file: ConfigFile =
        serde_json::from_str(json).map_err(|err| NavError::InvalidConfig(err.to_string()))?;
    file.projection.validate()?;

    let mut builder = GraphBuilder::default().root(file.root_path);
    for path in file.paths {
        let nodes = path.nodes.into_iter().map(NodeConfig::into_node).collect();
        builder = builder.stops(path.id, nodes);
    }
    for branch in file.branches {
        builder = builder.branch(BranchSpec {
            id: branch.id,
            label: branch.label,
            from_path: branch.from_path,
            from_node: branch.from_node,
            to_path: branch.to_path,
            side: branch.side,
        });
    }
    Ok((builder.build()?, file.projection))
}
