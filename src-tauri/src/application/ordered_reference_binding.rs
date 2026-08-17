use crate::application::generation_service::ReferenceManifest;
use crate::application::product_runtime_scope::{
    MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID,
};
use crate::domain::{AssetId, InputDefinition, Recipe};
use std::collections::HashSet;

/// Returns the effective image bounds for the two supported H3 REF2VA
/// runtimes.  The runtime package keeps `min_items: 0` because it also
/// describes non-image reference modes; Shot production uses the stricter
/// image-to-video contract of at least two images.
pub(crate) fn ref2va_image_bounds(
    workflow_id: &str,
    recipe: &Recipe,
) -> Result<Option<(usize, usize)>, String> {
    let is_ref2va = matches!(
        workflow_id,
        MINIMAX_H3_WORKFLOW_ID | MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID
    );
    if !is_ref2va {
        return Ok(None);
    }
    let Some(input) = recipe.inputs.get("reference_images") else {
        return Err("REF2VA Recipe 缺少 plural reference_images 输入".to_owned());
    };
    let InputDefinition::Images {
        min_items,
        max_items,
        ..
    } = input
    else {
        return Err("REF2VA Recipe 的 reference_images 必须是 images 输入".to_owned());
    };
    if min_items > max_items {
        return Err("REF2VA Recipe 的 reference_images min_items 不能大于 max_items".to_owned());
    }
    if is_ref2va {
        let min_items = (*min_items).max(2);
        if min_items > *max_items {
            return Err(format!(
                "REF2VA Recipe 最多允许 {} 张参考图，但实际最少需要 {} 张",
                max_items, min_items
            ));
        }
        Ok(Some((min_items, *max_items)))
    } else {
        Ok(None)
    }
}

pub(crate) fn validate_ordered_reference_ids(
    asset_ids: &[AssetId],
    bounds: Option<(usize, usize)>,
) -> Result<(), String> {
    let mut seen = HashSet::with_capacity(asset_ids.len());
    for asset_id in asset_ids {
        if !seen.insert(asset_id) {
            return Err(format!("参考图重复：{}", asset_id.as_str()));
        }
    }
    if let Some((min_items, max_items)) = bounds {
        if asset_ids.len() < min_items {
            return Err(format!(
                "REF2VA 至少需要 {} 张参考图，当前 {} 张",
                min_items,
                asset_ids.len()
            ));
        }
        if asset_ids.len() > max_items {
            return Err(format!(
                "REF2VA 最多允许 {} 张参考图，当前 {} 张",
                max_items,
                asset_ids.len()
            ));
        }
    }
    Ok(())
}

pub(crate) fn reference_manifest(input_key: &str, asset_ids: &[AssetId]) -> ReferenceManifest {
    ReferenceManifest {
        input_key: input_key.to_owned(),
        asset_ids: asset_ids.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ref2va_image_bounds, validate_ordered_reference_ids};
    use crate::domain::{AssetId, InputDefinition, Recipe};
    use std::collections::BTreeMap;

    fn recipe(min_items: usize, max_items: usize) -> Recipe {
        Recipe {
            schema_version: 1,
            id: "rcp_ref2va".to_owned(),
            name: "REF2VA".to_owned(),
            workflow: crate::domain::WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: BTreeMap::from([(
                "reference_images".to_owned(),
                InputDefinition::Images {
                    label: "References".to_owned(),
                    required: false,
                    min_items,
                    max_items,
                },
            )]),
            bindings: Vec::new(),
            outputs: Vec::new(),
        }
    }

    fn asset_id(value: &str) -> AssetId {
        AssetId::parse(format!("ast_{value}")).expect("valid asset id")
    }

    #[test]
    fn ref2va_runtime_overrides_optional_recipe_minimum() {
        assert_eq!(
            ref2va_image_bounds("wfl_minimax_h3_reference_video_quality", &recipe(0, 9))
                .expect("valid bounds"),
            Some((2, 9))
        );
    }

    #[test]
    fn ordered_reference_validation_preserves_duplicates_and_bounds_as_errors() {
        let duplicate = vec![asset_id("b"), asset_id("a"), asset_id("b")];
        assert!(validate_ordered_reference_ids(&duplicate, Some((2, 9)))
            .expect_err("duplicate must fail")
            .contains("参考图重复"));
        assert!(
            validate_ordered_reference_ids(&[asset_id("a")], Some((2, 9)))
                .expect_err("minimum must fail")
                .contains("至少需要 2")
        );
    }
}
