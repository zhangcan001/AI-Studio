//! DEV-078 Agent C: the Tauri start boundary must use the runtime admission service.

use std::{fs, path::Path};

use ai_studio_lib::AppError;
use serde_json::json;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

#[test]
fn production_queue_start_delegates_to_runtime_admission_service() {
    let source = fs::read_to_string(Path::new(ROOT).join("src/commands/production_queue.rs"))
        .expect("production queue command source should be readable");
    let start = source
        .find("pub async fn production_queue_start")
        .expect("production_queue_start command should exist");
    let end = source[start..]
        .find("\n#[tauri::command]")
        .map_or(source.len(), |offset| start + offset);
    let body = &source[start..end];

    assert!(
        body.contains("production_start_admission_service"),
        "production_queue_start must delegate to the runtime admission service"
    );
    assert!(
        body.contains(".start(&project_id, &batch_id)"),
        "the boundary must pass the original project and batch identifiers"
    );
    assert!(
        !body.contains("production_queue_service"),
        "production_queue_start must not bypass runtime admission"
    );
}

#[test]
fn formal_backend_start_call_sites_do_not_bypass_runtime_admission() {
    for relative_path in [
        "src/application/h3_local_import_service.rs",
        "src/application/production_item_review_service.rs",
        "src/application/production_orchestrator_service.rs",
        "src/application/workflow_benchmark_service.rs",
    ] {
        let source = fs::read_to_string(Path::new(ROOT).join(relative_path))
            .expect("formal production service source should be readable");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            !normalized.contains("production_queue_service .start("),
            "{relative_path} must route starts through ProductionStartAdmissionService"
        );
        assert!(
            source.contains("start_production"),
            "{relative_path} must have an explicit guarded start facade"
        );
    }
}

#[test]
fn runtime_block_uses_a_stable_top_level_error_code_and_details() {
    let error = AppError::production_start_admission_blocked(
        "Production batch runtime admission failed",
        json!({
            "code": "RUNTIME_ADMISSION_MISSING_NODES",
            "workflowVersionId": "wfv_h3",
            "recipeId": "rcp_h3",
            "missingNodes": ["KSampler"],
        }),
    );
    assert_eq!(error.code(), "PRODUCTION_START_ADMISSION_BLOCKED");
    let serialized = serde_json::to_value(error).expect("AppError should serialize");
    assert_eq!(serialized["code"], "PRODUCTION_START_ADMISSION_BLOCKED");
    assert_eq!(
        serialized["details"]["code"],
        "RUNTIME_ADMISSION_MISSING_NODES"
    );
    assert_eq!(serialized["details"]["missingNodes"][0], "KSampler");
}
