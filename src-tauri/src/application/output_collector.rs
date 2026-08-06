use crate::application::ports::{
    ComfyAdapter, ComfyAdapterError, ComfyHistory, ComfyOutputFile, ComfyOutputStream,
};
use crate::domain::{OutputType, Recipe};
use std::{collections::HashSet, error::Error, fmt, sync::Arc};

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

pub struct CollectedVideo {
    pub output_id: String,
    pub node_id: String,
    pub original_filename: String,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub position: usize,
    pub subfolder: String,
    pub folder_type: String,
    pub stream: Box<dyn ComfyOutputStream>,
}

pub enum CollectedOutput {
    Image(CollectedImage),
    Video(CollectedVideo),
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
                "OUTPUT_MISSING: required output {output_id} has no saved results at node {node_id}"
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
            let files = output_files(node_output);
            if files.is_empty() {
                if output.required {
                    return Err(OutputCollectorError::OutputMissing {
                        output_id: output.id.clone(),
                        node_id: output.node.clone(),
                    });
                }
                continue;
            }

            for (position, file) in files.iter().enumerate() {
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

    #[allow(dead_code)]
    pub async fn collect_outputs(
        &self,
        recipe: &Recipe,
        prompt_id: &str,
    ) -> Result<Vec<CollectedOutput>, OutputCollectorError> {
        self.collect_outputs_excluding(recipe, prompt_id, &HashSet::new())
            .await
    }

    pub async fn collect_outputs_excluding(
        &self,
        recipe: &Recipe,
        prompt_id: &str,
        existing_outputs: &HashSet<(String, usize)>,
    ) -> Result<Vec<CollectedOutput>, OutputCollectorError> {
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
        self.collect_outputs_from_history_excluding(recipe, &history, existing_outputs)
            .await
    }

    #[allow(dead_code)]
    pub async fn collect_outputs_from_history(
        &self,
        recipe: &Recipe,
        history: &ComfyHistory,
    ) -> Result<Vec<CollectedOutput>, OutputCollectorError> {
        self.collect_outputs_from_history_excluding(recipe, history, &HashSet::new())
            .await
    }

    pub async fn collect_outputs_from_history_excluding(
        &self,
        recipe: &Recipe,
        history: &ComfyHistory,
        existing_outputs: &HashSet<(String, usize)>,
    ) -> Result<Vec<CollectedOutput>, OutputCollectorError> {
        let mut collected = Vec::new();
        for output in &recipe.outputs {
            let Some(node_output) = history.outputs.get(&output.node) else {
                if output.required
                    && !existing_outputs
                        .iter()
                        .any(|(output_id, _)| output_id == &output.id)
                {
                    return Err(OutputCollectorError::OutputMissing {
                        output_id: output.id.clone(),
                        node_id: output.node.clone(),
                    });
                }
                continue;
            };
            let files = output_files(node_output);
            if files.is_empty() {
                if output.required
                    && !existing_outputs
                        .iter()
                        .any(|(output_id, _)| output_id == &output.id)
                {
                    return Err(OutputCollectorError::OutputMissing {
                        output_id: output.id.clone(),
                        node_id: output.node.clone(),
                    });
                }
                continue;
            }
            for (position, file) in files.iter().enumerate() {
                if existing_outputs.contains(&(output.id.clone(), position)) {
                    continue;
                }
                match output.output_type {
                    OutputType::Image => {
                        let data = self
                            .adapter
                            .download_output(file)
                            .await
                            .map_err(map_adapter_error)?;
                        collected.push(CollectedOutput::Image(CollectedImage {
                            output_id: output.id.clone(),
                            node_id: output.node.clone(),
                            original_filename: file.filename.clone(),
                            bytes: data.bytes,
                            content_type: data.content_type,
                            position,
                            subfolder: file.subfolder.clone(),
                            folder_type: file.folder_type.clone(),
                        }));
                    }
                    OutputType::Video => {
                        let stream = self
                            .adapter
                            .open_output_stream(file)
                            .await
                            .map_err(map_adapter_error)?;
                        let content_type = stream.content_type().map(str::to_owned);
                        let content_length = stream.content_length();
                        collected.push(CollectedOutput::Video(CollectedVideo {
                            output_id: output.id.clone(),
                            node_id: output.node.clone(),
                            original_filename: file.filename.clone(),
                            content_type,
                            content_length,
                            position,
                            subfolder: file.subfolder.clone(),
                            folder_type: file.folder_type.clone(),
                            stream,
                        }));
                    }
                }
            }
        }
        Ok(collected)
    }
}

fn output_files(node_output: &crate::application::ports::ComfyNodeOutput) -> Vec<ComfyOutputFile> {
    if node_output.saved_results.is_empty() {
        return node_output.images.clone();
    }
    node_output
        .saved_results
        .iter()
        .map(|result| result.file.clone())
        .collect()
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
    use super::{CollectedImage, CollectedOutput, OutputCollector, OutputCollectorError};
    use crate::application::ports::{
        ComfyAdapter, ComfyAdapterError, ComfyEventSubscription, ComfyHealth, ComfyHistory,
        ComfyNodeOutput, ComfyOutputData, ComfyOutputFile, ComfyOutputStream, ComfySavedResult,
        DeviceInfo, PromptSubmission, SystemStats,
    };
    use crate::domain::{
        Binding, InputDefinition, OutputDefinition, OutputType, Recipe, WorkflowRef,
    };
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, HashSet};
    use std::sync::Arc;

