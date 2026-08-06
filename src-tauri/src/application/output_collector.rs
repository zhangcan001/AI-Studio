use crate::application::ports::{ComfyAdapter, ComfyAdapterError, ComfyHistory};
use crate::domain::{OutputType, Recipe};
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct CollectedImage {
    pub output_id: String,
    pub node_id: String,
    pub original_filename: String,
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub position: usize,
    pub subfolder: String,
    pub folder_type: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputCollectorError {
    HistoryNotFound { prompt_id: String },
    OutputMissing { output_id: String, node_id: String },
    OutputDownloadFailed { message: String },
    OutputTooLarge { message: String },
    Protocol { message: String },
}

impl OutputCollectorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::HistoryNotFound { .. } => "HISTORY_NOT_FOUND",
            Self::OutputMissing { .. } => "OUTPUT_MISSING",
            Self::OutputDownloadFailed { .. } => "OUTPUT_DOWNLOAD_FAILED",
            Self::OutputTooLarge { .. } => "OUTPUT_TOO_LARGE",
            Self::Protocol { .. } => "OUTPUT_DOWNLOAD_FAILED",
        }
    }
}

impl fmt::Display for OutputCollectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HistoryNotFound { prompt_id } => {
                write!(
                    formatter,
                    "HISTORY_NOT_FOUND: prompt {prompt_id} was not found"
                )
            }
            Self::OutputMissing { output_id, node_id } => write!(
                formatter,
                "OUTPUT_MISSING: required output {output_id} has no images at node {node_id}"
            ),
            Self::OutputDownloadFailed { message } => {
                write!(formatter, "OUTPUT_DOWNLOAD_FAILED: {message}")
            }
            Self::OutputTooLarge { message } => write!(formatter, "OUTPUT_TOO_LARGE: {message}"),
            Self::Protocol { message } => write!(formatter, "OUTPUT_DOWNLOAD_FAILED: {message}"),
        }
    }
}

impl Error for OutputCollectorError {}

pub struct OutputCollector {
    adapter: Arc<dyn ComfyAdapter>,
}

impl OutputCollector {
    pub fn new(adapter: Arc<dyn ComfyAdapter>) -> Self {
        Self { adapter }
    }

    pub async fn collect(
        &self,
        recipe: &Recipe,
        prompt_id: &str,
    ) -> Result<Vec<CollectedImage>, OutputCollectorError> {
        let history = self
            .adapter
            .get_history(prompt_id)
            .await
            .map_err(map_adapter_error)?;
        if history.prompt_id != prompt_id {
            return Err(OutputCollectorError::Protocol {
                message: format!(
                    "history prompt_id mismatch: requested {prompt_id}, received {}",
                    history.prompt_id
                ),
            });
        }
        self.collect_from_history(recipe, &history).await
    }

    pub async fn collect_from_history(
        &self,
        recipe: &Recipe,
        history: &ComfyHistory,
    ) -> Result<Vec<CollectedImage>, OutputCollectorError> {
        let mut collected = Vec::new();
        for output in &recipe.outputs {
            if output.output_type != OutputType::Image {
                continue;
            }
            let Some(node_output) = history.outputs.get(&output.node) else {
                if output.required {
                    return Err(OutputCollectorError::OutputMissing {
                        output_id: output.id.clone(),
                        node_id: output.node.clone(),
                    });
                }
                continue;
            };
            if node_output.images.is_empty() {
                if output.required {
                    return Err(OutputCollectorError::OutputMissing {
                        output_id: output.id.clone(),
                        node_id: output.node.clone(),
                    });
                }
                continue;
            }

            for (position, file) in node_output.images.iter().enumerate() {
                let data = self
                    .adapter
                    .download_output(file)
                    .await
                    .map_err(map_adapter_error)?;
                collected.push(CollectedImage {
                    output_id: output.id.clone(),
                    node_id: output.node.clone(),
                    original_filename: file.filename.clone(),
                    bytes: data.bytes,
                    content_type: data.content_type,
                    position,
                    subfolder: file.subfolder.clone(),
                    folder_type: file.folder_type.clone(),
                });
            }
        }
        Ok(collected)
    }
}

