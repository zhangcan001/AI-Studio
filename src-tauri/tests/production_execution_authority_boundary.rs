use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

fn production_source(source: &str) -> &str {
    source
        .match_indices("#[cfg(test)]")
        .find_map(|(index, marker)| {
            source[index + marker.len()..]
                .trim_start()
                .starts_with("mod tests")
                .then_some(&source[..index])
        })
        .unwrap_or(source)
}

fn rust_sources(directory: &std::path::Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("commands directory should be readable") {
        let path = entry.expect("commands entry should be readable").path();
        if path.is_dir() {
            rust_sources(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

#[test]
fn production_execution_requires_queue_start() {
    let commands_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut command_files = Vec::new();
    rust_sources(&commands_root, &mut command_files);
    for path in command_files {
        let command = fs::read_to_string(&path).expect("command source should be readable");
        let production = production_source(&command);
        assert!(
            !production.contains(".start_generation("),
            "commands must not start GenerationService directly: {}",
            path.display()
        );
        assert!(
            !production.contains(".start_generation_with_task_hook("),
            "commands must not start GenerationService with a task hook: {}",
            path.display()
        );
    }

    let shot_source = source("src/commands/shot.rs");
    let shot_command = production_source(&shot_source);
    assert!(
        shot_command.contains("prepare_generation_submission(ShotGenerationRequest"),
        "shot_generate must only prepare Shot inputs before queue submission"
    );
    assert!(
        shot_command.contains("create_direct_generation(CreateDirectGenerationRequest"),
        "shot_generate must persist its request through the existing Production Queue"
    );

    let generation_source = source("src/commands/generation.rs");
    let generation_command = production_source(&generation_source);
    assert!(
        generation_command.contains("create_direct_generation(request)"),
        "generation_create must create a queue submission"
    );
    assert!(
        generation_command
            .contains("create_direct_generation_batch(CreateDirectGenerationBatchRequest"),
        "generation_create_batch must create one multi-item queue batch"
    );

    let app_registration = source("src/lib.rs");
    assert!(
        app_registration.contains("commands::production_queue::production_queue_start"),
        "the official Production Queue Start command must stay registered"
    );
    for registered_submission in [
        "commands::generation::generation_create",
        "commands::generation::generation_create_batch",
        "commands::shot::shot_generate",
    ] {
        assert!(
            app_registration.contains(registered_submission),
            "compatibility submission command registration changed: {registered_submission}"
        );
    }

    let queue_worker_source = source("src/application/production_queue_service.rs");
    let queue_worker = production_source(&queue_worker_source);
    assert!(
        queue_worker.contains(".start_generation_with_task_hook("),
        "the existing Queue worker must remain able to execute queued items"
    );

    let frontend_client = source("../src/services/tauriClient.ts");
    for forbidden_contract in [
        "invoke<TaskView>(\"generation_create\"",
        "invoke<TaskView>(\"shot_generate\"",
        "invoke<GenerationBatchCreateResult>(\"generation_create_batch\"",
    ] {
        assert!(
            !frontend_client.contains(forbidden_contract),
            "legacy submission command must expose a queued result, not TaskView: {forbidden_contract}"
        );
    }
    for queued_contract in [
        "invoke<ProductionBatchDetail>(\"generation_create\"",
        "invoke<ProductionBatchDetail>(\"shot_generate\"",
        "invoke<ProductionBatchDetail>(\"generation_create_batch\"",
    ] {
        assert!(
            frontend_client.contains(queued_contract),
            "submission client contract must describe an existing Production Queue result: {queued_contract}"
        );
    }

    for (path, submission) in [
        (
            "../src/features/studio/hooks/useGenerationSubmissionController.ts",
            "submitGeneration({",
        ),
        (
            "../src/features/workflows/WorkflowWorkspace.tsx",
            "submitGeneration({",
        ),
        (
            "../src/features/assets/AssetVideoBatchWorkspace.tsx",
            "submitGeneration({",
        ),
        (
            "../src/features/tasks/TaskHistoryDetail.tsx",
            "submitGeneration({",
        ),
        (
            "../src/features/shots/ShotWorkspace.tsx",
            "submitShotGeneration({",
        ),
    ] {
        let frontend = source(path);
        assert!(
            frontend.contains(submission),
            "product UI must submit through the queue result contract: {path}"
        );
        assert!(
            frontend.contains("startProductionQueue("),
            "one-click product generation must then call the formal Queue Start API: {path}"
        );
        assert!(
            !frontend.contains("createGeneration({") && !frontend.contains("generateShot({"),
            "product UI must not use legacy immediate-Task client names: {path}"
        );
    }
}
