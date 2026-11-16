use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::NavError;

/// Which side of the parent path a branch departs toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn sign(self) -> f64 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
        }
    }
}

/// A stop on a path. Content (HTML, meshes, textures) lives in the renderer;
/// the core only knows the id and a label for accessibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NodeLoc {
    pub label: String,
    pub path_id: String,
    pub index: usize,
}

/// Where a child path rejoins its parent when the traveler walks back off it.
#[derive(Debug, Clone)]
pub struct Parent {
    pub path_id: String,
    pub node_id: String,
    pub node_index: usize,
    pub branch_id: String,
}

#[derive(Debug, Clone)]
pub struct Path {
    pub id: String,
    pub nodes: Vec<Node>,
    pub parent: Option<Parent>,
}

/// A fork from one stop onto another path.
#[derive(Debug, Clone)]
pub struct Branch {
    pub id: String,
    pub label: String,
    pub from_path: String,
    pub from_node: String,
    pub from_index: usize,
    pub to_path: String,
    pub side: Side,
}

impl Branch {
    pub(crate) fn signed_angle(&self, radians: f64) -> f64 {
        self.side.sign() * radians
    }
}

/// Ordered paths, the stops on them, and the branches that connect them.
#[derive(Debug, Clone)]
pub struct Graph {
    root: String,
    paths: Vec<Path>,
    path_index: HashMap<String, usize>,
    nodes: HashMap<String, NodeLoc>,
    branches: Vec<Branch>,
    branch_index: HashMap<String, usize>,
}

impl Graph {
    pub fn builder() -> GraphBuilder {
        GraphBuilder::default()
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn path(&self, id: &str) -> Option<&Path> {
        self.path_index.get(id).map(|&i| &self.paths[i])
    }

    pub fn node(&self, id: &str) -> Option<&NodeLoc> {
        self.nodes.get(id)
    }

    pub fn branch(&self, id: &str) -> Option<&Branch> {
        self.branch_index.get(id).map(|&i| &self.branches[i])
    }

    pub fn branches_from<'a>(&'a self, path_id: &'a str) -> impl Iterator<Item = &'a Branch> + 'a {
        self.branches.iter().filter(move |b| b.from_path == path_id)
    }

    /// The branch whose destination is `path_id`, if this path is a child.
    pub fn branch_into(&self, path_id: &str) -> Option<&Branch> {
        self.branches.iter().find(|b| b.to_path == path_id)
    }

    pub fn parent_of(&self, path_id: &str) -> Option<&Parent> {
        self.path(path_id).and_then(|p| p.parent.as_ref())
    }
}

/// Description of a branch before the graph checks that its endpoints exist.
#[derive(Debug, Clone)]
pub struct BranchSpec {
    pub id: String,
    pub label: String,
    pub from_path: String,
    pub from_node: String,
    pub to_path: String,
    pub side: Side,
}

#[derive(Debug, Default)]
pub struct GraphBuilder {
    root: Option<String>,
    paths: Vec<(String, Vec<Node>)>,
    branches: Vec<BranchSpec>,
}

impl GraphBuilder {
    pub fn root(mut self, id: impl Into<String>) -> Self {
        self.root = Some(id.into());
        self
    }

    /// Add a path. `nodes` are `(id, label)` pairs in travel order.
    pub fn path(self, id: impl Into<String>, nodes: &[(&str, &str)]) -> Self {
        let owned = nodes
            .iter()
            .map(|(id, label)| ((*id).to_string(), (*label).to_string()))
            .collect();
        self.path_owned(id, owned)
    }

    pub fn path_owned(mut self, id: impl Into<String>, nodes: Vec<(String, String)>) -> Self {
        let nodes = nodes
            .into_iter()
            .map(|(id, label)| Node { id, label })
            .collect();
        self.paths.push((id.into(), nodes));
        self
    }

    pub fn branch(mut self, spec: BranchSpec) -> Self {
        self.branches.push(spec);
        self
    }

