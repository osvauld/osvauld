//! Dependency Graph for HUML Templates
//!
//! Tracks dependencies between:
//! - Variables (document fields, query results)
//! - Computed values (CEL expressions)
//! - UI nodes (blocks that render values)
//!
//! When a variable changes, the graph identifies which computed values
//! and UI nodes need to be re-evaluated/re-rendered.

use std::collections::{HashMap, HashSet};
use indexmap::IndexSet;

/// Unique identifier for a UI node
pub type NodeId = u32;

/// Dependency graph for reactive updates
///
/// **Structure**:
/// ```text
/// Variables (sources)
///     │
///     ├── Computed values (derived)
///     │       │
///     └───────┴── UI Nodes (consumers)
/// ```
///
/// **Usage**:
/// 1. Template loaded → build graph from parsed deps
/// 2. Variable changes → `invalidate(var)` → get dirty computed + nodes
/// 3. Re-evaluate dirty computed values
/// 4. Re-render dirty nodes
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Variable → computed values that depend on it
    variable_to_computed: HashMap<String, HashSet<String>>,

    /// Variable → UI nodes that depend on it
    variable_to_nodes: HashMap<String, HashSet<NodeId>>,

    /// Computed → variables it depends on (reverse lookup)
    computed_to_variables: HashMap<String, HashSet<String>>,

    /// Computed → UI nodes that depend on it
    computed_to_nodes: HashMap<String, HashSet<NodeId>>,

    /// Node → variables/computed it depends on
    node_to_deps: HashMap<NodeId, HashSet<String>>,

    /// Currently dirty computed values (need re-evaluation)
    dirty_computed: IndexSet<String>,

    /// Currently dirty nodes (need re-render)
    dirty_nodes: IndexSet<NodeId>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a computed value's dependencies
    ///
    /// **Called during**: Template setup
    /// **Example**: `computed "isPositive" depends on ["count"]`
    pub fn register_computed(&mut self, name: &str, deps: &[String]) {
        for dep in deps {
            self.variable_to_computed
                .entry(dep.clone())
                .or_default()
                .insert(name.to_string());
        }

        self.computed_to_variables
            .insert(name.to_string(), deps.iter().cloned().collect());
    }

    /// Register a UI node's dependencies
    ///
    /// **Called during**: Template setup
    /// **Example**: `node 5 depends on ["count", "isPositive"]`
    pub fn register_node(&mut self, node_id: NodeId, deps: &[String]) {
        for dep in deps {
            // Check if dep is a computed value or raw variable
            if self.computed_to_variables.contains_key(dep) {
                self.computed_to_nodes
                    .entry(dep.clone())
                    .or_default()
                    .insert(node_id);
            } else {
                self.variable_to_nodes
                    .entry(dep.clone())
                    .or_default()
                    .insert(node_id);
            }
        }

        self.node_to_deps
            .insert(node_id, deps.iter().cloned().collect());
    }

    /// Invalidate a variable (mark dependents as dirty)
    ///
    /// **Called when**: Data changes (from QueryDelta or user input)
    /// **Returns**: (dirty_computed, dirty_nodes) for re-evaluation
    pub fn invalidate(&mut self, variable: &str) {
        // Mark computed values as dirty
        if let Some(computed_deps) = self.variable_to_computed.get(variable) {
            for computed in computed_deps {
                self.dirty_computed.insert(computed.clone());

                // Also mark nodes that depend on this computed value
                if let Some(node_deps) = self.computed_to_nodes.get(computed) {
                    for node_id in node_deps {
                        self.dirty_nodes.insert(*node_id);
                    }
                }
            }
        }

        // Mark nodes that directly depend on this variable
        if let Some(node_deps) = self.variable_to_nodes.get(variable) {
            for node_id in node_deps {
                self.dirty_nodes.insert(*node_id);
            }
        }
    }

    /// Invalidate multiple variables at once
    pub fn invalidate_many(&mut self, variables: &[String]) {
        for var in variables {
            self.invalidate(var);
        }
    }

    /// Get dirty computed values in dependency order
    ///
    /// Returns computed values that need re-evaluation, ordered so that
    /// dependencies are evaluated before dependents.
    pub fn take_dirty_computed(&mut self) -> Vec<String> {
        let dirty: Vec<String> = self.dirty_computed.drain(..).collect();

        // TODO: Topological sort for proper dependency order
        // For MVP, just return in insertion order (usually correct for simple cases)
        dirty
    }

    /// Get dirty nodes
    pub fn take_dirty_nodes(&mut self) -> Vec<NodeId> {
        self.dirty_nodes.drain(..).collect()
    }

    /// Check if there are any dirty items
    pub fn has_dirty(&self) -> bool {
        !self.dirty_computed.is_empty() || !self.dirty_nodes.is_empty()
    }

    /// Mark all nodes as dirty (for initial render)
    pub fn mark_all_dirty(&mut self) {
        for computed in self.computed_to_variables.keys() {
            self.dirty_computed.insert(computed.clone());
        }
        for node_id in self.node_to_deps.keys() {
            self.dirty_nodes.insert(*node_id);
        }
    }

    /// Get all dependencies for a node
    pub fn get_node_deps(&self, node_id: NodeId) -> Option<&HashSet<String>> {
        self.node_to_deps.get(&node_id)
    }

    /// Clear all registrations (for template reload)
    pub fn clear(&mut self) {
        self.variable_to_computed.clear();
        self.variable_to_nodes.clear();
        self.computed_to_variables.clear();
        self.computed_to_nodes.clear();
        self.node_to_deps.clear();
        self.dirty_computed.clear();
        self.dirty_nodes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computed_dependency() {
        let mut graph = DependencyGraph::new();

        // Register: isPositive depends on count
        graph.register_computed("isPositive", &["count".to_string()]);

        // Register: node 1 depends on isPositive
        graph.register_node(1, &["isPositive".to_string()]);

        // Invalidate count
        graph.invalidate("count");

        // Both computed and node should be dirty
        assert!(graph.dirty_computed.contains("isPositive"));
        assert!(graph.dirty_nodes.contains(&1));
    }

    #[test]
    fn test_direct_variable_dependency() {
        let mut graph = DependencyGraph::new();

        // Register: node 1 depends directly on count (no computed)
        graph.register_node(1, &["count".to_string()]);

        // Invalidate count
        graph.invalidate("count");

        // Node should be dirty
        assert!(graph.dirty_nodes.contains(&1));
    }

    #[test]
    fn test_multiple_dependencies() {
        let mut graph = DependencyGraph::new();

        // isPositive depends on count
        graph.register_computed("isPositive", &["count".to_string()]);

        // doubleCount depends on count
        graph.register_computed("doubleCount", &["count".to_string()]);

        // Node 1 depends on both
        graph.register_node(1, &["isPositive".to_string(), "doubleCount".to_string()]);

        // Invalidate count
        graph.invalidate("count");

        // Both computed should be dirty
        assert!(graph.dirty_computed.contains("isPositive"));
        assert!(graph.dirty_computed.contains("doubleCount"));

        // Node should be dirty (only once)
        let dirty_nodes = graph.take_dirty_nodes();
        assert_eq!(dirty_nodes.len(), 1);
        assert_eq!(dirty_nodes[0], 1);
    }
}
