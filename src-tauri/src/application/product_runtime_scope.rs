use crate::domain::ShotStage;

pub const KERA2_WORKFLOW_ID: &str = "wfl_kera2_t2i_local_v2";
pub const MINIMAX_H3_WORKFLOW_ID: &str = "wfl_minimax_h3_reference_video";
pub const MINIMAX_H3_FL2VA_WORKFLOW_ID: &str = "wfl_minimax_h3_fl2va";
pub const MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID: &str = "wfl_minimax_h3_fl2va_t2v_quality";
pub const MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID: &str = "wfl_minimax_h3_fl2va_i2v_quality";
pub const MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID: &str =
    "wfl_minimax_h3_fl2va_first_last_quality";
pub const MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID: &str = "wfl_minimax_h3_reference_video_quality";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductionRuntimeKind {
    Kera2Image,
    MiniMaxH3Video,
}

pub fn production_runtime_for_workflow_id(workflow_id: &str) -> Option<ProductionRuntimeKind> {
    match workflow_id {
        KERA2_WORKFLOW_ID => Some(ProductionRuntimeKind::Kera2Image),
        MINIMAX_H3_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID
        | MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID
        | MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID => Some(ProductionRuntimeKind::MiniMaxH3Video),
        _ => None,
    }
}

pub fn production_runtime_for_stage(
    stage: ShotStage,
    workflow_id: &str,
) -> Option<ProductionRuntimeKind> {
    match (stage, production_runtime_for_workflow_id(workflow_id)) {
        (ShotStage::Image, Some(ProductionRuntimeKind::Kera2Image)) => {
            Some(ProductionRuntimeKind::Kera2Image)
        }
        (ShotStage::Video, Some(ProductionRuntimeKind::MiniMaxH3Video)) => {
            Some(ProductionRuntimeKind::MiniMaxH3Video)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        production_runtime_for_stage, production_runtime_for_workflow_id, ProductionRuntimeKind,
        KERA2_WORKFLOW_ID, MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID,
        MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID, MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
        MINIMAX_H3_FL2VA_WORKFLOW_ID, MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
        MINIMAX_H3_WORKFLOW_ID,
    };
    use crate::domain::ShotStage;

    #[test]
    fn exact_workflow_ids_are_the_only_product_runtime_entries() {
        assert_eq!(
            production_runtime_for_workflow_id(KERA2_WORKFLOW_ID),
            Some(ProductionRuntimeKind::Kera2Image)
        );
        assert_eq!(
            production_runtime_for_workflow_id(MINIMAX_H3_WORKFLOW_ID),
            Some(ProductionRuntimeKind::MiniMaxH3Video)
        );
        assert_eq!(
            production_runtime_for_workflow_id(MINIMAX_H3_FL2VA_WORKFLOW_ID),
            Some(ProductionRuntimeKind::MiniMaxH3Video)
        );
        for workflow_id in [
            MINIMAX_H3_FL2VA_T2V_QUALITY_WORKFLOW_ID,
            MINIMAX_H3_FL2VA_I2V_QUALITY_WORKFLOW_ID,
            MINIMAX_H3_FL2VA_FIRST_LAST_QUALITY_WORKFLOW_ID,
            MINIMAX_H3_REF2VA_QUALITY_WORKFLOW_ID,
        ] {
            assert_eq!(
                production_runtime_for_workflow_id(workflow_id),
                Some(ProductionRuntimeKind::MiniMaxH3Video)
            );
        }
        assert_eq!(production_runtime_for_workflow_id("wfl_other"), None);
    }

    #[test]
    fn nearby_ids_cannot_enter_a_stage_scope() {
        assert_eq!(
            production_runtime_for_workflow_id("wfl_other_kera2_test_fake"),
            None
        );
        assert_eq!(
            production_runtime_for_workflow_id("wfl_fake_minimax_h3_reference_video_clone"),
            None
        );
        assert_eq!(
            production_runtime_for_stage(ShotStage::Video, KERA2_WORKFLOW_ID),
            None
        );
        assert_eq!(
            production_runtime_for_stage(ShotStage::Image, MINIMAX_H3_WORKFLOW_ID),
            None
        );
    }
}
