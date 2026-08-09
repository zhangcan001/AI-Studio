use async_trait::async_trait;
use serde_json::Value;
use std::{path::Path, process::Command};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioMetadata {
    pub duration_ms: Option<u64>,
}

#[async_trait]
pub trait MediaProbe: Send + Sync {
    async fn probe_video(&self, path: &Path) -> VideoMetadata;
    async fn generate_video_poster(&self, path: &Path) -> Option<Vec<u8>>;

    async fn probe_audio(&self, _path: &Path) -> AudioMetadata {
        AudioMetadata::default()
    }
}

#[derive(Clone, Debug)]
pub struct CommandMediaProbe {
    ffprobe: String,
    ffmpeg: String,
}

impl Default for CommandMediaProbe {
    fn default() -> Self {
        Self {
            ffprobe: "ffprobe".to_owned(),
            ffmpeg: "ffmpeg".to_owned(),
        }
    }
}

#[async_trait]
impl MediaProbe for CommandMediaProbe {
    async fn probe_video(&self, path: &Path) -> VideoMetadata {
        let output = match Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height:format=duration",
                "-of",
                "json",
            ])
            .arg(path)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                tracing::debug!(
                    error_type = std::any::type_name_of_val(&error),
                    "ffprobe unavailable; video metadata remains optional"
                );
                return VideoMetadata::default();
            }
        };
        if !output.status.success() {
            tracing::debug!("ffprobe could not inspect video");
            return VideoMetadata::default();
        }
        parse_probe_json(&output.stdout)
    }

    async fn generate_video_poster(&self, path: &Path) -> Option<Vec<u8>> {
        let output = match Command::new(&self.ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-f",
                "image2pipe",
                "-vcodec",
                "png",
                "pipe:1",
            ])
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                tracing::debug!(
                    error_type = std::any::type_name_of_val(&error),
                    "ffmpeg unavailable; video poster skipped"
                );
                return None;
            }
        };
        if output.status.success() && !output.stdout.is_empty() {
            Some(output.stdout)
        } else {
            tracing::debug!("ffmpeg did not produce a video poster");
            None
        }
    }

    async fn probe_audio(&self, path: &Path) -> AudioMetadata {
        let output = match Command::new(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "format=duration",
                "-of",
                "json",
            ])
            .arg(path)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                tracing::debug!(
                    error_type = std::any::type_name_of_val(&error),
                    "ffprobe unavailable; audio metadata remains optional"
                );
                return AudioMetadata::default();
            }
        };
        if !output.status.success() {
            tracing::debug!("ffprobe could not inspect audio");
            return AudioMetadata::default();
        }
        AudioMetadata {
            duration_ms: parse_duration_ms(&output.stdout),
        }
    }
}

fn parse_probe_json(bytes: &[u8]) -> VideoMetadata {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return VideoMetadata::default();
    };
    let stream = value
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .and_then(Value::as_object);
    let width = stream
        .and_then(|stream| stream.get("width"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let height = stream
        .and_then(|stream| stream.get("height"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let duration_ms = parse_duration_ms(bytes);
    VideoMetadata {
        width,
        height,
        duration_ms,
    }
}

fn parse_duration_ms(bytes: &[u8]) -> Option<u64> {
    let value = serde_json::from_slice::<Value>(bytes).ok()?;
    let duration_seconds = value
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(|value| {
            value
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .or_else(|| value.as_f64())
        })?;
    duration_seconds
        .is_finite()
        .then_some(duration_seconds)
        .filter(|value| *value >= 0.0)
        .map(|value| (value * 1000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::parse_probe_json;

    #[test]
    fn parses_optional_video_metadata() {
        let metadata = parse_probe_json(
            br#"{"streams":[{"width":1280,"height":720}],"format":{"duration":"1.25"}}"#,
        );
        assert_eq!(metadata.width, Some(1280));
        assert_eq!(metadata.height, Some(720));
        assert_eq!(metadata.duration_ms, Some(1250));
    }
}
