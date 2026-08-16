//! Conservative execution scheduling metadata.
//!
//! This module deliberately does not own a queue or start work. It classifies
//! an already selected generation definition so the existing ProductionQueue
//! can remain strictly serial while future scheduling policy has a stable
//! seam.

use super::ports::GenerationDefinition;
use crate::application::product_runtime_scope::{
    KERA2_WORKFLOW_ID, MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID,
    MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID, MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
    MINIMAX_H3_FL2VA_WORKFLOW_ID, MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID, MINIMAX_H3_WORKFLOW_ID,
};
use crate::domain::Recipe;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionProfile {
    Krea2Image,
    H3Fast,
    H3Quality,
    H3Ref2vaFast,
    H3Ref2vaQuality,
    Unknown,
}

impl ExecutionProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Krea2Image => "KREA2_IMAGE",
            Self::H3Fast => "H3_FAST",
            Self::H3Quality => "H3_QUALITY",
            Self::H3Ref2vaFast => "H3_REF2VA_FAST",
            Self::H3Ref2vaQuality => "H3_REF2VA_QUALITY",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub const fn concurrency_class(self) -> ConcurrencyClass {
        match self {
            Self::Krea2Image | Self::H3Quality | Self::H3Ref2vaQuality | Self::H3Ref2vaFast => {
                ConcurrencyClass::GpuHeavySerial
            }
            Self::H3Fast => ConcurrencyClass::GpuStandardSerial,
            Self::Unknown => ConcurrencyClass::CpuLight,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConcurrencyClass {
    GpuHeavySerial,
    GpuStandardSerial,
    CpuLight,
}

impl ConcurrencyClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuHeavySerial => "GPU_HEAVY_SERIAL",
            Self::GpuStandardSerial => "GPU_STANDARD_SERIAL",
            Self::CpuLight => "CPU_LIGHT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerDecision {
    pub profile: ExecutionProfile,
    pub concurrency_class: ConcurrencyClass,
    pub max_concurrent: u32,
}

impl SchedulerDecision {
    pub const fn for_profile(profile: ExecutionProfile) -> Self {
        Self {
            profile,
            concurrency_class: profile.concurrency_class(),
            // v0.3.x keeps the published serial behavior. This value is a
            // policy fact, not a request to add a second executor.
            max_concurrent: 1,
        }
    }
}

/// Classify only exact product workflow identities. No substring or fuzzy
/// package-name matching is allowed here; an unknown definition is explicit.
pub fn classify_generation(
    definition: &GenerationDefinition,
    _recipe: &Recipe,
) -> ExecutionProfile {
    match definition.workflow_id.as_str() {
        KERA2_WORKFLOW_ID => ExecutionProfile::Krea2Image,
        MINIMAX_H3_FL2VA_WORKFLOW_ID | "wfl_aitudou_minimax_h3_lightx2v_8step_fast" => {
            ExecutionProfile::H3Fast
        }
        MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID => ExecutionProfile::H3Quality,
        MINIMAX_H3_WORKFLOW_ID => ExecutionProfile::H3Ref2vaFast,
        MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID => ExecutionProfile::H3Ref2vaQuality,
        _ => ExecutionProfile::Unknown,
    }
}

pub fn scheduler_decision(definition: &GenerationDefinition, recipe: &Recipe) -> SchedulerDecision {
    SchedulerDecision::for_profile(classify_generation(definition, recipe))
}

#[cfg(test)]
mod tests {
    use super::{classify_generation, scheduler_decision, ConcurrencyClass, ExecutionProfile};
    use crate::application::ports::GenerationDefinition;
    use crate::domain::{Recipe, WorkflowRef};
    use serde_json::json;

    fn definition(workflow_id: &str) -> GenerationDefinition {
        GenerationDefinition {
            workflow_id: workflow_id.to_owned(),
            workflow_version_id: "version-1".to_owned(),
            recipe_id: "recipe-1".to_owned(),
            workflow_version: "1.0.0".to_owned(),
            workflow_sha256: "workflow-sha".to_owned(),
            recipe_version: "1.0.0".to_owned(),
            recipe_sha256: "recipe-sha".to_owned(),
            package_name: Some("package".to_owned()),
            package_source_path: None,
            workflow_json: json!({}),
            recipe_yaml: "schema_version: 1\n".to_owned(),
        }
    }

    fn recipe() -> crate::domain::Recipe {
        Recipe {
            schema_version: 1,
            id: "recipe-1".to_owned(),
            name: "Test recipe".to_owned(),
            workflow: WorkflowRef {
                file: "workflow_api.json".to_owned(),
            },
            inputs: Default::default(),
            bindings: Vec::new(),
            outputs: Vec::new(),
        }
    }

    #[test]
    fn exact_profiles_are_classified_without_fuzzy_matching() {
        assert_eq!(
            classify_generation(&definition("wfl_kera2_t2i_local_v2"), &recipe()),
            ExecutionProfile::Krea2Image
        );
        assert_eq!(
            classify_generation(&definition("wfl_minimax_h3_fl2va"), &recipe()),
            ExecutionProfile::H3Fast
        );
        assert_eq!(
            classify_generation(&definition("wfl_minimax_h3_fl2va_t2v_quality"), &recipe()),
            ExecutionProfile::H3Quality
        );
        assert_eq!(
            classify_generation(&definition("wfl_minimax_h3_reference_video"), &recipe()),
            ExecutionProfile::H3Ref2vaFast
        );
        assert_eq!(
            classify_generation(
                &definition("wfl_minimax_h3_reference_video_quality"),
                &recipe()
            ),
            ExecutionProfile::H3Ref2vaQuality
        );
        assert_eq!(
            classify_generation(&definition("wfl_fake_minimax_h3_fl2va"), &recipe()),
            ExecutionProfile::Unknown
        );
    }

    #[test]
    fn all_product_profiles_are_serial_and_h3_quality_is_heavy() {
        for profile in [
            ExecutionProfile::Krea2Image,
            ExecutionProfile::H3Fast,
            ExecutionProfile::H3Quality,
            ExecutionProfile::H3Ref2vaFast,
            ExecutionProfile::H3Ref2vaQuality,
        ] {
            assert_eq!(
                scheduler_decision(&definition("wfl_minimax_h3_fl2va"), &recipe()).max_concurrent,
                1
            );
            assert_eq!(
                super::SchedulerDecision::for_profile(profile).max_concurrent,
                1
            );
        }
        assert_eq!(
            ExecutionProfile::H3Quality.concurrency_class(),
            ConcurrencyClass::GpuHeavySerial
        );
        assert_eq!(
            ExecutionProfile::H3Fast.concurrency_class(),
            ConcurrencyClass::GpuStandardSerial
        );
    }
}
