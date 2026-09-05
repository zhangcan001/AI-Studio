use ai_studio_lib::application::{
    ports::{
        WorkflowRuntimeArtifactRecord, WorkflowRuntimeArtifactRepository,
        WorkflowRuntimeRepository, WorkflowRuntimeStateRepository,
    },
    workflow_analysis_service::WorkflowAnalysisService,
    workflow_registry_service::WorkflowRegistryService,
};
use ai_studio_lib::domain::WorkflowDocument;
use ai_studio_lib::infrastructure::{
    database::{
        initialize, SqliteProjectWorkflowBindingRepository, SqliteWorkflowRegistryRepository,
        SqliteWorkflowRuntimeArtifactRepository, SqliteWorkflowRuntimeRepository,
        SqliteWorkflowRuntimeStateRepository,
    },
    time::SystemClock,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::{fs, sync::Arc};
use tempfile::tempdir;

const AITUDOU_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime_packages/aitudou_minimax_h3_lightx2v_8step_fast_1_0_0/workflow_api.json"
));

fn aitudou_workflow() -> WorkflowDocument {
    WorkflowDocument::parse(
        serde_json::from_str::<Value>(AITUDOU_WORKFLOW)
            .expect("AITUDOU fixture should be valid JSON"),
    )
    .expect("AITUDOU fixture should be an API workflow")
}

#[test]
fn dev083_aitudou_analysis_uses_one_graph_engine() {
    let report =
        WorkflowAnalysisService::analyze_workflow(&aitudou_workflow(), AITUDOU_WORKFLOW.as_bytes());

    let input = |key: &str| {
        report
            .inputs
            .iter()
            .find(|input| input.semantic_key == key)
            .unwrap_or_else(|| panic!("AITUDOU analysis should infer {key}"))
    };
    assert_eq!(
        (
            input("prompt").node_id.as_str(),
            input("prompt").input_name.as_str()
        ),
        ("59", "text")
    );
    assert_eq!(
        (
            input("width").node_id.as_str(),
            input("width").input_name.as_str()
        ),
        ("63", "width")
    );
    assert_eq!(
        (
            input("height").node_id.as_str(),
            input("height").input_name.as_str()
        ),
        ("63", "height")
    );
    assert_eq!(
        (
            input("duration_seconds").node_id.as_str(),
            input("duration_seconds").input_name.as_str()
        ),
        ("49", "value")
    );
    assert_eq!(
        (
            input("seed").node_id.as_str(),
            input("seed").input_name.as_str()
        ),
        ("2", "noise_seed")
    );
    assert_eq!(
        input("steps").value.as_ref().and_then(Value::as_i64),
        Some(8)
    );
    assert_eq!(
        input("denoise").value.as_ref().and_then(Value::as_f64),
        Some(1.0)
    );
    assert_eq!(
        input("fps").value.as_ref().and_then(Value::as_i64),
        Some(24)
    );
    assert!(report
        .outputs
        .iter()
        .any(|output| output.node_id == "62" && output.output_type == "video"));
}

#[tokio::test]
async fn dev083_analysis_has_zero_database_and_package_side_effects() {
    let directory = tempdir().expect("DEV-083 temporary directory should exist");
    let package_root = directory.path().join("packages");
    fs::create_dir_all(&package_root).expect("package directory should exist");
    fs::write(package_root.join("existing.txt"), b"unchanged")
        .expect("package sentinel should be written");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-083 database should initialize");
    let before_rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows")
        .fetch_one(&pool)
        .await
        .expect("workflow count should be readable");
    let before_artifacts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&pool)
            .await
            .expect("artifact count should be readable");
    let before_files = fs::read_dir(&package_root)
        .expect("package directory should be readable")
        .count();

    let report =
        WorkflowAnalysisService::analyze_workflow(&aitudou_workflow(), AITUDOU_WORKFLOW.as_bytes());
    assert!(report.recognized && report.importable);

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflows")
            .fetch_one(&pool)
            .await
            .expect("workflow count should remain readable"),
        before_rows
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workflow_runtime_artifacts")
            .fetch_one(&pool)
            .await
            .expect("artifact count should remain readable"),
        before_artifacts
    );
    assert_eq!(
        fs::read_dir(&package_root)
            .expect("package directory should remain readable")
            .count(),
        before_files
    );
}

