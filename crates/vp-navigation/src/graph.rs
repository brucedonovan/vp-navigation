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

/// How a stop's views share the front when the stop does not name a [`Grid`].
///
/// `Horizontal` is one row of columns. `Vertical` is one column of rows.
/// Either way, walking past the stop throws the views outward from the center
/// and fades them. A [`Grid`] uses that same departure for any other arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    /// One row. Views sit side by side.
    #[default]
    Horizontal,
    /// One column. Views sit one above the other.
    Vertical,
}

/// Columns and rows for a stop. Absent fields are inferred from the views.
///
/// `column` / `row` on a view are 1-based. Views that omit both flow left to
/// right, then down, into the next free cell. Sizes are viewport fractions,
/// so the same grid fills the front at any stage size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub columns: Option<u32>,
    pub rows: Option<u32>,
}

impl Grid {
    /// Wrap views into this many columns. Rows grow to fit.
    pub fn columns(columns: u32) -> Self {
        Self {
            columns: Some(columns),
            rows: None,
        }
    }

    /// A fixed grid. Views that do not fit are rejected.
    pub fn size(columns: u32, rows: u32) -> Self {
        Self {
            columns: Some(columns),
            rows: Some(rows),
        }
    }
}

/// One cell of a stop. `emphasis` is a percentage share of the front.
/// Shares are normalized, so 50 and 50 split a row in half.
///
/// `column` and `row` are 1-based and must be set together. Omit both and the
/// view flows into the next free cell. `col_span` and `row_span` default to 1.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub id: String,
    pub emphasis: f64,
    pub column: Option<u32>,
    pub row: Option<u32>,
    pub col_span: u32,
    pub row_span: u32,
}

impl View {
    pub fn new(id: impl Into<String>, emphasis: f64) -> Self {
        Self {
            id: id.into(),
            emphasis,
            column: None,
            row: None,
            col_span: 1,
            row_span: 1,
        }
    }

    /// Pin this view to a cell. `column` and `row` are 1-based.
    pub fn at(mut self, column: u32, row: u32) -> Self {
        self.column = Some(column);
        self.row = Some(row);
        self
    }

    /// Cover more than one cell. Spans start at 1.
    pub fn span(mut self, columns: u32, rows: u32) -> Self {
        self.col_span = columns;
        self.row_span = rows;
        self
    }
}

/// A stop on a path. Markup stays in the renderer. The core knows which views
/// share the front, and which grid they sit in.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub orientation: Orientation,
    /// When set, the stop is a grid of `columns` by `rows` instead of a single
    /// row or column. Cells may span, and unplaced views flow into the holes.
    pub grid: Option<Grid>,
    pub views: Vec<View>,
}

