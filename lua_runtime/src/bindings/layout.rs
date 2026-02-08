//! Layout bindings for Lua
//!
//! Provides graph layout algorithms for canvas apps.
//! Pure functions: takes nodes/edges, returns positions.

use mlua::{UserData, UserDataMethods, Error as LuaError, Table};
use std::collections::HashMap;

// Layout Bindings

/// Layout bindings for Lua - exposes graph layout algorithms
///
/// **Methods**:
/// - `layout:flowchart(nodes, edges)` - Layered/hierarchical layout (Sugiyama-style)
/// - `layout:force(nodes, edges, iterations)` - Force-directed layout
/// - `layout:tree(nodes, edges, root_id)` - Tree layout from root
/// - `layout:grid(nodes, columns)` - Simple grid layout
///
/// All methods are pure functions: input nodes/edges → output positions
pub struct LayoutBindings;

impl LayoutBindings {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LayoutBindings {
    fn default() -> Self {
        Self::new()
    }
}

impl UserData for LayoutBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // layout:flowchart(nodes, edges) -> positions
        // Input:
        //   nodes = { id = { width = 100, height = 50 }, ... }
        //   edges = { { from = "id1", to = "id2" }, ... }
        // Output:
        //   positions = { id = { x = 0, y = 0 }, ... }
        methods.add_method("flowchart", |lua, _, (nodes, edges): (Table, Table)| {
            flowchart_layout(lua, nodes, edges)
        });

        // layout:force(nodes, edges, iterations) -> positions
        // Simple force-directed layout
        methods.add_method("force", |lua, _, (nodes, edges, iterations): (Table, Table, Option<usize>)| {
            force_layout(lua, nodes, edges, iterations.unwrap_or(100))
        });

        // layout:tree(nodes, edges, root_id) -> positions
        // Tree layout starting from root
        methods.add_method("tree", |lua, _, (nodes, edges, root_id): (Table, Table, String)| {
            tree_layout(lua, nodes, edges, &root_id)
        });

        // layout:grid(nodes, columns) -> positions
        // Simple grid layout
        methods.add_method("grid", |lua, _, (nodes, columns): (Table, Option<usize>)| {
            grid_layout(lua, nodes, columns.unwrap_or(3))
        });
    }
}

/// Parse nodes table from Lua
/// Expected format: { id = { width = 100, height = 50 }, ... }
fn parse_nodes(nodes: &Table) -> Result<HashMap<String, (f64, f64)>, LuaError> {
    let mut result = HashMap::new();

    for pair in nodes.pairs::<String, Table>() {
        let (id, props) = pair?;
        let width: f64 = props.get("width").unwrap_or(100.0);
        let height: f64 = props.get("height").unwrap_or(50.0);
        result.insert(id, (width, height));
    }

    Ok(result)
}

/// Parse edges table from Lua
/// Expected format: { { from = "id1", to = "id2" }, ... }
fn parse_edges(edges: &Table) -> Result<Vec<(String, String)>, LuaError> {
    let mut result = Vec::new();

    for pair in edges.pairs::<usize, Table>() {
        let (_, edge) = pair?;
        let from: String = edge.get("from")?;
        let to: String = edge.get("to")?;
        result.push((from, to));
    }

    Ok(result)
}

/// Build positions table for Lua
fn build_positions_table(
    lua: &mlua::Lua,
    positions: &HashMap<String, (f64, f64)>,
) -> Result<Table, LuaError> {
    let result = lua.create_table()?;

    for (id, (x, y)) in positions {
        let pos = lua.create_table()?;
        pos.set("x", *x)?;
        pos.set("y", *y)?;
        result.set(id.clone(), pos)?;
    }

    Ok(result)
}