fn map_adapter_error(error: ComfyAdapterError) -> OutputCollectorError {
    match error {
        ComfyAdapterError::HistoryNotFound(prompt_id) => {
            OutputCollectorError::HistoryNotFound { prompt_id }
        }
        ComfyAdapterError::OutputTooLarge(message) => {
            OutputCollectorError::OutputTooLarge { message }
        }
        ComfyAdapterError::Protocol(message) => OutputCollectorError::Protocol { message },
        other => OutputCollectorError::OutputDownloadFailed {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectedImage, OutputCollector, OutputCollectorError};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, DeviceInfo, PromptSubmission,
        SystemStats,
    };
    use crate::domain::{
        Binding, InputDefinition, OutputDefinition, OutputType, Recipe, WorkflowRef,
    };
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct FakeAdapter {
        history: Result<ComfyHistory, ComfyAdapterError>,
        bytes: Vec<u8>,
    }

    #[async_trait]
    impl ComfyAdapter for FakeAdapter {
        async fn health_check(&self) -> Result<ComfyHealth, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_system_stats(&self) -> Result<SystemStats, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_object_info(&self) -> Result<Value, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn get_history(&self, _prompt_id: &str) -> Result<ComfyHistory, ComfyAdapterError> {
            self.history.clone()
        }

        async fn download_output(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<ComfyOutputData, ComfyAdapterError> {
            Ok(ComfyOutputData {
                bytes: self.bytes.clone(),
                content_type: Some("image/png".to_owned()),
            })
        }

        async fn submit_workflow(
            &self,
            _client_id: &str,
            _prompt_id: &str,
            _workflow: Value,
        ) -> Result<PromptSubmission, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }

        async fn subscribe_events(
            &self,
            _client_id: &str,
        ) -> Result<Box<dyn ComfyEventSubscription>, ComfyAdapterError> {
            Err(ComfyAdapterError::Incompatible("not used".to_owned()))
        }
    }

    fn recipe(required: bool) -> Recipe {
        Recipe {
            schema_version: 1,
            id: "recipe".to_owned(),
            name: "Recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::<String, InputDefinition>::new(),
            bindings: Vec::<Binding>::new(),
            outputs: vec![OutputDefinition {
                id: "generated_image".to_owned(),
                output_type: OutputType::Image,
                node: "9".to_owned(),
                required,
            }],
        }
    }

    fn history(images: Vec<ComfyOutputFile>) -> ComfyHistory {
        ComfyHistory {
            prompt_id: "prompt-1".to_owned(),
            status: Default::default(),
            outputs: BTreeMap::from([
                ("9".to_owned(), ComfyNodeOutput { images }),
                (
                    "3".to_owned(),
                    ComfyNodeOutput {
                        images: vec![ComfyOutputFile {
                            filename: "ignored.png".to_owned(),
                            subfolder: String::new(),
                            folder_type: "output".to_owned(),
                        }],
                    },
                ),
            ]),
        }
    }

    #[tokio::test]
    async fn collects_only_recipe_declared_node_and_preserves_multiple_positions() {
        let adapter = Arc::new(FakeAdapter {
            history: Ok(history(vec![
                ComfyOutputFile {
                    filename: "one.png".to_owned(),
                    subfolder: String::new(),
                    folder_type: "output".to_owned(),
                },
                ComfyOutputFile {
                    filename: "two.png".to_owned(),
                    subfolder: "nested".to_owned(),
                    folder_type: "output".to_owned(),
                },
            ])),
            bytes: vec![1, 2, 3],
        });
        let images = OutputCollector::new(adapter)
            .collect(&recipe(true), "prompt-1")
            .await
            .expect("collection should succeed");
        assert_eq!(images.len(), 2);
        assert_eq!(
            images
                .iter()
                .map(|image| (image.node_id.as_str(), image.position))
                .collect::<Vec<_>>(),
            vec![("9", 0), ("9", 1)]
        );
        assert!(images
            .iter()
            .all(|image: &CollectedImage| image.output_id == "generated_image"));
    }

    #[tokio::test]
    async fn missing_required_output_is_reported_before_download() {
        let adapter = Arc::new(FakeAdapter {
            history: Ok(ComfyHistory {
                prompt_id: "prompt-1".to_owned(),
                status: Default::default(),
                outputs: BTreeMap::new(),
            }),
            bytes: Vec::new(),
        });
        let error = OutputCollector::new(adapter)
            .collect(&recipe(true), "prompt-1")
            .await
            .expect_err("missing required output should fail");
        assert!(matches!(
            error,
            OutputCollectorError::OutputMissing { output_id, node_id }
                if output_id == "generated_image" && node_id == "9"
        ));
    }

    #[tokio::test]
    async fn missing_optional_output_is_allowed() {
        let adapter = Arc::new(FakeAdapter {
            history: Ok(ComfyHistory {
                prompt_id: "prompt-1".to_owned(),
                status: Default::default(),
                outputs: BTreeMap::new(),
            }),
            bytes: Vec::new(),
        });
        assert!(OutputCollector::new(adapter)
            .collect(&recipe(false), "prompt-1")
            .await
            .expect("optional output should be allowed")
            .is_empty());
    }

    #[allow(dead_code)]
    fn _json_fixture() -> Value {
        json!({})
    }

    #[allow(dead_code)]
    fn _unused_device() -> DeviceInfo {
        DeviceInfo {
            name: None,
            device_type: None,
            vram_total: None,
            vram_free: None,
        }
    }
}