impl Node {
    /// A stop with one view that takes the whole front.
    pub fn single(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: label.into(),
            orientation: Orientation::Horizontal,
            grid: None,
            views: vec![View::new(id.clone(), 100.0)],
            id,
        }
    }
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

    pub fn path_owned(self, id: impl Into<String>, nodes: Vec<(String, String)>) -> Self {
        let nodes = nodes
            .into_iter()
            .map(|(id, label)| Node::single(id, label))
            .collect();
        self.stops(id, nodes)
    }

    /// Add a path whose stops already name their views.
    pub fn stops(mut self, id: impl Into<String>, nodes: Vec<Node>) -> Self {
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
                validate_views(&id, node)?;
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

/// A view after it has been given a cell. Columns and rows are 0-based.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placed {
    pub id: String,
    pub emphasis: f64,
    pub column: u32,
    pub row: u32,
    pub col_span: u32,
    pub row_span: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placement {
    pub columns: u32,
    pub rows: u32,
    pub cells: Vec<Placed>,
}

/// Resolve a stop into a grid. A stop with no [`Grid`] and no placed or
/// spanning views is one row, or one column when its orientation is vertical.
pub(crate) fn place_views(node: &Node) -> Result<Placement, NavError> {
    for view in &node.views {
        if view.col_span == 0 || view.row_span == 0 {
            return Err(NavError::InvalidConfig(format!(
                "view '{}' on stop '{}' has a span of 0",
                view.id, node.id
            )));
        }
        match (view.column, view.row) {
            (None, None) | (Some(_), Some(_)) => {}
            _ => {
                return Err(NavError::InvalidConfig(format!(
                    "view '{}' on stop '{}' must set column and row together",
                    view.id, node.id
                )));
            }
        }
        if matches!(view.column, Some(0)) || matches!(view.row, Some(0)) {
            return Err(NavError::InvalidConfig(format!(
                "view '{}' on stop '{}' uses a 1-based column and row",
                view.id, node.id
            )));
        }
    }

    let custom = node.grid.is_some()
        || node
            .views
            .iter()
            .any(|view| view.column.is_some() || view.col_span != 1 || view.row_span != 1);
    if !custom {
        return Ok(strip(node));
    }

    let declared_cols = node.grid.as_ref().and_then(|grid| grid.columns);
    let declared_rows = node.grid.as_ref().and_then(|grid| grid.rows);
    if declared_cols == Some(0) || declared_rows == Some(0) {
        return Err(NavError::InvalidConfig(format!(
            "stop '{}' has a grid of 0 columns or rows",
            node.id
        )));
    }

    let mut columns = declared_cols.unwrap_or(0);
    let mut rows = declared_rows.unwrap_or(0);
    let mut drafts = Vec::with_capacity(node.views.len());
    for view in &node.views {
        let column = view.column.map(|column| column - 1);
        let row = view.row.map(|row| row - 1);
        if let (Some(column), Some(row)) = (column, row) {
            let end_column = column + view.col_span;
            let end_row = row + view.row_span;
            if let Some(max_columns) = declared_cols {
                if end_column > max_columns {
                    return Err(NavError::InvalidConfig(format!(
                        "view '{}' on stop '{}' ends past column {max_columns}",
                        view.id, node.id
                    )));
                }
            } else {
                columns = columns.max(end_column);
            }
            if let Some(max_rows) = declared_rows {
                if end_row > max_rows {
                    return Err(NavError::InvalidConfig(format!(
                        "view '{}' on stop '{}' ends past row {max_rows}",
                        view.id, node.id
                    )));
                }
            } else {
                rows = rows.max(end_row);
            }
        }
        drafts.push(Draft {
            id: view.id.clone(),
            emphasis: view.emphasis,
            column,
            row,
            col_span: view.col_span,
            row_span: view.row_span,
        });
    }

    if declared_cols.is_none() {
        columns = infer_columns(node, &drafts, declared_rows);
    }
    if columns == 0 {
        columns = 1;
    }
    if rows == 0 {
        rows = 1;
    }

    arrange(node, columns, declared_rows, rows, drafts)
}

#[derive(Clone)]
struct Draft {
    id: String,
    emphasis: f64,
    column: Option<u32>,
    row: Option<u32>,
    col_span: u32,
    row_span: u32,
}

fn strip(node: &Node) -> Placement {
    let count = node.views.len() as u32;
    let vertical = node.orientation == Orientation::Vertical && count > 1;
    let (columns, rows) = if count <= 1 {
        (1, 1)
    } else if vertical {
        (1, count)
    } else {
        (count, 1)
    };
    let cells = node
        .views
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let index = index as u32;
            let (column, row) = if vertical { (0, index) } else { (index, 0) };
            Placed {
                id: view.id.clone(),
                emphasis: view.emphasis,
                column,
                row,
                col_span: 1,
                row_span: 1,
            }
        })
        .collect();
    Placement {
        columns,
        rows,
        cells,
    }
}

fn infer_columns(node: &Node, drafts: &[Draft], declared_rows: Option<u32>) -> u32 {
    let max_span = drafts.iter().map(|draft| draft.col_span).max().unwrap_or(1);
    let explicit = drafts
        .iter()
        .filter_map(|draft| draft.column.map(|column| column + draft.col_span))
        .max()
        .unwrap_or(0);
    if let Some(row_count) = declared_rows {
        let mut columns = explicit.max(max_span).max(1);
        let cap = drafts.len() as u32 * max_span + explicit + 1;
        while columns <= cap {
            if arrange(node, columns, Some(row_count), row_count, drafts.to_vec()).is_ok() {
                return columns;
            }
            columns += 1;
        }
        return explicit.max(max_span).max(1);
    }
    let stacks = node.orientation == Orientation::Vertical && explicit <= 1 && max_span == 1;
    if stacks {
        return 1;
    }
    let flowed: u32 = drafts
        .iter()
        .filter(|draft| draft.column.is_none())
        .map(|draft| draft.col_span)
        .sum();
    explicit.max(flowed).max(max_span).max(1)
}