/// Flowchart/layered layout
///
/// Uses Sugiyama-style layered layout for DAGs
fn flowchart_layout(
    lua: &mlua::Lua,
    nodes_table: Table,
    edges_table: Table,
) -> Result<Table, LuaError> {
    let nodes = parse_nodes(&nodes_table)?;
    let edges = parse_edges(&edges_table)?;

    if nodes.is_empty() {
        return build_positions_table(lua, &HashMap::new());
    }

    // Build adjacency list
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    for id in nodes.keys() {
        adj.insert(id.clone(), Vec::new());
        in_degree.insert(id.clone(), 0);
    }

    for (from, to) in &edges {
        if nodes.contains_key(from) && nodes.contains_key(to) {
            adj.get_mut(from).unwrap().push(to.clone());
            *in_degree.get_mut(to).unwrap() += 1;
        }
    }

    // Topological sort to assign layers
    let mut layers: HashMap<String, usize> = HashMap::new();
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut current_layer = 0;
    while !queue.is_empty() {
        let mut next_queue = Vec::new();

        for id in queue {
            layers.insert(id.clone(), current_layer);

            for neighbor in adj.get(&id).unwrap_or(&Vec::new()) {
                let deg = in_degree.get_mut(neighbor).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next_queue.push(neighbor.clone());
                }
            }
        }

        queue = next_queue;
        current_layer += 1;
    }

    // Handle nodes not reached (cycles or disconnected)
    for id in nodes.keys() {
        if !layers.contains_key(id) {
            layers.insert(id.clone(), current_layer);
        }
    }

    // Assign positions within layers
    let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
    let mut layer_counts: HashMap<usize, usize> = HashMap::new();
    let layer_spacing = 150.0;
    let node_spacing = 180.0;

    for (id, (width, height)) in &nodes {
        let layer = *layers.get(id).unwrap_or(&0);
        let index = *layer_counts.get(&layer).unwrap_or(&0);
        layer_counts.insert(layer, index + 1);

        let x = index as f64 * node_spacing;
        let y = layer as f64 * layer_spacing;

        // Center the node (position is top-left corner)
        positions.insert(id.clone(), (x - width / 2.0, y - height / 2.0));
    }

    // Center each layer
    let max_layer = current_layer;
    for layer in 0..=max_layer {
        let count = *layer_counts.get(&layer).unwrap_or(&1);
        if count > 0 {
            let layer_width = (count as f64 - 1.0) * node_spacing;
            let offset = layer_width / 2.0;

            for (id, &l) in &layers {
                if l == layer {
                    if let Some((x, _y)) = positions.get_mut(id) {
                        *x -= offset;
                    }
                }
            }
        }
    }

    build_positions_table(lua, &positions)
}

/// Force-directed layout
///
/// Simple spring-based layout algorithm
fn force_layout(
    lua: &mlua::Lua,
    nodes_table: Table,
    edges_table: Table,
    iterations: usize,
) -> Result<Table, LuaError> {
    let nodes = parse_nodes(&nodes_table)?;
    let edges = parse_edges(&edges_table)?;

    if nodes.is_empty() {
        return build_positions_table(lua, &HashMap::new());
    }

    // Initialize positions in a circle
    let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
    let mut i = 0;
    for id in nodes.keys() {
        let angle = i as f64 * 2.0 * std::f64::consts::PI / nodes.len() as f64;
        let radius = 200.0;
        positions.insert(id.clone(), (radius * angle.cos(), radius * angle.sin()));
        i += 1;
    }

    // Force-directed iterations
    let repulsion = 5000.0;
    let attraction = 0.01;
    let damping = 0.85;

    for _ in 0..iterations {
        let mut forces: HashMap<String, (f64, f64)> = HashMap::new();

        for id in nodes.keys() {
            forces.insert(id.clone(), (0.0, 0.0));
        }

        // Repulsion between all nodes
        let ids: Vec<_> = nodes.keys().cloned().collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let id1 = &ids[i];
                let id2 = &ids[j];

                let (x1, y1) = positions[id1];
                let (x2, y2) = positions[id2];

                let dx = x2 - x1;
                let dy = y2 - y1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);

                let force = repulsion / (dist * dist);
                let fx = force * dx / dist;
                let fy = force * dy / dist;

                let f1 = forces.get_mut(id1).unwrap();
                f1.0 -= fx;
                f1.1 -= fy;

                let f2 = forces.get_mut(id2).unwrap();
                f2.0 += fx;
                f2.1 += fy;
            }
        }

        // Attraction along edges
        for (from, to) in &edges {
            if let (Some(&(x1, y1)), Some(&(x2, y2))) = (positions.get(from), positions.get(to)) {
                let dx = x2 - x1;
                let dy = y2 - y1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);

                let force = attraction * dist;
                let fx = force * dx / dist;
                let fy = force * dy / dist;

                if let Some(f) = forces.get_mut(from) {
                    f.0 += fx;
                    f.1 += fy;
                }
                if let Some(f) = forces.get_mut(to) {
                    f.0 -= fx;
                    f.1 -= fy;
                }
            }
        }

        // Apply forces with damping
        for (id, (fx, fy)) in &forces {
            if let Some((x, y)) = positions.get_mut(id) {
                *x += fx * damping;
                *y += fy * damping;
            }
        }
    }

    build_positions_table(lua, &positions)
}

