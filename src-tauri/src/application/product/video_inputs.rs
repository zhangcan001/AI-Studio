//! Stateless product DTOs over the single persistent shot-input authority.
use super::{error::ProductError, selection_ref::ExactGeneratorSelection};
use crate::application::{
    ports::{ShotVideoInputAsset, ShotVideoInputScope, ShotVideoInputSet, ShotVideoInputToken},
    shot_video_input_service::{ShotVideoInputError, ShotVideoInputService},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoInputSelection {
    pub project_id: String,
    pub shot_id: String,
    pub selection_ref: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoInputSave {
    pub selection: VideoInputSelection,
    pub expected: Option<ShotVideoInputToken>,
    pub inputs: Vec<ShotVideoInputAsset>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInputView {
    pub selection_ref: String,
    pub token: Option<ShotVideoInputToken>,
    pub inputs: Vec<ShotVideoInputAsset>,
    pub assets: Vec<InputAsset>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputAsset {
    pub id: String,
    pub name: String,
    pub media_kind: String,
}
impl From<crate::domain::Asset> for InputAsset {
    fn from(a: crate::domain::Asset) -> Self {
        Self {
            id: a.id.to_string(),
            name: a.name,
            media_kind: a.asset_type.as_str().into(),
        }
    }
}

impl VideoInputSelection {
    fn scope(&self) -> Result<ShotVideoInputScope, ProductError> {
        crate::domain::validate_project_id(&self.project_id)
            .map_err(|_| ProductError::invalid_selection())?;
        let pair = ExactGeneratorSelection::decode(&self.selection_ref)?;
        Ok(ShotVideoInputScope {
            project_id: self.project_id.clone(),
            shot_id: self.shot_id.clone(),
            workflow_version_id: pair.workflow_version_id,
            recipe_id: pair.recipe_id,
        })
    }
    fn view(&self, set: Option<ShotVideoInputSet>) -> VideoInputView {
        VideoInputView {
            selection_ref: self.selection_ref.clone(),
            token: set.as_ref().map(|s| s.token.clone()),
            inputs: set.map(|s| s.inputs).unwrap_or_default(),
            assets: Vec::new(),
        }
    }
}
fn map_error(error: ShotVideoInputError) -> ProductError {
    match error {
        ShotVideoInputError::Conflict => ProductError::new(
            "SHOT_VIDEO_INPUT_CONFLICT",
            "镜头输入已被更新；请刷新后重新保存，当前修改未覆盖服务器。",
            Some("REFRESH_INPUTS"),
        ),
        ShotVideoInputError::Invalid(_) => ProductError::new(
            "VIDEO_INPUT_INVALID",
            "输入不可用或不属于当前 Recipe，请重新选择受管理素材。",
            Some("RESELECT_INPUT"),
        ),
        ShotVideoInputError::Combination => ProductError::new(
            "SHOT_VIDEO_INPUT_COMBINATION_INVALID",
            "参考图最多 9 个、参考视频最多 3 个、参考音频最多 3 个，合计不超过 12 个，且不能只有音频。",
            Some("ADJUST_INPUTS"),
        ),
    }
}
pub async fn get(
    service: &ShotVideoInputService,
    selection: VideoInputSelection,
) -> Result<VideoInputView, ProductError> {
    let mut view = selection.view(service.get(&selection.scope()?).await.map_err(map_error)?);
    view.assets = service
        .available_assets(&selection.project_id, &view.inputs)
        .await
        .map_err(map_error)?
        .into_iter()
        .map(InputAsset::from)
        .collect();
    Ok(view)
}
pub async fn save(
    service: &ShotVideoInputService,
    request: VideoInputSave,
) -> Result<VideoInputView, ProductError> {
    let set = service
        .save(
            &request.selection.scope()?,
            request.expected.as_ref(),
            &request.inputs,
        )
        .await
        .map_err(map_error)?;
    let mut view = request.selection.view(Some(set));
    view.assets = service
        .available_assets(&request.selection.project_id, &view.inputs)
        .await
        .map_err(map_error)?
        .into_iter()
        .map(InputAsset::from)
        .collect();
    Ok(view)
}
