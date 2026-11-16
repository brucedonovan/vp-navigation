use serde::Deserialize;

use crate::error::NavError;
use crate::graph::{BranchSpec, Graph, GraphBuilder, Side};
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
struct NodeConfig {
    id: String,
    label: String,
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
        let nodes = path
            .nodes
            .into_iter()
            .map(|node| (node.id, node.label))
            .collect();
        builder = builder.path_owned(path.id, nodes);
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