#[tokio::test]
async fn dev083_registry_groups_versions_and_resolves_each_recipe_artifact() {
    let directory = tempdir().expect("DEV-083 temporary directory should exist");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-083 database should initialize");
    let timestamp = "2026-09-05T00:00:00Z";
    sqlx::query(
        "INSERT INTO workflows
         (id, name, category, mode, current_version_id, created_at, updated_at)
         VALUES ('wfl_dev083', 'DEV-083 Workflow', 'video', 'text_to_video', 'wfv_dev083_2', ?, ?)",
    )
    .bind(timestamp)
    .bind(timestamp)
    .execute(&pool)
    .await
    .expect("workflow fixture should insert");
    for (version_id, version, hash) in [
        ("wfv_dev083_1", "1.0.0", "workflow-sha-1"),
        ("wfv_dev083_2", "2.0.0", "workflow-sha-2"),
    ] {
        sqlx::query(
            "INSERT INTO workflow_versions
             (id, workflow_id, version, api_workflow_json, workflow_sha256, created_at)
             VALUES (?, 'wfl_dev083', ?, ?, ?, ?)",
        )
        .bind(version_id)
        .bind(version)
        .bind(json!({"1": {"inputs": {}, "class_type": "SaveVideo"}}).to_string())
        .bind(hash)
        .bind(timestamp)
        .execute(&pool)
        .await
        .expect("workflow version fixture should insert");
    }
    for (recipe_id, version, hash) in [
        ("rcp_dev083_1", "1.0.0", "recipe-sha-1"),
        ("rcp_dev083_2", "1.1.0", "recipe-sha-2"),
        ("rcp_dev083_3", "1.0.0", "recipe-sha-3"),
    ] {
        let workflow_version_id = if recipe_id.ends_with('3') {
            "wfv_dev083_2"
        } else {
            "wfv_dev083_1"
        };
        sqlx::query(
            "INSERT INTO recipes
             (id, workflow_version_id, version, schema_version, recipe_yaml,
              recipe_sha256, created_at)
             VALUES (?, ?, ?, 1, 'schema_version: 1', ?, ?)",
        )
        .bind(recipe_id)
        .bind(workflow_version_id)
        .bind(version)
        .bind(hash)
        .bind(timestamp)
        .execute(&pool)
        .await
        .expect("recipe fixture should insert");
    }

    let artifacts = SqliteWorkflowRuntimeArtifactRepository::new(pool.clone());
    for (id, recipe_id, package_name, recipe_hash) in [
        ("wra_dev083_a", "rcp_dev083_1", "package-a", "recipe-sha-1"),
        ("wra_dev083_b", "rcp_dev083_2", "package-b", "recipe-sha-2"),
    ] {
        artifacts
            .upsert(&WorkflowRuntimeArtifactRecord {
                id: id.to_owned(),
                workflow_version_id: "wfv_dev083_1".to_owned(),
                recipe_id: recipe_id.to_owned(),
                package_name: package_name.to_owned(),
                source_kind: "USER".to_owned(),
                package_source_path: None,
                workflow_sha256: "workflow-sha-1".to_owned(),
                recipe_sha256: recipe_hash.to_owned(),
                created_at: Utc::now(),
            })
            .await
            .expect("exact artifact fixture should insert");
    }

    let runtime: Arc<dyn WorkflowRuntimeRepository> =
        Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone()));
    let states: Arc<dyn WorkflowRuntimeStateRepository> =
        Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone()));
    let bindings: Arc<dyn ai_studio_lib::application::ports::ProjectWorkflowBindingRepository> =
        Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone()));
    let registry = WorkflowRegistryService::new(runtime, states, bindings, Arc::new(SystemClock))
        .with_registry_repository(Arc::new(SqliteWorkflowRegistryRepository::new(pool)))
        .with_runtime_artifact_repository(Arc::new(artifacts));

    let views = registry.list().await.expect("registry list should succeed");
    assert_eq!(views.len(), 1, "one logical workflow must produce one row");
    assert_eq!(views[0].versions.len(), 2);
    assert_eq!(views[0].recipes.len(), 3);
    assert_eq!(views[0].current_version_id.as_deref(), Some("wfv_dev083_2"));
    assert!(registry
        .is_available("wfv_dev083_1", "rcp_dev083_1")
        .await
        .expect("historical exact binding should remain available"));
    assert_eq!(
        registry
            .resolve("wfv_dev083_1", "rcp_dev083_1")
            .await
            .expect("recipe A should resolve")
            .expect("recipe A should be available")
            .package_name
            .as_deref(),
        Some("package-a")
    );
    assert_eq!(
        registry
            .resolve("wfv_dev083_1", "rcp_dev083_2")
            .await
            .expect("recipe B should resolve")
            .expect("recipe B should be available")
            .package_name
            .as_deref(),
        Some("package-b")
    );
}
