//! Explicit selected-video delivery. No database, Task or Queue writes.
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub struct ExportSource {
    pub ordinal: i64,
    pub shot_id: String,
    pub shot_name: String,
    pub asset_id: String,
    pub path: Option<(PathBuf, String, u64)>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntry {
    pub ordinal: i64,
    pub shot_id: String,
    pub shot_name: String,
    pub asset_id: String,
    pub file_name: Option<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportOptions {
    pub concat_video: bool,
    pub export_nle_timeline: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedVideoExport {
    pub exported: usize,
    pub failed: usize,
    pub directory: String,
    pub entries: Vec<ExportEntry>,
    pub manifest_error: Option<String>,
    pub open_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_video: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edl_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fcpxml_file: Option<String>,
    pub timeline_error: Option<String>,
}
/// Confirmation is tied to the exact current rejected set, not a blanket bypass.
pub fn check_review_confirmation(
    rejected: &[String],
    confirmed: Option<&[String]>,
) -> Result<(), super::error::ProductError> {
    use std::collections::BTreeSet;
    let actual = rejected.iter().collect::<BTreeSet<_>>();
    let expected = confirmed
        .unwrap_or_default()
        .iter()
        .collect::<BTreeSet<_>>();
    if actual == expected {
        return Ok(());
    }
    let mut error = super::error::ProductError::new(
        "EXPORT_REJECTED_CONFIRMATION_REQUIRED",
        "已选用视频包含已退回候选，请明确确认后导出。",
        None,
    );
    error.details.rejected_asset_ids = Some(rejected.to_vec());
    Err(error)
}

fn safe_name(value: &str) -> String {
    let name: String = value
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(60)
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    if name.is_empty() {
        "镜头".into()
    } else {
        name.into()
    }
}
/// Validated sources only; create a new owned directory, never overwrite user files.
pub fn copy_selected_videos(
    parent: &Path,
    sources: &[ExportSource],
) -> io::Result<SelectedVideoExport> {
    copy_selected_videos_with_options(parent, sources, ExportOptions::default())
}

pub fn copy_selected_videos_with_options(
    parent: &Path,
    sources: &[ExportSource],
    options: ExportOptions,
) -> io::Result<SelectedVideoExport> {
    if sources.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "没有已选用的视频",
        ));
    }
    let estimated_bytes: u64 = sources
        .iter()
        .filter_map(|source| source.path.as_ref().map(|(_, _, size)| *size))
        .sum();
    let required_bytes = if options.concat_video {
        estimated_bytes
            .saturating_mul(2)
            .saturating_add(32 * 1024 * 1024)
    } else {
        estimated_bytes.saturating_add(16 * 1024 * 1024)
    };
    if let Some((free_bytes, _)) = crate::infrastructure::filesystem::disk_space_bytes(parent) {
        if free_bytes < required_bytes {
            let need_mb = required_bytes / (1024 * 1024);
            let free_mb = free_bytes / (1024 * 1024);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("目标磁盘剩余空间不足（约需 {need_mb} MB，当前剩余 {free_mb} MB），请更换导出目录或清理磁盘。"),
            ));
        }
    }
    let directory = {
        let mut attempt = 0u32;
        loop {
            let short = &uuid::Uuid::new_v4().simple().to_string()[..8];
            let candidate = parent.join(format!("AI-Studio-selected-{short}"));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempt < 16 => {
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    };
    let mut entries = Vec::new();
    for source in sources {
        let mut entry = ExportEntry {
            ordinal: source.ordinal,
            shot_id: source.shot_id.clone(),
            shot_name: source.shot_name.clone(),
            asset_id: source.asset_id.clone(),
            file_name: None,
            error: source.error.clone(),
        };
        if entry.error.is_none() {
            let result = (|| -> io::Result<String> {
                let (path, sha256, size) = source
                    .path
                    .as_ref()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "source unavailable"))?;
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !["mp4", "webm", "mov", "mkv", "avi"].contains(&ext.as_str()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsupported video container",
                    ));
                }
                let file_name = format!(
                    "{:04}_{}.{}",
                    source.ordinal.saturating_add(1),
                    safe_name(&source.shot_name),
                    ext
                );
                let mut input = fs::File::open(path)?;
                let metadata = input.metadata()?;
                if !metadata.is_file() || metadata.len() == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
                }
                if metadata.len() != *size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "video size changed",
                    ));
                }
                publish_new_file(&directory, &file_name, |output| {
                    use sha2::{Digest, Sha256};
                    use std::io::{Read, Write};
                    let mut hash = Sha256::new();
                    let mut buffer = [0; 64 * 1024];
                    let mut copied = 0u64;
                    loop {
                        let count = input.read(&mut buffer)?;
                        if count == 0 {
                            break;
                        }
                        copied += count as u64;
                        if copied > *size {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "video size changed",
                            ));
                        }
                        hash.update(&buffer[..count]);
                        output.write_all(&buffer[..count])?;
                    }
                    if copied != *size
                        || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(sha256)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "video content changed",
                        ));
                    }
                    Ok(())
                })?;
                Ok(file_name)
            })();
            match result{Ok(name)=>entry.file_name=Some(name),Err(e)=>entry.error=Some(format!("视频复制失败（{:?}），请检查源文件、目录权限及剩余空间；同名未完成文件不可当作成功视频。",e.kind()))}
        }
        entries.push(entry);
    }
    let exported = entries
        .iter()
        .filter(|e| e.error.is_none() && e.file_name.is_some())
        .count();
    let manifest_result = publish_new_file(&directory, "export-manifest.json", |file| {
        serde_json::to_writer_pretty(
            file,
            &serde_json::json!({"format":"AI_STUDIO_SELECTED_VIDEOS_V1","entries":entries}),
        )
        .map_err(io::Error::from)
    });
    let exported_files: Vec<(&ExportEntry, String)> = entries
        .iter()
        .filter_map(|entry| {
            if entry.error.is_none() {
                entry.file_name.clone().map(|name| (entry, name))
            } else {
                None
            }
        })
        .collect();
    let (edl_file, fcpxml_file, timeline_error) =
        if options.export_nle_timeline && !exported_files.is_empty() {
            match write_nle_timelines(&directory, &exported_files) {
                Ok((edl, xml)) => (edl, xml, None),
                Err(error) => (None, None, Some(error)),
            }
        } else {
            (None, None, None)
        };
    let (merged_video, merged_error) = if options.concat_video && !exported_files.is_empty() {
        concat_exported_videos(&directory, &exported_files)
    } else {
        (None, None)
    };
    Ok(SelectedVideoExport {
        exported,
        failed: entries.len() - exported,
        directory: directory.to_string_lossy().into_owned(),
        entries,
        manifest_error: manifest_result
            .err()
            .map(|_| "导出映射清单写入失败；已复制视频保留，请勿误认为全部未导出。".into()),
        open_error: None,
        merged_video,
        merged_error,
        edl_file,
        fcpxml_file,
        timeline_error,
    })
}