    pub fn build(self) -> Result<Graph, NavError> {
        if self.paths.is_empty() {
            return Err(NavError::InvalidConfig("graph has no paths".into()));
        }

        let mut path_index = HashMap::new();
        let mut nodes = HashMap::new();
        let mut paths = Vec::with_capacity(self.paths.len());

        for (id, path_nodes) in self.paths {
            if id.is_empty() {
                return Err(NavError::InvalidConfig("path id is empty".into()));
            }
            if path_index.contains_key(&id) {
                return Err(NavError::InvalidConfig(format!("duplicate path '{id}'")));
            }
            if path_nodes.is_empty() {
                return Err(NavError::InvalidConfig(format!("path '{id}' has no stops")));
            }
            for (index, node) in path_nodes.iter().enumerate() {
                if node.id.is_empty() {
                    return Err(NavError::InvalidConfig(format!(
                        "path '{id}' has a stop with an empty id"
                    )));
                }
                if nodes.contains_key(&node.id) {
                    return Err(NavError::InvalidConfig(format!(
                        "duplicate node '{}'",
                        node.id
                    )));
                }
                nodes.insert(
                    node.id.clone(),
                    NodeLoc {
                        label: node.label.clone(),
                        path_id: id.clone(),
                        index,
                    },
                );
            }
            path_index.insert(id.clone(), paths.len());
            paths.push(Path {
                id,
                nodes: path_nodes,
                parent: None,
            });
        }

        let root = if let Some(root) = self.root {
            if !path_index.contains_key(&root) {
                return Err(NavError::InvalidConfig(format!(
                    "root path '{root}' does not exist"
                )));
            }
            root
        } else if paths.len() == 1 {
            paths[0].id.clone()
        } else {
            return Err(NavError::InvalidConfig(
                "rootPath is required when the graph has more than one path".into(),
            ));
        };

        let mut branch_index = HashMap::new();
        let mut branches = Vec::with_capacity(self.branches.len());
        let mut child_claimed: HashSet<String> = HashSet::new();

        for spec in self.branches {
            if spec.id.is_empty() {
                return Err(NavError::InvalidConfig("branch id is empty".into()));
            }
            if branch_index.contains_key(&spec.id) {
                return Err(NavError::InvalidConfig(format!(
                    "duplicate branch '{}'",
                    spec.id
                )));
            }
            if spec.from_path == spec.to_path {
                return Err(NavError::InvalidConfig(format!(
                    "branch '{}' starts and ends on the same path",
                    spec.id
                )));
            }
            if !path_index.contains_key(&spec.from_path) {
                return Err(NavError::InvalidConfig(format!(
                    "branch '{}' leaves unknown path '{}'",
                    spec.id, spec.from_path
                )));
            }
            if !path_index.contains_key(&spec.to_path) {
                return Err(NavError::InvalidConfig(format!(
                    "branch '{}' enters unknown path '{}'",
                    spec.id, spec.to_path
                )));
            }
            if spec.to_path == root {
                return Err(NavError::InvalidConfig(format!(
                    "branch '{}' cannot target the root path",
                    spec.id
                )));
            }
            let loc = nodes.get(&spec.from_node).ok_or_else(|| {
                NavError::InvalidConfig(format!(
                    "branch '{}' leaves unknown node '{}'",
                    spec.id, spec.from_node
                ))
            })?;
            if loc.path_id != spec.from_path {
                return Err(NavError::InvalidConfig(format!(
                    "node '{}' is not on path '{}'",
                    spec.from_node, spec.from_path
                )));
            }
            if !child_claimed.insert(spec.to_path.clone()) {
                return Err(NavError::InvalidConfig(format!(
                    "path '{}' is the destination of more than one branch",
                    spec.to_path
                )));
            }

            let parent = Parent {
                path_id: spec.from_path.clone(),
                node_id: spec.from_node.clone(),
                node_index: loc.index,
                branch_id: spec.id.clone(),
            };
            let child_i = path_index[&spec.to_path];
            paths[child_i].parent = Some(parent);

            branch_index.insert(spec.id.clone(), branches.len());
            branches.push(Branch {
                id: spec.id,
                label: spec.label,
                from_path: spec.from_path,
                from_node: spec.from_node,
                from_index: loc.index,
                to_path: spec.to_path,
                side: spec.side,
            });
        }

        let graph = Graph {
            root,
            paths,
            path_index,
            nodes,
            branches,
            branch_index,
        };
        graph.assert_no_cycle()?;
        Ok(graph)
    }
}

impl Graph {
    fn assert_no_cycle(&self) -> Result<(), NavError> {
        for path in &self.paths {
            let mut seen = HashSet::new();
            let mut cursor = Some(path.id.as_str());
            while let Some(id) = cursor {
                if !seen.insert(id.to_string()) {
                    return Err(NavError::InvalidConfig(format!(
                        "branch cycle involving path '{id}'"
                    )));
                }
                cursor = self.parent_of(id).map(|p| p.path_id.as_str());
            }
        }
        Ok(())
    }
}
