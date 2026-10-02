//! Selection-aware preparation for normal Create. Legacy Shot/retry stays intact.
use super::*;

impl ShotService {
    /// Read-only: the session selection does not update a project binding or Shot config.
    /// Generic media are explicit draft inputs, never persistent Shot references.
    pub async fn prepare_creation_submission(
        &self,
        request: ShotGenerationRequest,
        workflow_version_id: String,
        recipe_id: String,
    ) -> Result<PreparedShotGeneration, ShotServiceError> {
        validate_project(&request.project_id)?;
        if request.retry_task_id.is_some() {
            return Err(ShotServiceError::InvalidInput(
                "重试必须使用原运行的不可变快照".to_owned(),
            ));
        }
        let data = self
            .repository
            .find(&request.project_id, &request.shot_id)
            .await?
            .ok_or_else(|| ShotServiceError::NotFound(request.shot_id.clone()))?;
        let definition = self
            .definition_repository
            .find(&workflow_version_id, &recipe_id)
            .await?
            .ok_or_else(|| ShotServiceError::InvalidInput("生成器不可用".to_owned()))?;
        let recipe = RecipeParser::parse(&definition.recipe_yaml)
            .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?;
        let output = match request.stage {
            ShotStage::Image => crate::domain::OutputType::Image,
            ShotStage::Video => crate::domain::OutputType::Video,
        };
        if !recipe.outputs.iter().any(|item| item.output_type == output) {
            return Err(ShotServiceError::InvalidInput(
                "生成器与当前阶段不匹配".to_owned(),
            ));
        }
        let mut values = match data.stage_configs.iter().find(|config| {
            config.stage == request.stage
                && config.workflow_version_id == workflow_version_id
                && config.recipe_id == recipe_id
        }) {
            Some(config) => scalar_values_from_json(&config.scalar_values)?,
            None => BTreeMap::new(),
        };
        values.extend(request.values);
        // Only a formally named positive prompt may inherit stage context. Never
        // guess the first textarea (which can be negative_prompt/delimiter).
        if matches!(
            recipe.inputs.get("prompt"),
            Some(InputDefinition::TextArea { .. })
        ) && !values.contains_key("prompt")
        {
            let prompt = self
                .stage_prompt_text(
                    &request.project_id,
                    &request.shot_id,
                    request.stage,
                    &data.shot,
                )
                .await?;
            values.insert("prompt".to_owned(), GenerationInputValue::Text(prompt));
        }
        // Multiple image slots (first/last, etc.) must be supplied by field identity.
        // Only an unambiguous single image field may inherit persistent context.
        let image_fields: Vec<_> = recipe
            .inputs
            .iter()
            .filter(|(_, input)| {
                matches!(
                    input,
                    InputDefinition::Image { .. } | InputDefinition::Images { .. }
                )
            })
            .collect();
        if let [(key, input)] = image_fields.as_slice() {
            if !values.contains_key(*key) {
                let references =
                    ordered_reference_asset_ids(&data.reference_assets, request.stage)?;
                let inherited = match input {
                    InputDefinition::Image { .. } if request.stage == ShotStage::Video => data
                        .shot
                        .selected_image_asset_id
                        .as_ref()
                        .map(|id| AssetId::parse(id.clone()))
                        .transpose()
                        .map_err(|error| ShotServiceError::InvalidInput(error.to_string()))?
                        .map(GenerationInputValue::ImageAsset),
                    InputDefinition::Image { .. } if references.len() == 1 => {
                        Some(GenerationInputValue::ImageAsset(references[0].clone()))
                    }
                    InputDefinition::Images { .. } if !references.is_empty() => {
                        Some(GenerationInputValue::ImageAssets(references))
                    }
                    _ => None,
                };
                if let Some(value) = inherited {
                    values.insert((*key).clone(), value);
                }
            }
        }
        Ok(PreparedShotGeneration {
            project_id: request.project_id,
            shot_id: request.shot_id,
            stage: request.stage,
            workflow_version_id,
            recipe_id,
            values,
            submission_idempotency_key: request.submission_idempotency_key,
            parent_task_id: None,
        })
    }
}