/// Tree layout from a root node
///
/// Places children below parent with even spacing
fn tree_layout(
    lua: &mlua::Lua,
    nodes_table: Table,
    edges_table: Table,
    root_id: &str,
) -> Result<Table, LuaError> {
    let nodes = parse_nodes(&nodes_table)?;
    let edges = parse_edges(&edges_table)?;

    if nodes.is_empty() || !nodes.contains_key(root_id) {
        return build_positions_table(lua, &HashMap::new());
    }

    // Build adjacency list (parent -> children)
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for id in nodes.keys() {
        children.insert(id.clone(), Vec::new());
    }

    for (from, to) in &edges {
        if nodes.contains_key(from) && nodes.contains_key(to) {
            children.get_mut(from).unwrap().push(to.clone());
        }
    }

    let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
    let mut widths: HashMap<String, f64> = HashMap::new();

    // Calculate subtree widths
    fn calc_width(
        id: &str,
        children: &HashMap<String, Vec<String>>,
        nodes: &HashMap<String, (f64, f64)>,
        widths: &mut HashMap<String, f64>,
    ) -> f64 {
        let node_width = nodes.get(id).map(|(w, _)| *w).unwrap_or(100.0);
        let child_list = children.get(id).cloned().unwrap_or_default();

        if child_list.is_empty() {
            widths.insert(id.to_string(), node_width);
            return node_width;
        }

        let total: f64 = child_list
            .iter()
            .map(|c| calc_width(c, children, nodes, widths))
            .sum::<f64>() + 50.0 * (child_list.len() as f64 - 1.0);

        let width = total.max(node_width);
        widths.insert(id.to_string(), width);
        width
    }

    calc_width(root_id, &children, &nodes, &mut widths);

    // Position nodes
    fn position_node(
        id: &str,
        x: f64,
        y: f64,
        children: &HashMap<String, Vec<String>>,
        nodes: &HashMap<String, (f64, f64)>,
        widths: &HashMap<String, f64>,
        positions: &mut HashMap<String, (f64, f64)>,
    ) {
        let (node_width, node_height) = nodes.get(id).copied().unwrap_or((100.0, 50.0));
        positions.insert(id.to_string(), (x - node_width / 2.0, y));

        let child_list = children.get(id).cloned().unwrap_or_default();
        if child_list.is_empty() {
            return;
        }

        let total_width: f64 = child_list.iter().map(|c| widths.get(c).copied().unwrap_or(100.0)).sum::<f64>()
            + 50.0 * (child_list.len() as f64 - 1.0);

        let mut child_x = x - total_width / 2.0;
        let child_y = y + node_height + 80.0;

        for child in child_list {
            let child_width = widths.get(&child).copied().unwrap_or(100.0);
            position_node(
                &child,
                child_x + child_width / 2.0,
                child_y,
                children,
                nodes,
                widths,
                positions,
            );
            child_x += child_width + 50.0;
        }
    }

    position_node(root_id, 0.0, 0.0, &children, &nodes, &widths, &mut positions);

    build_positions_table(lua, &positions)
}

/// Simple grid layout
///
/// Arranges nodes in a grid with specified columns
fn grid_layout(
    lua: &mlua::Lua,
    nodes_table: Table,
    columns: usize,
) -> Result<Table, LuaError> {
    let nodes = parse_nodes(&nodes_table)?;

    if nodes.is_empty() {
        return build_positions_table(lua, &HashMap::new());
    }

    let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
    let spacing_x = 180.0;
    let spacing_y = 120.0;

    // Sort nodes by ID for consistent ordering
    let mut ids: Vec<_> = nodes.keys().cloned().collect();
    ids.sort();

    for (i, id) in ids.iter().enumerate() {
        let col = i % columns;
        let row = i / columns;
        let x = col as f64 * spacing_x;
        let y = row as f64 * spacing_y;
        positions.insert(id.clone(), (x, y));
    }

    build_positions_table(lua, &positions)
}