    struct FakeVideoStream {
        sent: bool,
    }

    #[async_trait]
    impl ComfyOutputStream for FakeVideoStream {
        fn content_type(&self) -> Option<&str> {
            Some("video/mp4")
        }

        fn content_length(&self) -> Option<u64> {
            Some(8)
        }

        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ComfyAdapterError> {
            if self.sent {
                Ok(None)
            } else {
                self.sent = true;
                Ok(Some(vec![0, 0, 0, 0, b'f', b't', b'y', b'p']))
            }
        }
    }

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

        async fn open_output_stream(
            &self,
            _file: &ComfyOutputFile,
        ) -> Result<Box<dyn ComfyOutputStream>, ComfyAdapterError> {
            Ok(Box::new(FakeVideoStream { sent: false }))
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
                (
                    "9".to_owned(),
                    ComfyNodeOutput {
                        images,
                        saved_results: Vec::new(),
                    },
                ),
                (
                    "3".to_owned(),
                    ComfyNodeOutput {
                        images: vec![ComfyOutputFile {
                            filename: "ignored.png".to_owned(),
                            subfolder: String::new(),
                            folder_type: "output".to_owned(),
                        }],
                        saved_results: Vec::new(),
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

    #[tokio::test]
    async fn excludes_already_mapped_output_positions_before_download() {
        let fixture_history = history(vec![
            ComfyOutputFile {
                filename: "one.png".to_owned(),
                subfolder: String::new(),
                folder_type: "output".to_owned(),
            },
            ComfyOutputFile {
                filename: "two.png".to_owned(),
                subfolder: String::new(),
                folder_type: "output".to_owned(),
            },
        ]);
        let adapter = Arc::new(FakeAdapter {
            history: Ok(fixture_history.clone()),
            bytes: vec![1, 2, 3],
        });
        let existing = HashSet::from([("generated_image".to_owned(), 0usize)]);
        let outputs = OutputCollector::new(adapter)
            .collect_outputs_from_history_excluding(&recipe(true), &fixture_history, &existing)
            .await
            .expect("mapped output should be skipped");
        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            CollectedOutput::Image(image) => assert_eq!(image.position, 1),
            CollectedOutput::Video(_) => panic!("image recipe must not produce video output"),
        }
    }

    #[tokio::test]
    async fn video_output_uses_saved_results_and_streams_the_declared_media() {
        let adapter = Arc::new(FakeAdapter {
            history: Ok(ComfyHistory {
                prompt_id: "prompt-1".to_owned(),
                status: Default::default(),
                outputs: BTreeMap::from([(
                    "11".to_owned(),
                    ComfyNodeOutput {
                        images: Vec::new(),
                        saved_results: vec![ComfySavedResult {
                            file: ComfyOutputFile {
                                filename: "ComfyUI_00001.mp4".to_owned(),
                                subfolder: String::new(),
                                folder_type: "output".to_owned(),
                            },
                            animated: Some(true),
                        }],
                    },
                )]),
            }),
            bytes: Vec::new(),
        });
        let recipe = Recipe {
            schema_version: 1,
            id: "video_recipe".to_owned(),
            name: "Video".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::new(),
            bindings: Vec::new(),
            outputs: vec![OutputDefinition {
                id: "generated_video".to_owned(),
                output_type: OutputType::Video,
                node: "11".to_owned(),
                required: true,
            }],
        };
        let outputs = OutputCollector::new(adapter)
            .collect_outputs(&recipe, "prompt-1")
            .await
            .expect("video output should collect");
        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            CollectedOutput::Video(video) => {
                assert_eq!(video.content_type.as_deref(), Some("video/mp4"))
            }
            CollectedOutput::Image(_) => panic!("video recipe must not produce an image output"),
        }
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
