//! Generic graph helpers for ComfyUI API workflows.
//!
//! The link shape is intentionally delegated to the existing onboarding
//! parser. MAIN must make that helper `pub(crate)` before registering this
//! module; keeping the parser in one place avoids two subtly different link
//! definitions.

use crate::{application::workflow_onboarding_service::possible_link, domain::WorkflowDocument};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkflowLink {
    pub source_node_id: String,
    pub source_output_index: u64,
    pub target_node_id: String,
    pub target_input: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowSource {
    pub node_id: String,
    pub input: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowSourceTrace {
    pub source: WorkflowSource,
    /// Links ordered from the requested target input towards the leaf source.
    pub path: Vec<WorkflowLink>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowGraphError {
    NodeNotObject {
        node_id: String,
    },
    InputsMissing {
        node_id: String,
    },
    BrokenLink {
        source_node_id: String,
        target_node_id: String,
        target_input: String,
    },
}

impl fmt::Display for WorkflowGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotObject { node_id } => {
                write!(formatter, "workflow node {node_id} is not an object")
            }
            Self::InputsMissing { node_id } => {
                write!(formatter, "workflow node {node_id} is missing object inputs")
            }
            Self::BrokenLink {
                source_node_id,
                target_node_id,
                target_input,
            } => write!(
                formatter,
                "source node {source_node_id}, target node {target_node_id}, target input {target_input}"
            ),
        }
    }
}

impl Error for WorkflowGraphError {}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkflowGraph {
    pub nodes: BTreeSet<String>,
    /// Incoming links indexed by target node.
    pub upstream: BTreeMap<String, Vec<WorkflowLink>>,
    /// Outgoing links indexed by source node.
    pub downstream: BTreeMap<String, Vec<WorkflowLink>>,
    literal_inputs: BTreeMap<String, BTreeMap<String, Value>>,
}

impl WorkflowGraph {
    pub fn from_document(document: &WorkflowDocument) -> Result<Self, WorkflowGraphError> {
        let Some(workflow) = document.value().as_object() else {
            return Ok(Self {
                nodes: BTreeSet::new(),
                upstream: BTreeMap::new(),
                downstream: BTreeMap::new(),
                literal_inputs: BTreeMap::new(),
            });
        };

        let nodes = workflow.keys().cloned().collect::<BTreeSet<_>>();
        let mut graph = Self {
            nodes,
            upstream: BTreeMap::new(),
            downstream: BTreeMap::new(),
            literal_inputs: BTreeMap::new(),
        };

        for (target_node_id, node) in workflow {
            let Some(node) = node.as_object() else {
                return Err(WorkflowGraphError::NodeNotObject {
                    node_id: target_node_id.clone(),
                });
            };
            let Some(inputs) = node.get("inputs").and_then(Value::as_object) else {
                return Err(WorkflowGraphError::InputsMissing {
                    node_id: target_node_id.clone(),
                });
            };

            for (target_input, value) in inputs {
                if let Some((source_node_id, source_output_index)) = possible_link(value) {
                    if !graph.nodes.contains(source_node_id) {
                        return Err(WorkflowGraphError::BrokenLink {
                            source_node_id: source_node_id.to_owned(),
                            target_node_id: target_node_id.clone(),
                            target_input: target_input.clone(),
                        });
                    }
                    let link = WorkflowLink {
                        source_node_id: source_node_id.to_owned(),
                        source_output_index,
                        target_node_id: target_node_id.clone(),
                        target_input: target_input.clone(),
                    };
                    graph
                        .upstream
                        .entry(target_node_id.clone())
                        .or_default()
                        .push(link.clone());
                    graph
                        .downstream
                        .entry(source_node_id.to_owned())
                        .or_default()
                        .push(link);
                } else {
                    graph
                        .literal_inputs
                        .entry(target_node_id.clone())
                        .or_default()
                        .insert(target_input.clone(), value.clone());
                }
            }
        }

        for links in graph.upstream.values_mut() {
            links.sort();
        }
        for links in graph.downstream.values_mut() {
            links.sort();
        }
        Ok(graph)
    }