fn arrange(
    node: &Node,
    columns: u32,
    declared_rows: Option<u32>,
    mut rows: u32,
    mut drafts: Vec<Draft>,
) -> Result<Placement, NavError> {
    let mut occupied: HashMap<(u32, u32), String> = HashMap::new();
    for draft in drafts.iter().filter(|draft| draft.column.is_some()) {
        claim(
            &mut occupied,
            &node.id,
            &draft.id,
            draft.column.unwrap_or(0),
            draft.row.unwrap_or(0),
            draft.col_span,
            draft.row_span,
        )?;
    }

    let max_span = drafts.iter().map(|draft| draft.row_span).max().unwrap_or(1);
    let search_rows = (drafts.len() as u32)
        .saturating_mul(max_span)
        .saturating_add(rows);
    for draft in drafts.iter_mut().filter(|draft| draft.column.is_none()) {
        let found = find_cell(
            columns,
            declared_rows.map(|_| rows),
            draft.col_span,
            draft.row_span,
            &occupied,
            search_rows,
        );
        let Some((column, row)) = found else {
            return Err(NavError::InvalidConfig(format!(
                "view '{}' does not fit the grid on stop '{}'",
                draft.id, node.id
            )));
        };
        if declared_rows.is_none() {
            rows = rows.max(row + draft.row_span);
        }
        claim(
            &mut occupied,
            &node.id,
            &draft.id,
            column,
            row,
            draft.col_span,
            draft.row_span,
        )?;
        draft.column = Some(column);
        draft.row = Some(row);
    }

    if let Some(declared) = declared_rows {
        rows = declared;
    }
    let cells = drafts
        .into_iter()
        .map(|draft| Placed {
            id: draft.id,
            emphasis: draft.emphasis,
            column: draft.column.unwrap_or(0),
            row: draft.row.unwrap_or(0),
            col_span: draft.col_span,
            row_span: draft.row_span,
        })
        .collect();
    Ok(Placement {
        columns,
        rows,
        cells,
    })
}

fn find_cell(
    columns: u32,
    rows_limit: Option<u32>,
    col_span: u32,
    row_span: u32,
    occupied: &HashMap<(u32, u32), String>,
    search_rows: u32,
) -> Option<(u32, u32)> {
    if columns == 0 || col_span > columns {
        return None;
    }
    let last_row = rows_limit.unwrap_or(search_rows.max(row_span));
    if row_span > last_row {
        return None;
    }
    for row in 0..=last_row - row_span {
        for column in 0..=columns - col_span {
            let free = (0..row_span)
                .all(|dr| (0..col_span).all(|dc| !occupied.contains_key(&(column + dc, row + dr))));
            if free {
                return Some((column, row));
            }
        }
    }
    None
}

fn claim(
    occupied: &mut HashMap<(u32, u32), String>,
    node_id: &str,
    id: &str,
    column: u32,
    row: u32,
    col_span: u32,
    row_span: u32,
) -> Result<(), NavError> {
    for y in row..row + row_span {
        for x in column..column + col_span {
            if let Some(other) = occupied.get(&(x, y)) {
                return Err(NavError::InvalidConfig(format!(
                    "stop '{node_id}' overlaps views '{other}' and '{id}'"
                )));
            }
        }
    }
    for y in row..row + row_span {
        for x in column..column + col_span {
            occupied.insert((x, y), id.to_string());
        }
    }
    Ok(())
}

fn validate_views(path_id: &str, node: &Node) -> Result<(), NavError> {
    if node.views.is_empty() {
        return Err(NavError::InvalidConfig(format!(
            "stop '{}' on path '{path_id}' has no views",
            node.id
        )));
    }
    let mut seen = HashSet::new();
    for view in &node.views {
        if view.id.is_empty() {
            return Err(NavError::InvalidConfig(format!(
                "stop '{}' on path '{path_id}' has a view with an empty id",
                node.id
            )));
        }
        if !seen.insert(view.id.as_str()) {
            return Err(NavError::InvalidConfig(format!(
                "stop '{}' has a duplicate view '{}'",
                node.id, view.id
            )));
        }
        if !(view.emphasis > 0.0 && view.emphasis <= 100.0) {
            return Err(NavError::InvalidConfig(format!(
                "view '{}' on stop '{}' has an emphasis outside 0–100",
                view.id, node.id
            )));
        }
    }
    place_views(node)?;
    Ok(())
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
