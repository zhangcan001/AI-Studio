use super::*;
use crate::application::ports::{
    ExternalAssetImportRecord, ShotVideoInputAsset, ShotVideoInputScope, ShotVideoInputSet,
    ShotVideoInputToken,
};
use crate::infrastructure::database::repositories::{map_sqlx_error, parse_datetime};

pub(super) async fn export(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<(Vec<ShotVideoInputSet>, Vec<ExternalAssetImportRecord>), RepositoryError> {
    let headers = sqlx::query("SELECT * FROM shot_video_input_sets WHERE project_id = ? ORDER BY shot_id, workflow_version_id, recipe_id")
        .bind(project_id).fetch_all(&mut **tx).await.map_err(map_sqlx_error)?;
    let children = sqlx::query("SELECT * FROM shot_video_input_assets WHERE project_id = ? ORDER BY shot_id, workflow_version_id, recipe_id, input_key, ordinal")
        .bind(project_id).fetch_all(&mut **tx).await.map_err(map_sqlx_error)?;
    let mut grouped = HashMap::<(String, String, String), Vec<ShotVideoInputAsset>>::new();
    for child in children {
        let key = (
            child.try_get("shot_id").map_err(map_sqlx_error)?,
            child
                .try_get("workflow_version_id")
                .map_err(map_sqlx_error)?,
            child.try_get("recipe_id").map_err(map_sqlx_error)?,
        );
        grouped.entry(key).or_default().push(ShotVideoInputAsset {
            input_key: child.try_get("input_key").map_err(map_sqlx_error)?,
            ordinal: child.try_get("ordinal").map_err(map_sqlx_error)?,
            asset_id: child.try_get("asset_id").map_err(map_sqlx_error)?,
        });
    }
    let mut sets = Vec::with_capacity(headers.len());
    for header in headers {
        let scope = ShotVideoInputScope {
            project_id: project_id.into(),
            shot_id: header.try_get("shot_id").map_err(map_sqlx_error)?,
            workflow_version_id: header
                .try_get("workflow_version_id")
                .map_err(map_sqlx_error)?,
            recipe_id: header.try_get("recipe_id").map_err(map_sqlx_error)?,
        };
        let inputs = grouped
            .remove(&(
                scope.shot_id.clone(),
                scope.workflow_version_id.clone(),
                scope.recipe_id.clone(),
            ))
            .unwrap_or_default();
        sets.push(ShotVideoInputSet {
            scope,
            inputs,
            token: ShotVideoInputToken {
                instance_id: header.try_get("instance_id").map_err(map_sqlx_error)?,
                revision: header.try_get("revision").map_err(map_sqlx_error)?,
            },
            updated_at: parse_datetime(
                "video input updated_at",
                &header
                    .try_get::<String, _>("updated_at")
                    .map_err(map_sqlx_error)?,
            )?,
        });
    }
    let rows =
        sqlx::query("SELECT * FROM external_asset_imports WHERE project_id = ? ORDER BY asset_id")
            .bind(project_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(map_sqlx_error)?;
    let mut receipts = Vec::with_capacity(rows.len());
    for row in rows {
        let unsigned = |key| -> Result<u64, RepositoryError> {
            u64::try_from(row.try_get::<i64, _>(key).map_err(map_sqlx_error)?)
                .map_err(|_| RepositoryError::integrity("negative import metadata"))
        };
        receipts.push(ExternalAssetImportRecord {
            asset_id: row.try_get("asset_id").map_err(map_sqlx_error)?,
            project_id: project_id.into(),
            media_type: row.try_get("media_type").map_err(map_sqlx_error)?,
            sha256: row.try_get("sha256").map_err(map_sqlx_error)?,
            mime_type: row.try_get("mime_type").map_err(map_sqlx_error)?,
            file_size: unsigned("file_size")?,
            width: u32::try_from(unsigned("width")?)
                .map_err(|_| RepositoryError::integrity("import width overflow"))?,
            height: u32::try_from(unsigned("height")?)
                .map_err(|_| RepositoryError::integrity("import height overflow"))?,
            duration_ms: row
                .try_get::<Option<i64>, _>("duration_ms")
                .map_err(map_sqlx_error)?
                .map(|v| {
                    u64::try_from(v)
                        .map_err(|_| RepositoryError::integrity("negative import duration"))
                })
                .transpose()?,
            imported_at: parse_datetime(
                "import imported_at",
                &row.try_get::<String, _>("imported_at")
                    .map_err(map_sqlx_error)?,
            )?,
        });
    }
    Ok((sets, receipts))
}

pub(super) async fn append_workflow_refs(
    tx: &mut Transaction<'_, Sqlite>,
    sets: &[ShotVideoInputSet],
    refs: &mut Vec<WorkflowReference>,
) -> Result<(), RepositoryError> {
    for set in sets {
        if refs.iter().any(|r| {
            r.workflow_version_id == set.scope.workflow_version_id
                && r.recipe_id == set.scope.recipe_id
        }) {
            continue;
        }
        let workflow_id: Option<String> =
            sqlx::query_scalar("SELECT workflow_id FROM workflow_versions WHERE id = ?")
                .bind(&set.scope.workflow_version_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(map_sqlx_error)?;
        if let Some(workflow_id) = workflow_id {
            refs.push(WorkflowReference {
                workflow_id,
                workflow_version_id: set.scope.workflow_version_id.clone(),
                recipe_id: set.scope.recipe_id.clone(),
            });
        }
        // Missing soft runtime identity is retained as stale, never guessed by name.
    }
    Ok(())
}

pub(super) async fn restore(
    tx: &mut Transaction<'_, Sqlite>,
    project: &ProjectRecord,
    doc: &BackupDocument,
    shot_ids: &HashMap<String, String>,
    asset_ids: &HashMap<String, String>,
    runtime_ids: &HashMap<String, String>,
) -> Result<(), RepositoryError> {
    let missing = || RepositoryError::integrity("video input backup identity mapping missing");
    for receipt in &doc.external_asset_imports {
        sqlx::query("INSERT INTO external_asset_imports (asset_id, project_id, media_type, sha256, mime_type, file_size, width, height, duration_ms, imported_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(asset_ids.get(&receipt.asset_id).ok_or_else(missing)?).bind(&project.id)
            .bind(&receipt.media_type).bind(&receipt.sha256).bind(&receipt.mime_type)
            .bind(i64::try_from(receipt.file_size).map_err(|_| missing())?).bind(i64::from(receipt.width)).bind(i64::from(receipt.height))
            .bind(receipt.duration_ms.map(i64::try_from).transpose().map_err(|_| missing())?)
            .bind(receipt.imported_at.to_rfc3339()).execute(&mut **tx).await.map_err(map_sqlx_error)?;
    }
    for set in &doc.shot_video_input_sets {
        let shot_id = shot_ids.get(&set.scope.shot_id).ok_or_else(missing)?;
        let wv = resolve_mapped_id(runtime_ids, &set.scope.workflow_version_id);
        let recipe = resolve_mapped_id(runtime_ids, &set.scope.recipe_id);
        sqlx::query("INSERT INTO shot_video_input_sets (project_id, shot_id, workflow_version_id, recipe_id, instance_id, revision, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&project.id).bind(shot_id).bind(&wv).bind(&recipe).bind(format!("vin_{}", Uuid::new_v4()))
            .bind(set.token.revision).bind(set.updated_at.to_rfc3339()).execute(&mut **tx).await.map_err(map_sqlx_error)?;
        for input in &set.inputs {
            sqlx::query("INSERT INTO shot_video_input_assets (project_id, shot_id, workflow_version_id, recipe_id, input_key, ordinal, asset_id) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&project.id).bind(shot_id).bind(&wv).bind(&recipe).bind(&input.input_key).bind(input.ordinal)
                .bind(asset_ids.get(&input.asset_id).ok_or_else(missing)?)
                .execute(&mut **tx).await.map_err(map_sqlx_error)?;
        }
    }
    Ok(())
}