    pub fn upstream_of(&self, node_id: &str) -> &[WorkflowLink] {
        self.upstream.get(node_id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn incoming_source(&self, node_id: &str, input_name: &str) -> Option<&str> {
        self.upstream_of(node_id)
            .iter()
            .find(|link| link.target_input == input_name)
            .map(|link| link.source_node_id.as_str())
    }

    pub fn downstream_of(&self, node_id: &str) -> &[WorkflowLink] {
        self.downstream
            .get(node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns the node itself and every node reachable through incoming links.
    pub fn upstream_closure(&self, node_id: &str) -> BTreeSet<String> {
        self.closure(node_id, true)
    }

    /// Returns the node itself and every node reachable through outgoing links.
    pub fn downstream_closure(&self, node_id: &str) -> BTreeSet<String> {
        self.closure(node_id, false)
    }

    /// Traces primitive literal leaves for a linked or literal input.
    pub fn trace_sources(
        &self,
        target_node_id: &str,
        target_input: &str,
    ) -> Vec<WorkflowSourceTrace> {
        let links = self
            .upstream_of(target_node_id)
            .iter()
            .filter(|link| link.target_input == target_input)
            .cloned()
            .collect::<Vec<_>>();

        if links.is_empty() {
            return self
                .literal_inputs
                .get(target_node_id)
                .and_then(|inputs| inputs.get(target_input))
                .filter(|value| is_json_scalar(value))
                .map(|value| WorkflowSourceTrace {
                    source: WorkflowSource {
                        node_id: target_node_id.to_owned(),
                        input: target_input.to_owned(),
                        value: value.clone(),
                    },
                    path: Vec::new(),
                })
                .into_iter()
                .collect();
        }

        let mut pending = links
            .into_iter()
            .map(|link| (link.source_node_id.clone(), vec![link]))
            .collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        let mut traces = Vec::new();

        while let Some((node_id, path)) = pending.pop() {
            if !visited.insert(node_id.clone()) {
                continue;
            }

            let incoming = self.upstream_of(&node_id);
            let linked_inputs = incoming
                .iter()
                .map(|link| link.target_input.as_str())
                .collect::<BTreeSet<_>>();
            if let Some(inputs) = self.literal_inputs.get(&node_id) {
                for (input, value) in inputs {
                    if !linked_inputs.contains(input.as_str()) && is_json_scalar(value) {
                        traces.push(WorkflowSourceTrace {
                            source: WorkflowSource {
                                node_id: node_id.clone(),
                                input: input.clone(),
                                value: value.clone(),
                            },
                            path: path.clone(),
                        });
                    }
                }
            }

            for link in incoming {
                let mut next_path = path.clone();
                next_path.push(link.clone());
                pending.push((link.source_node_id.clone(), next_path));
            }
        }

        traces.sort_by(|left, right| {
            left.source
                .node_id
                .cmp(&right.source.node_id)
                .then(left.source.input.cmp(&right.source.input))
                .then(left.path.len().cmp(&right.path.len()))
        });
        traces
    }

    /// Traces numeric literal leaves, which is the useful form for derived
    /// frame/duration inference.
    pub fn trace_scalar_sources(
        &self,
        target_node_id: &str,
        target_input: &str,
    ) -> Vec<WorkflowSourceTrace> {
        self.trace_sources(target_node_id, target_input)
            .into_iter()
            .filter(|trace| trace.source.value.is_number())
            .collect()
    }

    /// Returns a source only when exactly one distinct numeric leaf exists.
    pub fn unique_scalar_source(
        &self,
        target_node_id: &str,
        target_input: &str,
    ) -> Option<WorkflowSource> {
        let mut sources = BTreeMap::new();
        for trace in self.trace_scalar_sources(target_node_id, target_input) {
            let key = (trace.source.node_id.clone(), trace.source.input.clone());
            sources.entry(key).or_insert(trace.source);
        }
        (sources.len() == 1)
            .then(|| sources.into_values().next())
            .flatten()
    }

    /// `node_id` is on the selected output's dependency path, including the
    /// output node itself.
    pub fn is_on_output_path(&self, node_id: &str, output_node_id: &str) -> bool {
        self.upstream_closure(output_node_id).contains(node_id)
    }

    fn closure(&self, start: &str, upstream: bool) -> BTreeSet<String> {
        if !self.nodes.contains(start) {
            return BTreeSet::new();
        }
        let mut result = BTreeSet::new();
        let mut pending = vec![start.to_owned()];
        while let Some(node_id) = pending.pop() {
            if !result.insert(node_id.clone()) {
                continue;
            }
            let links = if upstream {
                self.upstream_of(&node_id)
            } else {
                self.downstream_of(&node_id)
            };
            for link in links {
                pending.push(if upstream {
                    link.source_node_id.clone()
                } else {
                    link.target_node_id.clone()
                });
            }
        }
        result
    }
}

fn is_json_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_workflow() -> WorkflowDocument {
        WorkflowDocument::parse(json!({
            "49": {"class_type": "FloatConstant", "inputs": {"value": 5}},
            "35": {"class_type": "ComfyMathExpression", "inputs": {
                "expression": "a * 24", "values.a": ["49", 0]
            }},
            "59": {"class_type": "Text Multiline", "inputs": {"text": "test prompt"}},
            "63": {"class_type": "VideoGenerator", "inputs": {
                "prompt": ["59", 0], "length": ["35", 1]
            }},
            "62": {"class_type": "VHS_VideoCombine", "inputs": {
                "images": ["63", 0]
            }},
            "40": {"class_type": "easy clearCacheAll", "inputs": {
                "anything": ["63", 0]
            }}
        }))
        .unwrap()
    }

    #[test]
    fn indexes_links_and_closures_in_both_directions() {
        let graph = WorkflowGraph::from_document(&sample_workflow()).unwrap();

        assert_eq!(graph.upstream_of("63").len(), 2);
        assert_eq!(graph.downstream_of("59")[0].target_input, "prompt");
        assert_eq!(
            graph.upstream_closure("62"),
            ["35", "49", "59", "62", "63"]
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(
            graph.downstream_closure("49"),
            ["35", "49", "62", "63", "40"]
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn traces_prompt_and_unique_numeric_leaf_without_node_id_rules() {
        let graph = WorkflowGraph::from_document(&sample_workflow()).unwrap();

        let prompt = graph.trace_sources("63", "prompt");
        assert_eq!(prompt.len(), 1);
        assert_eq!(prompt[0].source.node_id, "59");
        assert_eq!(prompt[0].source.input, "text");

        let duration = graph.unique_scalar_source("63", "length").unwrap();
        assert_eq!(duration.node_id, "49");
        assert_eq!(duration.input, "value");
        assert_eq!(duration.value, json!(5));
        assert_eq!(graph.trace_scalar_sources("63", "length")[0].path.len(), 2);
    }

    #[test]
    fn leaves_unique_source_empty_when_a_math_path_has_multiple_numeric_inputs() {
        let document = WorkflowDocument::parse(json!({
            "1": {"class_type": "Math", "inputs": {"a": 1, "b": 2}},
            "2": {"class_type": "Consumer", "inputs": {"value": ["1", 0]}}
        }))
        .unwrap();
        let graph = WorkflowGraph::from_document(&document).unwrap();

        assert_eq!(graph.trace_scalar_sources("2", "value").len(), 2);
        assert!(graph.unique_scalar_source("2", "value").is_none());
    }

    #[test]
    fn output_path_does_not_include_unrelated_utility_branch() {
        let graph = WorkflowGraph::from_document(&sample_workflow()).unwrap();

        assert!(graph.is_on_output_path("63", "62"));
        assert!(graph.is_on_output_path("49", "62"));
        assert!(!graph.is_on_output_path("40", "62"));
    }

    #[test]
    fn rejects_broken_links() {
        let document = WorkflowDocument::parse(json!({
            "1": {"class_type": "Consumer", "inputs": {"value": ["404", 0]}}
        }))
        .unwrap();

        assert!(matches!(
            WorkflowGraph::from_document(&document),
            Err(WorkflowGraphError::BrokenLink { .. })
        ));
    }
}