fn configure_no_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

#[derive(Clone, Debug)]
struct ClipStreamInfo {
    width: u32,
    height: u32,
    has_audio: bool,
    duration: f64,
    copy_signature: Option<serde_json::Value>,
}

fn parse_clip_stream_info(value: &serde_json::Value) -> Result<ClipStreamInfo, String> {
    let duration = value["format"]["duration"]
        .as_str()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("无法确定视频真实时长，已停止生成时间线或合片")?;
    let streams = value["streams"].as_array().ok_or("缺少视频流信息")?;
    let video = streams
        .iter()
        .find(|v| v["codec_type"] == "video")
        .ok_or("未找到视频流")?;
    let width = video["width"]
        .as_u64()
        .filter(|v| (2..=16384).contains(v))
        .ok_or("视频宽度不可用")? as u32;
    let height = video["height"]
        .as_u64()
        .filter(|v| (2..=16384).contains(v))
        .ok_or("视频高度不可用")? as u32;
    let complete = streams.iter().all(|v| {
        v["codec_name"].is_string()
            && v["time_base"].is_string()
            && v["extradata_hash"].is_string()
            && matches!(v["codec_type"].as_str(), Some("video" | "audio"))
    });
    let copy_signature = complete.then(|| {
        serde_json::Value::Array(
            streams
                .iter()
                .map(|v| {
                    let keys = [
                        "codec_type",
                        "codec_name",
                        "profile",
                        "level",
                        "width",
                        "height",
                        "pix_fmt",
                        "time_base",
                        "avg_frame_rate",
                        "extradata_hash",
                        "sample_rate",
                        "channels",
                        "channel_layout",
                        "color_range",
                        "color_space",
                        "color_transfer",
                        "color_primaries",
                    ];
                    serde_json::Value::Object(
                        keys.into_iter()
                            .map(|key| (key.to_owned(), v[key].clone()))
                            .collect(),
                    )
                })
                .collect(),
        )
    });
    Ok(ClipStreamInfo {
        width: width & !1,
        height: height & !1,
        has_audio: streams.iter().any(|v| v["codec_type"] == "audio"),
        duration,
        copy_signature,
    })
}

