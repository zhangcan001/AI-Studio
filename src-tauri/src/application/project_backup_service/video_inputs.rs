use super::{AppError, BackupDocument};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(super) fn validate(document: &BackupDocument, version: u32) -> Result<(), AppError> {
    let invalid = || AppError::backup_invalid("备份正式视频输入或外部导入凭据无效");
    if version < 21
        && (!document.shot_video_input_sets.is_empty()
            || !document.external_asset_imports.is_empty())
    {
        return Err(invalid());
    }
    let assets = document
        .assets
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect::<HashMap<_, _>>();
    let shots = document
        .shots
        .iter()
        .map(|s| s.id.as_str())
        .collect::<HashSet<_>>();
    let mut receipts = HashSet::new();
    for receipt in &document.external_asset_imports {
        let asset = assets.get(receipt.asset_id.as_str()).ok_or_else(invalid)?;
        if receipt.project_id != document.project.id
            || !receipts.insert(receipt.asset_id.as_str())
            || asset.source_task_id.is_some()
            || asset.category != format!("source_{}", receipt.media_type)
            || !matches!(receipt.media_type.as_str(), "image" | "video" | "audio")
            || asset.asset_type != receipt.media_type
            || asset.sha256 != receipt.sha256
            || asset.mime_type != receipt.mime_type
            || asset.file_size <= 0
            || asset.file_size as u64 != receipt.file_size
            || u32::try_from(asset.width).ok() != Some(receipt.width)
            || u32::try_from(asset.height).ok() != Some(receipt.height)
            || asset.duration_ms.map(u64::try_from).transpose().ok() != Some(receipt.duration_ms)
            || !receipt
                .mime_type
                .starts_with(&format!("{}/", receipt.media_type))
            || receipt.sha256.len() != 64
            || !receipt
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid());
        }
    }
    let mut pairs = HashSet::new();
    for set in &document.shot_video_input_sets {
        let scope = &set.scope;
        if scope.project_id != document.project.id
            || !shots.contains(scope.shot_id.as_str())
            || scope.workflow_version_id.trim().is_empty()
            || scope.recipe_id.trim().is_empty()
            || set.token.instance_id.trim().is_empty()
            || set.token.revision < 1
            || !pairs.insert((&scope.shot_id, &scope.workflow_version_id, &scope.recipe_id))
        {
            return Err(invalid());
        }
        let mut ordinals: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for input in &set.inputs {
            let asset = assets.get(input.asset_id.as_str()).ok_or_else(invalid)?;
            let kind = match input.input_key.as_str() {
                "first_frame" | "last_frame" | "reference_images" => "image",
                "reference_videos" => "video",
                "reference_audios" => "audio",
                _ => return Err(invalid()),
            };
            if asset.asset_type != kind
                || (kind == "image" && !receipts.contains(input.asset_id.as_str()))
            {
                return Err(invalid());
            }
            ordinals
                .entry(&input.input_key)
                .or_default()
                .push(input.ordinal);
        }
        for (key, mut values) in ordinals {
            values.sort_unstable();
            if values.iter().enumerate().any(|(i, v)| *v != i as i64)
                || (matches!(key, "first_frame" | "last_frame") && values.len() != 1)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
