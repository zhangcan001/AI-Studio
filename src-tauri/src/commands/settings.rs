use crate::{app_state::AppState, application::ports::RuntimeParameterProfile, error::AppError};

#[tauri::command(rename_all = "camelCase")]
pub fn runtime_profiles_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RuntimeParameterProfile>, AppError> {
    Ok(state.settings_service.runtime_profiles())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn runtime_profiles_save(
    state: tauri::State<'_, AppState>,
    profile: RuntimeParameterProfile,
) -> Result<RuntimeParameterProfile, AppError> {
    state.settings_service.save_runtime_profile(profile).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn runtime_profiles_delete(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), AppError> {
    state
        .settings_service
        .delete_runtime_profile(&profile_id)
        .await
}
