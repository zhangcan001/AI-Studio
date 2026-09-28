//! Generic graph helpers for ComfyUI API workflows.
//!
//! The link shape is intentionally delegated to the existing onboarding
//! parser. MAIN must make that helper `pub(crate)` before registering this
//! module; keeping the parser in one place avoids two subtly different link
//! definitions.

use crate::{application::workflow_onboarding_service::possible_link, domain::WorkflowDocument};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    sync::{Arc, Mutex},
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
    NoOutputRoots,
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
            Self::NoOutputRoots => write!(formatter, "workflow has no resolved output roots"),
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

type TraceKey = (String, String);

/// W-09: memoized derived queries. The graph is immutable after construction,
/// so cached answers never go stale. The cache is not part of graph identity.
#[derive(Default)]
struct GraphQueryCache {
    traces: Mutex<HashMap<TraceKey, Arc<Vec<WorkflowSourceTrace>>>>,
    upstream_closures: Mutex<HashMap<String, Arc<BTreeSet<String>>>>,
}

impl Clone for GraphQueryCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl fmt::Debug for GraphQueryCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GraphQueryCache")
    }
}

impl PartialEq for GraphQueryCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkflowGraph {
    pub nodes: BTreeSet<String>,
    /// Incoming links indexed by target node.
    pub upstream: BTreeMap<String, Vec<WorkflowLink>>,
    /// Outgoing links indexed by source node.
    pub downstream: BTreeMap<String, Vec<WorkflowLink>>,
    literal_inputs: BTreeMap<String, BTreeMap<String, Value>>,
    cache: GraphQueryCache,
}

impl WorkflowGraph {
    pub fn from_document(document: &WorkflowDocument) -> Result<Self, WorkflowGraphError> {
        let Some(workflow) = document.value().as_object() else {
            return Ok(Self::default());
        };

        let nodes = workflow.keys().cloned().collect::<BTreeSet<_>>();
        let mut graph = Self {
            nodes,
            ..Self::default()
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
        self.upstream_closure_shared(node_id).as_ref().clone()
    }

    /// Memoized upstream closure (W-09).
    pub fn upstream_closure_shared(&self, node_id: &str) -> Arc<BTreeSet<String>> {
        if let Some(cached) = self
            .cache
            .upstream_closures
            .lock()
            .ok()
            .and_then(|cache| cache.get(node_id).cloned())
        {
            return cached;
        }
        let computed = Arc::new(self.closure(node_id, true));
        if let Ok(mut cache) = self.cache.upstream_closures.lock() {
            cache.insert(node_id.to_owned(), computed.clone());
        }
        computed
    }

    /// Returns the node itself and every node reachable through outgoing links.
    pub fn downstream_closure(&self, node_id: &str) -> BTreeSet<String> {
        self.closure(node_id, false)
    }

    /// Traces primitive literal leaves for a linked or literal input.
    ///
    /// W-09: results are memoized per (node, input); the traversal itself keeps
    /// a visited set so cycles terminate.
    pub fn trace_sources(
        &self,
        target_node_id: &str,
        target_input: &str,
    ) -> Vec<WorkflowSourceTrace> {
        self.trace_sources_shared(target_node_id, target_input)
            .as_ref()
            .clone()
    }

    pub fn trace_sources_shared(
        &self,
        target_node_id: &str,
        target_input: &str,
    ) -> Arc<Vec<WorkflowSourceTrace>> {
        let key = (target_node_id.to_owned(), target_input.to_owned());
        if let Some(cached) = self
            .cache
            .traces
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned())
        {
            return cached;
        }
        let computed = Arc::new(self.trace_sources_uncached(target_node_id, target_input));
        if let Ok(mut cache) = self.cache.traces.lock() {
            cache.insert(key, computed.clone());
        }
        computed
    }

    fn trace_sources_uncached(
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
        self.trace_sources_shared(target_node_id, target_input)
            .iter()
            .filter(|trace| trace.source.value.is_number())
            .cloned()
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
        self.upstream_closure_shared(output_node_id)
            .contains(node_id)
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
    fn w09_trace_sources_memoized_equivalence() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workflow_recognition_v3/real");
        let mut checked = 0usize;
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path().join("sample.api.json");
            if !path.exists() {
                continue;
            }
            let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let Ok(document) = WorkflowDocument::parse(value) else {
                continue;
            };
            let Ok(graph) = WorkflowGraph::from_document(&document) else {
                continue;
            };
            let object = document.value().as_object().unwrap();
            for (node_id, node) in object {
                let Some(inputs) = node.get("inputs").and_then(Value::as_object) else {
                    continue;
                };
                for input in inputs.keys() {
                    let expected = graph.trace_sources_uncached(node_id, input);
                    assert_eq!(graph.trace_sources(node_id, input), expected);
                    // Second call is served from the cache and must be identical.
                    assert_eq!(graph.trace_sources(node_id, input), expected);
                    assert_eq!(
                        graph.upstream_closure(node_id),
                        graph.closure(node_id, true)
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "only {checked} inputs checked");
    }

    #[test]
    fn w09_large_graph_under_budget() {
        let mut nodes = serde_json::Map::new();
        nodes.insert(
            "0".to_owned(),
            json!({"class_type": "Seed", "inputs": {"value": 1, "text": "leaf"}}),
        );
        for index in 1..800 {
            nodes.insert(
                index.to_string(),
                json!({"class_type": "Relay", "inputs": {
                    "value": [(index - 1).to_string(), 0],
                    "scale": 2
                }}),
            );
        }
        let document = WorkflowDocument::parse(Value::Object(nodes)).unwrap();
        let graph = WorkflowGraph::from_document(&document).unwrap();
        let started = std::time::Instant::now();
        // Repeated per-input queries (as analysis does for every mapping
        // candidate) are served from the memo instead of re-running the DFS.
        for _ in 0..50 {
            let _ = graph.trace_sources_shared("799", "value");
        }
        for index in 0..800 {
            assert!(graph.is_on_output_path(&index.to_string(), "799"));
        }
        let elapsed = started.elapsed();
        // Target 500 ms; allow 2 s on slow CI runners.
        assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
        assert_eq!(graph.trace_scalar_sources("799", "value").len(), 799);
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
