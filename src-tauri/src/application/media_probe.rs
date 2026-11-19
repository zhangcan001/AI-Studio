use async_trait::async_trait;
use serde_json::Value;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{process::Command, time::timeout};

// A hung decoder must not prevent a serial import batch from returning its receipt.
const PROBE_BUDGET: Duration = Duration::from_secs(20);
const POSTER_BUDGET: Duration = Duration::from_secs(30);

async fn bounded_output(command: &mut Command, budget: Duration) -> Option<std::process::Output> {
    command.kill_on_drop(true);
    command.stdin(Stdio::null());
    command.stderr(Stdio::null());
    match timeout(budget, command.output()).await {
        Ok(Ok(output)) => Some(output),
        Ok(Err(error)) => {
            tracing::debug!(
                error_type = std::any::type_name_of_val(&error),
                "media probe process unavailable"
            );
            None
        }
        Err(_) => {
            tracing::warn!("media probe process exceeded its bounded execution budget");
            None
        }
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaProbeOutcome {
    Decodable,
    DecodeFailed,
    ProbeUnavailable,
}

#[async_trait]
pub trait MediaProbe: Send + Sync {
    async fn inspect_preview(&self, _path: &Path, _audio: bool) -> MediaProbeOutcome {
        MediaProbeOutcome::ProbeUnavailable
    }

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
    async fn inspect_preview(&self, path: &Path, audio: bool) -> MediaProbeOutcome {
        let mut command = Command::new(&self.ffprobe);
        command
            .args([
                "-v",
                "error",
                "-select_streams",
                if audio { "a:0" } else { "v:0" },
                "-show_entries",
                "stream=codec_type",
                "-of",
                "json",
            ])
            .arg(path);
        match bounded_output(&mut command, PROBE_BUDGET).await {
            Some(output) if output.status.success() => {
                let expected = if audio { "audio" } else { "video" };
                if serde_json::from_slice::<serde_json::Value>(&output.stdout)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("streams")
                            .and_then(|streams| streams.as_array())
                            .map(|streams| {
                                streams.iter().any(|stream| {
                                    stream.get("codec_type").and_then(|kind| kind.as_str())
                                        == Some(expected)
                                })
                            })
                    })
                    .unwrap_or(false)
                {
                    MediaProbeOutcome::Decodable
                } else {
                    MediaProbeOutcome::DecodeFailed
                }
            }
            Some(_) => MediaProbeOutcome::DecodeFailed,
            None => MediaProbeOutcome::ProbeUnavailable,
        }
    }

    async fn probe_video(&self, path: &Path) -> VideoMetadata {
        let mut command = Command::new(&self.ffprobe);
        command
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,duration,duration_ts,time_base:stream_tags=DURATION,duration",
                "-of",
                "json",
            ])
            .arg(path);
        let Some(output) = bounded_output(&mut command, PROBE_BUDGET).await else {
            return VideoMetadata::default();
        };
        if !output.status.success() {
            tracing::debug!("ffprobe could not inspect video");
            return VideoMetadata::default();
        }
        parse_probe_json(&output.stdout)
    }

    async fn generate_video_poster(&self, path: &Path) -> Option<Vec<u8>> {
        let mut command = Command::new(&self.ffmpeg);
        command
            .args(["-nostdin", "-v", "error", "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-f",
                "image2pipe",
                "-vcodec",
                "png",
                "pipe:1",
            ]);
        let Some(output) = bounded_output(&mut command, POSTER_BUDGET).await else {
            return None;
        };
        if output.status.success() && !output.stdout.is_empty() {
            Some(output.stdout)
        } else {
            tracing::debug!("ffmpeg did not produce a video poster");
            None
        }
    }

    async fn probe_audio(&self, path: &Path) -> AudioMetadata {
        let mut command = Command::new(&self.ffprobe);
        command
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "format=duration:stream=codec_type",
                "-of",
                "json",
            ])
            .arg(path);
        let Some(output) = bounded_output(&mut command, PROBE_BUDGET).await else {
            return AudioMetadata::default();
        };
        if !output.status.success() {
            tracing::debug!("ffprobe could not inspect audio");
            return AudioMetadata::default();
        }
        if !serde_json::from_slice::<Value>(&output.stdout)
            .ok()
            .and_then(|v| v.get("streams").cloned())
            .and_then(|s| s.as_array().cloned())
            .is_some_and(|streams| streams.iter().any(|s| s["codec_type"] == "audio"))
        {
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
    // Container duration may include a longer audio track. Video admission
    // must use the selected video stream only; unknown duration stays unknown.
    let seconds = stream
        .and_then(|s| s.get("duration"))
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| v.as_f64())
        })
        .or_else(|| {
            let stream = stream?;
            let ticks = stream.get("duration_ts")?.as_i64()? as f64;
            let (n, d) = stream.get("time_base")?.as_str()?.split_once('/')?;
            let numerator = n.parse::<f64>().ok()?;
            let denominator = d.parse::<f64>().ok()?;
            (denominator > 0.0 && numerator > 0.0).then_some(ticks * numerator / denominator)
        })
        .or_else(|| {
            // Matroska commonly exposes track duration as a stream tag rather
            // than duration_ts. Never read the format/container DURATION tag.
            let tags = stream?.get("tags")?;
            let value = tags
                .get("DURATION")
                .or_else(|| tags.get("duration"))?
                .as_str()?;
            let parts: Vec<_> = value.split(':').collect();
            if parts.len() != 3 {
                return None;
            }
            let hours = parts[0].parse::<u64>().ok()?;
            let minutes = parts[1].parse::<u64>().ok()?;
            let seconds = parts[2].parse::<f64>().ok()?;
            (minutes < 60 && seconds.is_finite() && (0.0..60.0).contains(&seconds))
                .then_some(hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds)
        });
    let duration_ms = seconds
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| (v * 1000.0).round() as u64);
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

    #[tokio::test]
    async fn hung_probe_returns_without_waiting_for_child() {
        use super::*;
        #[cfg(windows)]
        let mut command = {
            let mut cmd = Command::new("powershell.exe");
            cmd.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 8",
            ]);
            cmd
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", "sleep 8"]);
            cmd
        };
        assert!(bounded_output(&mut command, Duration::from_millis(500))
            .await
            .is_none());
    }

    #[tokio::test]
    async fn typed_probe_spawn_failure_is_unavailable_and_default_fake_does_not_claim_success() {
        use super::*;
        let probe = CommandMediaProbe {
            ffprobe: "ai-studio-owned-nonexistent-probe-fixture".into(),
            ffmpeg: "unused".into(),
        };
        assert_eq!(
            probe
                .inspect_preview(Path::new("owned-unused"), false)
                .await,
            MediaProbeOutcome::ProbeUnavailable
        );
        assert_eq!(
            probe.inspect_preview(Path::new("owned-unused"), true).await,
            MediaProbeOutcome::ProbeUnavailable
        );
        struct Fake;
        #[async_trait]
        impl MediaProbe for Fake {
            async fn probe_video(&self, _: &Path) -> VideoMetadata {
                VideoMetadata::default()
            }
            async fn generate_video_poster(&self, _: &Path) -> Option<Vec<u8>> {
                None
            }
        }
        assert_eq!(
            Fake.inspect_preview(Path::new("owned-unused"), false).await,
            MediaProbeOutcome::ProbeUnavailable
        );
    }

    #[test]
    fn parses_optional_video_metadata() {
        let metadata = parse_probe_json(
            br#"{"streams":[{"width":1280,"height":720,"duration":"1.25"}],"format":{"duration":"4.0"}}"#,
        );
        assert_eq!(metadata.width, Some(1280));
        assert_eq!(metadata.height, Some(720));
        assert_eq!(metadata.duration_ms, Some(1250));
    }
    #[test]
    fn r32_matroska_uses_selected_video_track_tag_not_longer_container_audio() {
        let metadata = parse_probe_json(br#"{"streams":[{"width":16,"height":16,"tags":{"DURATION":"00:00:01.250000000"}}],"format":{"duration":"4.0"}}"#);
        assert_eq!(metadata.duration_ms, Some(1250));
        let malformed = parse_probe_json(
            br#"{"streams":[{"tags":{"DURATION":"00:99:01.0"}}],"format":{"duration":"4.0"}}"#,
        );
        assert_eq!(malformed.duration_ms, None);
    }
    #[test]
    fn r32_video_duration_never_inherits_longer_audio_container_duration() {
        let metadata = parse_probe_json(br#"{"streams":[{"width":16,"height":16,"duration":"1.0"}],"format":{"duration":"4.0"}}"#);
        assert_eq!(metadata.duration_ms, Some(1000));
        let unknown = parse_probe_json(
            br#"{"streams":[{"width":16,"height":16}],"format":{"duration":"4.0"}}"#,
        );
        assert_eq!(unknown.duration_ms, None);
    }
}
