//! Explicit acceptance process: real SQLx events, no production telemetry.
#[allow(dead_code, unused_imports)]
#[path = "../tests/dev061b_queue_recovery.rs"]
mod support;

use ai_studio_lib::application::{
    artifact_service::ArtifactService,
    asset_data_service::AssetDataService,
    asset_deletion_service::AssetDeletionService,
    asset_library_service::AssetLibraryService,
    asset_query_service::AssetQueryService,
    asset_usage_service::AssetUsageService,
    consistency_profile_service::ConsistencyProfileService,
    product::{library_facade::*, run_facade::*},
    prompt_library_service::PromptLibraryService,
    reference_set_service::ReferenceSetService,
    shot_service::ShotService,
    task_history_service::TaskHistoryService,
    task_query_service::TaskQueryService,
};
use ai_studio_lib::infrastructure::database::repositories::SqliteAssetUsageRepository;
use ai_studio_lib::infrastructure::{
    database::*, filesystem::FileSystemAssetStore, time::SystemClock,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqlitePoolOptions, Row};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};
use tracing_subscriber::layer::SubscriberExt;

#[path = "../tests/support/phase12_query_counter.rs"]
mod query_counter;
use query_counter::Counter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("AI_STUDIO_PERF_ISOLATED").as_deref() != Ok("YES") {
        return Err("Explicit isolation required".into());
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let path = PathBuf::from(args.first().ok_or("Database argument required")?).canonicalize()?;
    let root = PathBuf::from(std::env::var("TEMP")?)
        .join("ai-studio-phase12-native")
        .canonicalize()?;
    if path.parent() != Some(root.as_path()) {
        return Err("Only owned Phase12 fixture database permitted".into());
    }
    let output = args.get(1).ok_or("Output argument required")?;
    let counter = Counter::default();
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(counter.clone()))?;
    let mutation = args.get(2).is_some_and(|s| s == "--allow-mutation");
    if mutation && path.file_name().is_some_and(|s| s == "app.db") {
        return Err("Mutation profiling requires an owned database copy".into());
    }
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .read_only(!mutation)
        .thread_name(|_| "phase12-query-worker".into());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    // Prove capture works before profiling; setup is outside every scenario marker.
    counter.begin();
    sqlx::query("SELECT 1").fetch_all(&pool).await?;
    let calibration = counter.end();
    if calibration["totalSql"].as_u64() != Some(1) {
        return Err("SQL capture calibration failed; never report false zero".into());
    }
    let (runs, queue) = support::phase12_read_facade(&pool, &root);
    let projects = Arc::new(SqliteProjectRepository::new(pool.clone()));
    let assets = Arc::new(SqliteAssetRepository::new(pool.clone()));
    let organization = Arc::new(SqliteOrganizationRepository::new(pool.clone()));
    let profile_repo = Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
    let set_repo = Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
    let browse = AssetLibraryService::new(
        Arc::new(SqliteAssetBrowseRepository::new(pool.clone())),
        organization.clone(),
    );
    let detail = AssetQueryService::new(
        assets.clone(),
        Arc::new(FileSystemAssetStore),
        projects.clone(),
    )
    .with_organization_repository(organization);
    let prompts = PromptLibraryService::new(
        Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
        Arc::new(SystemClock),
    );
    let profiles = ConsistencyProfileService::new(
        profile_repo.clone(),
        set_repo.clone(),
        projects.clone(),
        Arc::new(SystemClock),
    );
    let sets = ReferenceSetService::new(
        set_repo,
        profile_repo,
        assets.clone(),
        Arc::new(SqliteReferenceAnchorRepository::new(pool.clone())),
        projects.clone(),
        Arc::new(SystemClock),
    );
    let tasks = Arc::new(SqliteTaskRepository::new(pool.clone()));
    let definitions = Arc::new(SqliteGenerationDefinitionRepository::new(pool.clone()));
    let history = TaskHistoryService::new(
        Arc::new(SqliteTaskHistoryRepository::new(pool.clone())),
        Arc::new(SqliteGenerationSnapshotRepository::new(pool.clone())),
        definitions.clone(),
        assets.clone(),
    );
    let artifacts = ArtifactService::new(
        Arc::new(SqliteArtifactRepository::new(pool.clone())),
        projects.clone(),
        tasks.clone(),
        queue,
        Arc::new(SystemClock),
    );
    let shots = ShotService::new(
        Arc::new(SqliteShotRepository::new(pool.clone())),
        tasks.clone(),
        assets.clone(),
        definitions.clone(),
        Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
        Arc::new(TaskQueryService::new(tasks, assets.clone(), definitions)),
        Arc::new(SqliteProductionQueueRepository::new(pool.clone())),
        Arc::new(SystemClock),
    );
    let run_details = RunDetailServices {
        history: &history,
        assets: &detail,
        artifacts: &artifacts,
        shots: &shots,
    };
    let library = LibraryServices {
        assets: &browse,
        asset_detail: &detail,
        prompts: &prompts,
        profiles: &profiles,
        reference_sets: &sets,
    };
    let usage = AssetUsageService::new(Arc::new(SqliteAssetUsageRepository::new(pool.clone())));
    let deletion = AssetDeletionService::new(
        assets.clone(),
        Arc::new(SqliteAssetDeletionRepository::new(pool.clone())),
        Arc::new(SqliteProjectRepository::new(pool.clone())),
        Arc::new(FileSystemAssetStore),
    );
    let data = AssetDataService::new(assets);
    let operations = LibraryOperations {
        library,
        usage: &usage,
        deletion: &deletion,
        data: &data,
    };
    let selected = sqlx::query(
        "SELECT id FROM tasks WHERE project_id='prj_default' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await?
    .get::<String, _>(0);
    let prompt_id =
        sqlx::query("SELECT id FROM prompt_entries WHERE project_id='prj_default' LIMIT 1")
            .fetch_one(&pool)
            .await?
            .get::<String, _>(0);
    let resource = ResourceRef::Prompt {
        id: prompt_id.clone(),
    };
    let mut scenarios = BTreeMap::new();
    for name in [
        "product_run_list",
        "product_run_get",
        "product_run_results_get",
        "product_library_list",
        "product_library_get",
        "product_library_relations_get",
        "product_library_versions_get",
        "product_library_resource_edit",
    ] {
        if name == "product_library_resource_edit" && !mutation {
            continue;
        }
        let mut samples = Vec::new();
        for index in 0..6 {
            counter.begin();
            let start = Instant::now();
            let projected = match name {
                "product_run_list" => serde_json::to_value(
                    runs.list("prj_default", RunListFilter::All, None)
                        .await
                        .map_err(|e| e.code)?,
                )?,
                "product_run_get" => serde_json::to_value(
                    runs.get_detail(
                        "prj_default",
                        RunRef {
                            source: RunSource::Task,
                            id: selected.clone(),
                        },
                        &run_details,
                    )
                    .await
                    .map_err(|e| e.code)?,
                )?,
                "product_run_results_get" => serde_json::to_value(
                    runs.results_get(
                        "prj_default",
                        RunRef {
                            source: RunSource::Task,
                            id: selected.clone(),
                        },
                        &run_details,
                    )
                    .await
                    .map_err(|e| e.code)?,
                )?,
                "product_library_list" => serde_json::to_value(
                    operations
                        .library
                        .list(
                            "prj_default",
                            LibraryQuery {
                                favorite_only: None,
                                tag_id: None,
                                category: LibraryCategory::Prompts,
                                keyword: None,
                                cursor: None,
                                limit: Some(30),
                            },
                        )
                        .await
                        .map_err(|e| e.code)?,
                )?,
                "product_library_get" => serde_json::to_value(
                    operations
                        .library
                        .get("prj_default", &resource)
                        .await
                        .map_err(|e| e.code)?,
                )?,
                "product_library_relations_get" => serde_json::to_value(
                    operations
                        .relations_get("prj_default", &resource)
                        .await
                        .map_err(|e| e.code)?,
                )?,
                "product_library_versions_get" => serde_json::to_value(
                    operations
                        .versions_get("prj_default", &resource)
                        .await
                        .map_err(|e| e.code)?,
                )?,
                _ => serde_json::to_value(
                    operations
                        .resource_edit(
                            "prj_default",
                            LibraryEditRequest::Prompt {
                                id: prompt_id.clone(),
                                text: format!("Isolated SQL profiling version {index}"),
                                model_version_id: None,
                            },
                        )
                        .await
                        .map_err(|e| e.code)?,
                )?,
            };
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let queries = counter.end();
            if index > 0 {
                samples.push(json!({"elapsedMs":elapsed,"queries":queries,
                "projectionSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&projected)?))}));
            }
        }
        println!("{}: {}", name, samples[0]["queries"]["totalSql"]);
        scenarios.insert(name, samples);
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(
            &json!({"method":"SQLx actual query-completion events in isolated acceptance process",
        "warmup":1,"samples":5,"calibration":calibration,"scenarios":scenarios}),
        )?,
    )?;
    pool.close().await;
    Ok(())
}
