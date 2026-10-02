use serde::Serialize;

/// Details survive the existing IPC error normalizer; UI reads typed codes,
/// never technical error strings.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductError {
    pub code: &'static str,
    pub message: &'static str,
    pub details: ProductErrorDetails,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_location: Option<super::run_facade::RunActionLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_binding: Option<super::project_facade::GeneratorBindingSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<&'static str>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub technical_details: Option<String>,
}

impl ProductError {
    pub fn new(code: &'static str, message: &'static str, action: Option<&'static str>) -> Self {
        Self {
            code,
            message,
            details: ProductErrorDetails {
                action,
                ..Default::default()
            },
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        let mut value = Self::new(
            "INTERNAL_ERROR",
            "暂时无法完成操作，请稍后重试。",
            Some("TRY_LATER"),
        );
        value.details.technical_details = Some(error.to_string());
        value
    }

    pub fn binding(
        error: crate::application::project_workflow_binding_service::ProjectWorkflowBindingServiceError,
    ) -> Self {
        use crate::application::project_workflow_binding_service::ProjectWorkflowBindingServiceError as E;
        match error {
            E::ProjectNotFound(_) => Self::new(
                "PROJECT_NOT_FOUND",
                "项目不存在，请返回项目列表。",
                Some("OPEN_PROJECTS"),
            ),
            E::Conflict(conflict) => {
                let mut value = Self::new(
                    "GENERATOR_BINDING_CONFLICT",
                    "生成器设置已被更新。请查看最新设置后再保存，当前修改未覆盖服务器。",
                    Some("REVIEW_CURRENT_BINDING"),
                );
                if let (Some(version), Some(recipe), Some(instance), Some(revision)) = (
                    conflict.current_workflow_version_id,
                    conflict.current_recipe_id,
                    conflict.current_binding_instance_id,
                    conflict.current_revision,
                ) {
                    match (super::selection_ref::ExactGeneratorSelection {
                        workflow_version_id: version,
                        recipe_id: recipe,
                    })
                    .encode()
                    {
                        Ok(selection_ref) => {
                            value.details.current_binding =
                                Some(super::project_facade::GeneratorBindingSummary {
                                    stage: conflict.stage,
                                    mode: conflict.mode,
                                    selection_ref,
                                    binding_instance_id: instance,
                                    revision,
                                })
                        }
                        Err(error) => value.details.technical_details = Some(error.code.to_owned()),
                    }
                }
                value
            }
            E::Invalid(detail) => {
                let mut value = Self::new(
                    "GENERATOR_UNAVAILABLE",
                    "该生成器当前不能用于此位置，请检查选择和项目设置。",
                    Some("SELECT_GENERATOR"),
                );
                value.details.technical_details = Some(detail);
                value
            }
            other => Self::internal(other),
        }
    }
    pub fn invalid_selection() -> Self {
        Self {
            code: "GENERATOR_UNAVAILABLE",
            message: "生成器选择无效，请重新选择。",
            details: ProductErrorDetails {
                field: Some("selectionRef".to_owned()),
                action: Some("SELECT_GENERATOR"),
                ..Default::default()
            },
        }
    }
}
