use crate::{
    app_state::AppState,
    application::ports::{ComfyEnvironmentProfile, RuntimeParameterProfile, WorkspaceResume},
    error::AppError,
};

#[tauri::command(rename_all = "camelCase")]
pub fn comfy_environment_profiles_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ComfyEnvironmentProfile>, AppError> {
    Ok(state.system.settings.comfy_environment_profiles())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_environment_profile_save(
    state: tauri::State<'_, AppState>,
    profile: ComfyEnvironmentProfile,
) -> Result<ComfyEnvironmentProfile, AppError> {
    state
        .system
        .settings
        .save_comfy_environment_profile(profile)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_environment_profile_delete(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), AppError> {
    state
        .system
        .settings
        .delete_comfy_environment_profile(&profile_id)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn comfy_environment_profile_apply(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<crate::application::settings_service::SettingsView, AppError> {
    state
        .system
        .settings
        .apply_comfy_environment_profile(&profile_id)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub fn runtime_profiles_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RuntimeParameterProfile>, AppError> {
    Ok(state.system.settings.runtime_profiles())
}

#[tauri::command(rename_all = "camelCase")]
pub fn production_queue_name_presets_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    Ok(state.system.settings.production_queue_name_presets())
}

#[tauri::command(rename_all = "camelCase")]
pub fn workspace_resume_get(state: tauri::State<'_, AppState>) -> WorkspaceResume {
    state.system.settings.workspace_resume()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn workspace_resume_save(
    state: tauri::State<'_, AppState>,
    workspace_resume: WorkspaceResume,
) -> Result<WorkspaceResume, AppError> {
    state
        .system
        .settings
        .save_workspace_resume(workspace_resume)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_queue_name_preset_save(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Vec<String>, AppError> {
    state
        .system
        .settings
        .save_production_queue_name_preset(&name)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn production_queue_name_preset_delete(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    state
        .system
        .settings
        .delete_production_queue_name_preset(&name)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn runtime_profiles_save(
    state: tauri::State<'_, AppState>,
    profile: RuntimeParameterProfile,
) -> Result<RuntimeParameterProfile, AppError> {
    state.system.settings.save_runtime_profile(profile).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn runtime_profiles_delete(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), AppError> {
    state
        .system
        .settings
        .delete_runtime_profile(&profile_id)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub fn desktop_preferences_get(
    state: tauri::State<'_, AppState>,
) -> crate::application::ports::DesktopPreferences {
    state.system.settings.desktop_preferences()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn desktop_preferences_save(
    state: tauri::State<'_, AppState>,
    preferences: crate::application::ports::DesktopPreferences,
) -> Result<crate::application::ports::DesktopPreferences, AppError> {
    state
        .system
        .settings
        .save_desktop_preferences(preferences)
        .await
}

#[tauri::command(rename_all = "camelCase")]
pub fn app_set_unsaved_draft(state: tauri::State<'_, AppState>, dirty: bool) {
    state
        .system
        .unsaved_draft
        .store(dirty, std::sync::atomic::Ordering::SeqCst);
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStorageUsageView {
    pub project_id: String,
    pub project_name: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummaryView {
    pub current_data_root: String,
    pub default_data_root: String,
    pub persisted_custom_root: Option<String>,
    pub pending_migration_target: Option<String>,
    pub env_override_active: bool,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub database_bytes: u64,
    pub logs_bytes: u64,
    pub projects_total_bytes: u64,
    pub projects: Vec<ProjectStorageUsageView>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn storage_summary_get(
    state: tauri::State<'_, AppState>,
) -> Result<StorageSummaryView, AppError> {
    let projects = state
        .projects
        .project
        .list()
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
    let data_dirs = state.system.data_dirs.clone();
    let default_data_root = state.system.default_data_root.clone();
    let project_pairs: Vec<(String, String, std::path::PathBuf)> = projects
        .into_iter()
        .map(|project| {
            let root = data_dirs.projects.join(&project.id);
            (project.id, project.name, root)
        })
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        use crate::infrastructure::filesystem::{
            directory_size_bytes, disk_space_bytes, read_persisted_data_root,
        };
        let (free_bytes, total_bytes) = disk_space_bytes(&data_dirs.root)
            .map(|(free, total)| (Some(free), Some(total)))
            .unwrap_or((None, None));
        let database_bytes = directory_size_bytes(&data_dirs.database)
            .saturating_add(directory_size_bytes(&data_dirs.root.join("app.db-wal")));
        let logs_bytes = directory_size_bytes(&data_dirs.logs);
        let mut projects = Vec::with_capacity(project_pairs.len());
        let mut projects_total_bytes = 0u64;
        for (project_id, project_name, root_path) in project_pairs {
            let size_bytes = directory_size_bytes(&root_path);
            projects_total_bytes = projects_total_bytes.saturating_add(size_bytes);
            projects.push(ProjectStorageUsageView {
                project_id,
                project_name,
                size_bytes,
            });
        }
        projects.sort_by(|left, right| right.size_bytes.cmp(&left.size_bytes));
        StorageSummaryView {
            current_data_root: data_dirs.root.to_string_lossy().into_owned(),
            default_data_root: default_data_root.to_string_lossy().into_owned(),
            persisted_custom_root: read_persisted_data_root()
                .map(|path| path.to_string_lossy().into_owned()),
            pending_migration_target: crate::infrastructure::filesystem::pending_data_root()
                .map(|p| p.to_string_lossy().into_owned()),
            env_override_active: std::env::var_os("AI_STUDIO_DATA_ROOT")
                .is_some_and(|value| !value.is_empty()),
            free_bytes,
            total_bytes,
            database_bytes,
            logs_bytes,
            projects_total_bytes,
            projects,
        }
    })
    .await
    .map_err(|error| AppError::internal(format!("计算存储空间失败：{error}")))
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataRootMigrationView {
    pub target_path: String,
    pub migrated_files: usize,
    pub migrated_bytes: u64,
    pub backup_retained_path: String,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn storage_data_root_pick_and_set(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    migrate_existing: Option<bool>,
) -> Result<Option<DataRootMigrationView>, AppError> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;
    let activity = state.system.diagnostics.runtime_activity_status().await?;
    if activity.active_task_count > 0 || activity.production_busy {
        return Err(AppError::invalid_input(
            "当前仍有生成任务或生产队列运行中，完成后才能迁移数据目录。",
        ));
    }
    let mut dialog = app_handle
        .dialog()
        .file()
        .set_title("选择新的 AI Studio 数据存储目录（将自动安全拷贝现有数据）");
    if let Some(window) = app_handle.webview_windows().into_values().next() {
        dialog = dialog.set_parent(&window);
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    dialog.pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let Some(folder) = rx
        .await
        .map_err(|_| AppError::filesystem("选择目录已中止"))?
    else {
        return Ok(None);
    };
    if migrate_existing == Some(false) {
        return Err(AppError::invalid_input("迁移必须保留现有数据"));
    }
    let path = folder
        .into_path()
        .map_err(|_| AppError::filesystem("所选目录不可用"))?;
    let target = path.join("AIStudioData");
    let source = state.system.data_dirs.root.clone();
    crate::infrastructure::filesystem::schedule_data_root_migration(&source, &target)?;
    Ok(Some(DataRootMigrationView {
        target_path: target.to_string_lossy().into_owned(),
        migrated_files: 0,
        migrated_bytes: 0,
        backup_retained_path: source.to_string_lossy().into_owned(),
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn storage_data_root_reset() -> Result<(), AppError> {
    crate::infrastructure::filesystem::clear_persisted_data_root()
}

#[tauri::command(rename_all = "camelCase")]
pub fn storage_open_directory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    target: String,
) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    let path = match target.as_str() {
        "logs" => state.system.data_dirs.logs.clone(),
        "backups" => state.system.data_dirs.root.join("backups"),
        _ => state.system.data_dirs.root.clone(),
    };
    let _ = std::fs::create_dir_all(&path);
    app_handle
        .opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|error| AppError::filesystem(format!("无法打开目录：{error}")))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn storage_wal_checkpoint(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.system.diagnostics.checkpoint_wal().await
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedCleanupResultView {
    pub inspected: usize,
    pub deleted_count: usize,
    pub skipped_in_use: usize,
    pub freed_bytes: u64,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn storage_cleanup_rejected_videos(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<RejectedCleanupResultView, AppError> {
    let service = &state.assets.artifact;
    let project = &project_id;
    let candidate_ids = collect_cleanup_candidates(|offset| async move {
        let page = service
            .review_queue(
                project,
                crate::application::ports::ArtifactReviewQueueFilter::Completed,
                100,
                offset,
            )
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        let count = page.items.len();
        let ids = page
            .items
            .into_iter()
            .filter(|item| {
                item.artifact.media_type == "video"
                    && item
                        .artifact
                        .review_status
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case("REJECTED"))
            })
            .map(|item| item.artifact.id)
            .collect();
        Ok((count, ids))
    })
    .await?;
    if candidate_ids.is_empty() {
        return Ok(RejectedCleanupResultView {
            inspected: 0,
            deleted_count: 0,
            skipped_in_use: 0,
            freed_bytes: 0,
        });
    }
    let mut report = RejectedCleanupResultView {
        inspected: 0,
        deleted_count: 0,
        skipped_in_use: 0,
        freed_bytes: 0,
    };
    for ids in candidate_ids.chunks(100) {
        let inspection = state
            .assets
            .deletion
            .inspect(&project_id, ids)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        report.inspected += inspection.items.len();
        report.skipped_in_use += inspection.blocked.len();
        if inspection.deletable.is_empty() {
            continue;
        }
        let deleted = state
            .assets
            .deletion
            .delete(&project_id, &inspection.deletable)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        report.deleted_count += deleted.deleted_count;
        report.freed_bytes = report.freed_bytes.saturating_add(
            inspection
                .items
                .iter()
                .filter(|i| i.can_delete)
                .map(|i| i.file_size)
                .sum::<u64>(),
        );
    }
    Ok(report)
}

async fn collect_cleanup_candidates<F, Fut>(mut page: F) -> Result<Vec<String>, AppError>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<(usize, Vec<String>), AppError>>,
{
    let mut offset = 0;
    let mut ids = Vec::new();
    loop {
        let (inspected, candidates) = page(offset).await?;
        ids.extend(candidates);
        if inspected < 100 {
            break;
        }
        offset += inspected;
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod cleanup_tests {
    #[tokio::test]
    async fn rejected_videos_are_found_after_a_full_page_of_approved_reviews() {
        let mut queried = Vec::new();
        let ids = super::collect_cleanup_candidates(|offset| {
            queried.push(offset);
            std::future::ready(Ok(if offset == 0 {
                (100, Vec::new())
            } else {
                (1, vec!["older-rejected-video".to_owned()])
            }))
        })
        .await
        .unwrap();
        assert_eq!(queried, vec![0, 100]);
        assert_eq!(ids, vec!["older-rejected-video"]);
    }
}
