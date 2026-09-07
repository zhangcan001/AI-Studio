use std::{fs, path::Path};

use ai_studio_lib::AppError;
use serde_json::json;

#[test]
fn app_error_serialization_preserves_the_ipc_contract() {
    let invalid_input = serde_json::to_value(AppError::invalid_input("bad request")).unwrap();
    assert_eq!(
        invalid_input,
        json!({
            "code": "INVALID_INPUT",
            "message": "bad request",
        })
    );

    let queue_busy = serde_json::to_value(AppError::production_queue_busy(
        "queue is busy",
        json!({ "batchId": "batch-1" }),
    ))
    .unwrap();
    assert_eq!(queue_busy["details"]["batchId"], "batch-1");

    let package_error = serde_json::to_value(AppError::production_package(
        "PACKAGE_RECIPE_INCOMPATIBLE",
        "package rejected",
        json!({}),
    ))
    .unwrap();
    assert_eq!(
        package_error["details"]["packageErrorCode"],
        "PACKAGE_RECIPE_INCOMPATIBLE"
    );
}

#[test]
fn tauri_commands_do_not_introduce_string_error_results() {
    let commands_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    for entry in fs::read_dir(commands_root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        let production = source.split("#[cfg(test)]").next().unwrap_or(&source);
        let mut command_signature = false;
        for line in production.lines() {
            if line.contains("#[tauri::command") {
                command_signature = true;
                continue;
            }
            if command_signature && line.contains(", String>") {
                panic!(
                    "tauri command in {} must return AppError instead of String",
                    path.display()
                );
            }
            if command_signature && line.contains('{') {
                command_signature = false;
            }
        }
    }
}