fn probe_clip_stream_info(path: &Path) -> Result<ClipStreamInfo, String> {
    let mut command = std::process::Command::new("ffprobe");
    configure_no_window(&mut command);
    let output = command
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-show_data_hash",
            "sha256",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|_| {
            "未找到或无法启动 ffprobe；单镜视频已保留，未生成不可靠的时间线或合片".to_owned()
        })?;
    if !output.status.success() {
        return Err("视频信息探测失败，未生成不可靠的时间线或合片".into());
    }
    let value =
        serde_json::from_slice(&output.stdout).map_err(|_| "视频信息解析失败".to_owned())?;
    parse_clip_stream_info(&value)
}

fn streams_allow_copy(infos: &[ClipStreamInfo]) -> bool {
    infos.first().is_some_and(|first| {
        first.copy_signature.is_some()
            && infos
                .iter()
                .all(|info| info.copy_signature == first.copy_signature)
    })
}

fn format_edl_tc(seconds: f64, fps: u32) -> String {
    let total_frames = (seconds.max(0.0) * f64::from(fps)).round() as u64;
    let f = total_frames % u64::from(fps);
    let total_secs = total_frames / u64::from(fps);
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    format!("{h:02}:{m:02}:{s:02}:{f:02}")
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn write_nle_timelines(
    directory: &Path,
    exported_files: &[(&ExportEntry, String)],
) -> Result<(Option<String>, Option<String>), String> {
    let infos = exported_files
        .iter()
        .map(|(_, name)| probe_clip_stream_info(&directory.join(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let (edl, fcpxml) = nle_documents(directory, exported_files, &infos)?;
    publish_new_file(directory, "timeline.edl", |file| {
        use std::io::Write;
        file.write_all(edl.as_bytes())
    })
    .map_err(|_| "无法写入 EDL 时间线".to_owned())?;
    publish_new_file(directory, "timeline.fcpxml", |file| {
        use std::io::Write;
        file.write_all(fcpxml.as_bytes())
    })
    .map_err(|_| "无法写入 FCPXML 时间线".to_owned())?;
    Ok((Some("timeline.edl".into()), Some("timeline.fcpxml".into())))
}

fn nle_documents(
    directory: &Path,
    exported_files: &[(&ExportEntry, String)],
    infos: &[ClipStreamInfo],
) -> Result<(String, String), String> {
    let fps = 24u32;
    let durations: Vec<f64> = infos.iter().map(|info| info.duration).collect();
    let mut edl = String::from("TITLE: AI Studio Timeline\nFCM: NON-DROP FRAME\n\n");
    let mut rec_cursor = 0.0f64;
    for (idx, ((entry, file_name), duration)) in
        exported_files.iter().zip(durations.iter()).enumerate()
    {
        let event_num = idx + 1;
        let src_in = format_edl_tc(0.0, fps);
        let src_out = format_edl_tc(*duration, fps);
        let rec_in = format_edl_tc(rec_cursor, fps);
        rec_cursor += *duration;
        let rec_out = format_edl_tc(rec_cursor, fps);
        edl.push_str(&format!(
            "{event_num:03}  AX       V     C        {src_in} {src_out} {rec_in} {rec_out}\n* FROM CLIP NAME: {file_name}\n* COMMENT: {}\n\n",
            entry.shot_name.replace(['\r', '\n'], " ")
        ));
    }
    let total_frames = (rec_cursor * f64::from(fps)).round() as u64;
    let mut assets_xml = String::new();
    let mut clips_xml = String::new();
    let mut offset_frames = 0u64;
    for (idx, ((entry, file_name), duration)) in
        exported_files.iter().zip(durations.iter()).enumerate()
    {
        let asset_id = format!("r{}", idx + 2);
        let clip_frames = (*duration * f64::from(fps)).round().max(1.0) as u64;
        let abs_src = directory.join(file_name);
        let file_url = url::Url::from_file_path(&abs_src)
            .map_err(|_| "剪辑媒体路径不是有效的绝对路径".to_owned())?;
        assets_xml.push_str(&format!(
            "        <asset id=\"{asset_id}\" name=\"{}\" start=\"0s\" duration=\"{clip_frames}/{fps}s\" hasVideo=\"1\" hasAudio=\"{}\"><media-rep kind=\"original-media\" src=\"{}\"/></asset>\n",
            xml_escape(&entry.shot_name),
            if infos[idx].has_audio { "1" } else { "0" },
            xml_escape(file_url.as_str())
        ));
        clips_xml.push_str(&format!(
            "                        <asset-clip name=\"{}\" ref=\"{asset_id}\" offset=\"{offset_frames}/{fps}s\" duration=\"{clip_frames}/{fps}s\" start=\"0s\"/>\n",
            xml_escape(&entry.shot_name)
        ));
        offset_frames += clip_frames;
    }
    let fcpxml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.9\">\n    <resources>\n        <format id=\"r1\" name=\"FFVideoFormat1080p24\" frameDuration=\"1/{fps}s\" width=\"1920\" height=\"1080\"/>\n{assets_xml}    </resources>\n    <library>\n        <event name=\"AI Studio Export\">\n            <project name=\"AI Studio Timeline\">\n                <sequence format=\"r1\" duration=\"{total_frames}/{fps}s\" tcStart=\"0s\" tcFormat=\"NDF\">\n                    <spine>\n{clips_xml}                    </spine>\n                </sequence>\n            </project>\n        </event>\n    </library>\n</fcpxml>\n"
    );
    Ok((edl, fcpxml))
}

fn concat_exported_videos(
    directory: &Path,
    exported_files: &[(&ExportEntry, String)],
) -> (Option<String>, Option<String>) {
    let list_path = directory.join(".concat-list.txt");
    let mut list_content = String::new();
    for (_, file_name) in exported_files {
        let escaped = file_name.replace('\'', "'\\''");
        list_content.push_str(&format!("file '{escaped}'\n"));
    }
    if let Err(error) = fs::write(&list_path, list_content) {
        return (None, Some(format!("无法写入 FFmpeg 合片清单：{error}")));
    }
    let merged_name = "AI-Studio-merged.mp4";
    let merged_path = directory.join(merged_name);
    let infos = exported_files
        .iter()
        .map(|(_, file_name)| probe_clip_stream_info(&directory.join(file_name)))
        .collect::<Result<Vec<_>, _>>();
    let infos = match infos {
        Ok(infos) => infos,
        Err(error) => {
            let _ = fs::remove_file(&list_path);
            return (None, Some(error));
        }
    };
    let uniform_streams = streams_allow_copy(&infos);

    if uniform_streams {
        let mut copy_cmd = std::process::Command::new("ffmpeg");
        configure_no_window(&mut copy_cmd);
        let copy_attempt = copy_cmd
            .current_dir(directory)
            .args([
                "-y",
                "-nostdin",
                "-v",
                "error",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
                ".concat-list.txt",
                "-c",
                "copy",
                "-movflags",
                "+faststart",
                merged_name,
            ])
            .output();
        match copy_attempt {
            Ok(output) if output.status.success() && merged_path.is_file() => {
                let _ = fs::remove_file(&list_path);
                return (Some(merged_name.to_owned()), None);
            }
            Err(_) => {
                let _ = fs::remove_file(&list_path);
                return (
                    None,
                    Some("未检测到系统 ffmpeg，已保留全部单镜视频与剪辑时间线。".into()),
                );
            }
            Ok(_) => {
                let _ = fs::remove_file(&merged_path);
            }
        }
    }

    // Normalized commercial fallback: scale+pad every clip to the first clip's dimensions
    // and synthesize silence for silent clips whenever any clip in the timeline has audio.
    let target_w = infos.first().map(|i| i.width).unwrap_or(1280);
    let target_h = infos.first().map(|i| i.height).unwrap_or(720);
    let any_audio = infos.iter().any(|i| i.has_audio);
    let mut cmd = std::process::Command::new("ffmpeg");
    configure_no_window(&mut cmd);
    cmd.current_dir(directory).args([
        "-y",
        "-nostdin",
        "-v",
        "error",
        "-filter_complex_threads",
        "1",
    ]);
    for (_, file_name) in exported_files {
        cmd.args(["-threads", "1", "-i"]).arg(file_name);
    }
    let mut filter = String::new();
    let mut concat_inputs = String::new();
    for (idx, info) in infos.iter().enumerate() {
        filter.push_str(&format!(
            "[{idx}:v]scale={target_w}:{target_h}:force_original_aspect_ratio=decrease,pad={target_w}:{target_h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=24,format=yuv420p[v{idx}];"
        ));
        if any_audio {
            if info.has_audio {
                filter.push_str(&format!(
                    "[{idx}:a]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo[a{idx}];"
                ));
            } else {
                filter.push_str(&format!(
                    "anullsrc=channel_layout=stereo:sample_rate=48000,atrim=0:{:.3}[a{idx}];",
                    info.duration.max(0.1)
                ));
            }
            concat_inputs.push_str(&format!("[v{idx}][a{idx}]"));
        } else {
            concat_inputs.push_str(&format!("[v{idx}]"));
        }
    }
    let n = exported_files.len();
    if any_audio {
        filter.push_str(&format!("{concat_inputs}concat=n={n}:v=1:a=1[vout][aout]"));
        cmd.args([
            "-filter_complex",
            &filter,
            "-map",
            "[vout]",
            "-map",
            "[aout]",
        ]);
        cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
    } else {
        filter.push_str(&format!("{concat_inputs}concat=n={n}:v=1:a=0[vout]"));
        cmd.args(["-filter_complex", &filter, "-map", "[vout]"]);
        cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
    }
    cmd.args(["-threads", "2", "-movflags", "+faststart", merged_name]);
    let result = match cmd.output() {
        Ok(output) if output.status.success() && merged_path.is_file() => {
            (Some(merged_name.to_owned()), None)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let _ = fs::remove_file(&merged_path);
            (
                None,
                Some(if stderr.is_empty() {
                    "FFmpeg 合片失败，请检查各镜头编码或单独使用导出的分镜视频。".into()
                } else {
                    format!("FFmpeg 合片失败：{stderr}")
                }),
            )
        }
        Err(_) => (
            None,
            Some("未检测到系统 ffmpeg，已保留全部单镜视频与剪辑时间线。".into()),
        ),
    };
    let _ = fs::remove_file(&list_path);
    result
}
fn publish_new_file(
    directory: &Path,
    name: &str,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let short = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let temporary = directory.join(format!(".{short}.part"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| {
        let destination = directory.join(name);
        if destination.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "export file already exists",
            ));
        }
        fs::rename(&temporary, destination)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
/// Identity of the videos actually selected for export. Unselected shots never
/// participate; a name or ordinal change of a selected shot is a new identity.
pub fn export_selection_identity<'a, I>(
    selected: I,
) -> std::collections::BTreeMap<String, (String, String, i64)>
where
    I: IntoIterator<Item = (&'a str, &'a str, &'a str, i64)>,
{
    selected
        .into_iter()
        .map(|(shot_id, asset_id, name, ordinal)| {
            (
                shot_id.to_owned(),
                (asset_id.to_owned(), name.to_owned(), ordinal),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_export_requires_exact_fresh_confirmation() {
        let rejected = vec!["a".to_owned()];
        assert!(check_review_confirmation(&rejected, None).is_err());
        assert!(check_review_confirmation(&rejected, Some(&rejected)).is_ok());
        assert!(check_review_confirmation(&["a".into(), "b".into()], Some(&rejected)).is_err());
        assert!(check_review_confirmation(&[], Some(&rejected)).is_err());
        assert!(check_review_confirmation(&[], None).is_ok());
    }
    #[test]
    fn export_command_uses_nonblocking_native_picker_and_revalidates_before_copy() {
        let source = include_str!("../../commands/product.rs");
        let command = source
            .split("pub async fn product_selected_videos_export(")
            .nth(1)
            .unwrap()
            .split("#[tauri::command")
            .next()
            .unwrap();
        assert!(!command.contains("blocking_pick_folder"));
        assert!(command.contains(".pick_folder("));
        assert!(command.contains("rx.await"));
        assert_eq!(command.matches("check_review_confirmation(").count(), 2);
        assert_eq!(command.matches("selected_video_export_sources(").count(), 2);
        assert!(
            command.find("current_selection != selection").unwrap()
                < command.find("copy_selected_videos_with_options(").unwrap()
        );
        assert!(
            command
                .find("selected_video_export_sources(&state, &project_id, &current_shots)")
                .unwrap()
                > command.find("current_selection != selection").unwrap()
        );
        assert!(command.contains("(asset.clone(), s.name.clone(), s.ordinal)"));
    }
    #[test]
    fn r57_r58_selection_identity_includes_name_and_ordinal_but_ignores_unselected_shots() {
        let selected = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let renamed = export_selection_identity([
            ("shot-a", "asset-a", "新名称", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let reordered = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 1),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        let unselected_tail_changed = export_selection_identity([
            ("shot-a", "asset-a", "镜头一", 0),
            ("shot-c", "asset-c", "镜头三", 2),
        ]);
        assert_ne!(selected, renamed);
        assert_ne!(selected, reordered);
        assert_eq!(selected, unselected_tail_changed);
        assert!(!selected.contains_key("shot-b"));
    }

    fn source(ordinal: i64, name: &str, path: PathBuf) -> ExportSource {
        ExportSource {
            ordinal,
            shot_id: format!("shot-{ordinal}"),
            shot_name: name.into(),
            asset_id: format!("asset-{ordinal}"),
            path: {
                use sha2::{Digest, Sha256};
                let bytes = fs::read(&path).unwrap_or_default();
                Some((
                    path,
                    format!("{:x}", Sha256::digest(&bytes)),
                    bytes.len() as u64,
                ))
            },
            error: None,
        }
    }
    #[test]
    fn selected_video_delivery_preserves_shot_identity_bytes_and_private_path_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("source-a.mp4");
        let b = dir.path().join("source-b.webm");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let result = copy_selected_videos(
            dir.path(),
            &[source(2, "镜头三", a.clone()), source(8, "镜头九", b)],
        )
        .unwrap();
        assert_eq!(result.exported, 2);
        assert_eq!(result.failed, 0);
        let root = Path::new(&result.directory);
        assert_eq!(fs::read(root.join("0003_镜头三.mp4")).unwrap(), b"a");
        assert_eq!(fs::read(root.join("0009_镜头九.webm")).unwrap(), b"b");
        let text = fs::read_to_string(root.join("export-manifest.json")).unwrap();
        assert!(text.contains("shot-8"));
        assert!(!text.contains(&dir.path().to_string_lossy().to_string()));
        let next = copy_selected_videos(dir.path(), &[source(2, "镜头三", a.clone())]).unwrap();
        assert_ne!(result.directory, next.directory);
        assert_eq!(fs::read(a).unwrap(), b"a");
    }
    #[test]
    fn selected_video_delivery_reports_each_failure_and_sanitizes_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.mp4");
        fs::write(&a, b"data").unwrap();
        let result = copy_selected_videos(
            dir.path(),
            &[
                source(0, "../C:\"bad*", a),
                source(9, "missing", dir.path().join("missing.mp4")),
            ],
        )
        .unwrap();
        assert_eq!(result.exported, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.entries[1].shot_id, "shot-9");
        assert!(result.entries[1].error.is_some());
        let name = result.entries[0].file_name.as_ref().unwrap();
        assert!(!name.contains('/'));
        assert!(!name.contains(':'));
        assert!(!name.contains('*'));
        assert!(copy_selected_videos(dir.path(), &[]).is_err());
    }
    #[test]
    fn rejects_replaced_video_and_removes_partial_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("video.mp4");
        fs::write(&path, b"original").unwrap();
        let item = source(0, "镜头", path.clone());
        fs::write(&path, b"replaced").unwrap();
        let receipt = copy_selected_videos(dir.path(), &[item]).unwrap();
        assert_eq!((receipt.exported, receipt.failed), (0, 1));
        let names: Vec<_> = fs::read_dir(receipt.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["export-manifest.json"]);
    }
    #[test]
    fn r23_empty_video_is_a_failed_manifest_entry_not_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.mp4");
        fs::write(&path, []).unwrap();
        let receipt = copy_selected_videos(dir.path(), &[source(0, "空视频", path)]).unwrap();
        assert_eq!(receipt.exported, 0);
        assert_eq!(receipt.failed, 1);
        assert!(receipt.entries[0].error.is_some());
        assert!(receipt.entries[0].file_name.is_none());
    }
    fn stream_fixture(codec: &str) -> serde_json::Value {
        serde_json::json!({"format":{"duration":"4.0"},"streams":[{"codec_type":"video","codec_name":codec,"width":128,"height":72,"time_base":"1/12288","extradata_hash":"SHA256:fixture","pix_fmt":"yuv420p"}]})
    }
    #[test]
    fn stream_copy_requires_codec_configuration_compatibility_not_just_dimensions() {
        let first = parse_clip_stream_info(&stream_fixture("h264")).unwrap();
        let other = parse_clip_stream_info(&stream_fixture("mpeg4")).unwrap();
        assert!(!streams_allow_copy(&[first.clone(), other]));
        assert!(streams_allow_copy(&[first.clone(), first]));
        let mut missing = stream_fixture("h264");
        missing["streams"][0]
            .as_object_mut()
            .unwrap()
            .remove("extradata_hash");
        assert!(!streams_allow_copy(&[
            parse_clip_stream_info(&missing).unwrap()
        ]));
    }
    #[test]
    fn missing_or_invalid_duration_is_an_error_never_a_fabricated_five_seconds() {
        for duration in ["", "N/A", "NaN", "0", "-2"] {
            let mut value = stream_fixture("h264");
            value["format"]["duration"] = serde_json::json!(duration);
            assert!(parse_clip_stream_info(&value).is_err());
        }
    }
    #[test]
    fn fcpxml_19_uses_media_representations_escaped_urls_and_actual_clip_lengths() {
        let temp = tempfile::tempdir().unwrap();
        let entry = ExportEntry {
            ordinal: 0,
            shot_id: "s".into(),
            shot_name: "shot".into(),
            asset_id: "a".into(),
            file_name: None,
            error: None,
        };
        let files = vec![
            (&entry, "one #1.mp4".into()),
            (&entry, "two.mp4".into()),
            (&entry, "three.mp4".into()),
        ];
        let (edl, xml) = nle_documents(
            temp.path(),
            &files,
            &[4.0, 6.0, 8.0]
                .into_iter()
                .enumerate()
                .map(|(i, duration)| ClipStreamInfo {
                    width: 128,
                    height: 72,
                    has_audio: i == 1,
                    duration,
                    copy_signature: None,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(xml.contains("<media-rep kind=\"original-media\" src=\"file:///"));
        assert!(xml.contains("one%20%231.mp4"));
        assert!(xml.contains("duration=\"432/24s\""));
        assert!(edl.contains("00:00:18:00"));
        assert!(xml.contains("hasAudio=\"1\""));
        assert!(xml.contains("hasAudio=\"0\""));
    }
}
