//! Worker DAG scheduling (spec §80-§81). Dependent tasks never run together.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::error::{NodkrayError, NodkrayResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagNode {
    pub id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Independent layers. Each layer may run in parallel; layers run in order.
pub fn schedule(nodes: &[DagNode]) -> NodkrayResult<Vec<Vec<String>>> {
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    if ids.len() != nodes.len() {
        return Err(NodkrayError::user_input(
            "DAG_DUPLICATE_TASK",
            "task ids in the DAG must be unique",
        ));
    }

    for node in nodes {
        incoming.entry(node.id.as_str()).or_insert(0);
        for dep in &node.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(NodkrayError::user_input(
                    "DAG_UNKNOWN_DEPENDENCY",
                    format!("task '{}' depends on unknown '{}'", node.id, dep),
                ));
            }
            *incoming.entry(node.id.as_str()).or_insert(0) += 1;
            outgoing.entry(dep.as_str()).or_default().push(node.id.as_str());
        }
    }

    let mut ready: VecDeque<&str> = incoming
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut remaining = incoming;
    let mut layers = Vec::new();
    let mut seen = 0usize;

    while !ready.is_empty() {
        let mut layer: Vec<String> = ready.drain(..).map(|id| id.to_string()).collect();
        layer.sort();
        for id in &layer {
            seen += 1;
            if let Some(children) = outgoing.get(id.as_str()) {
                for child in children {
                    if let Some(deg) = remaining.get_mut(child) {
                        *deg -= 1;
                        if *deg == 0 {
                            ready.push_back(child);
                        }
                    }
                }
            }
        }
        layers.push(layer);
    }

    if seen != nodes.len() {
        return Err(NodkrayError::user_input(
            "DAG_CYCLE",
            "task DAG contains a cycle; dependent work cannot be scheduled",
        ));
    }
    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, deps: &[&str]) -> DagNode {
        DagNode {
            id: id.to_string(),
            depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn independent_tasks_share_a_layer() {
        let layers = schedule(&[node("backend", &[]), node("frontend", &[]), node("docs", &[])])
            .expect("schedule");
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0], ["backend", "docs", "frontend"]);
    }

    #[test]
    fn dependents_wait_for_both_parents() {
        let layers = schedule(&[
            node("backend", &[]),
            node("frontend", &[]),
            node("integration", &["backend", "frontend"]),
        ])
        .expect("schedule");
        assert_eq!(layers.len(), 2);
        assert!(layers[0].contains(&"backend".to_string()));
        assert!(layers[0].contains(&"frontend".to_string()));
        assert_eq!(layers[1], ["integration"]);
    }

    #[test]
    fn cycle_is_rejected() {
        let err = schedule(&[node("a", &["b"]), node("b", &["a"])]).expect_err("cycle");
        assert_eq!(err.code(), "DAG_CYCLE");
    }
}
