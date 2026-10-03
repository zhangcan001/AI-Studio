//! Phase 5 targets 1, 2 and 8: real project-scoped authorities and typed reads.
#[path = "support/project_database.rs"]
mod project_database;
use ai_studio_lib::application::{
    asset_data_service::AssetDataService,
    asset_deletion_service::AssetDeletionService,
    asset_library_service::AssetLibraryService,
    asset_query_service::AssetQueryService,
    asset_usage_service::AssetUsageService,
    consistency_profile_service::{ConsistencyProfileService, CreatePropProfileRequest},
    ports::{AssetRepository, ProjectRepository},
    product::library_facade::*,
    prompt_library_service::PromptLibraryService,
    reference_set_service::{
        CreateReferenceSetRequest, ReferenceSetItemRequest, ReferenceSetService,
    },
};
use ai_studio_lib::domain::consistency::ReferenceSetPurpose;
use ai_studio_lib::infrastructure::database::repositories::SqliteAssetUsageRepository;
use ai_studio_lib::infrastructure::{
    database::*, filesystem::FileSystemAssetStore, time::SystemClock,
};
use chrono::Utc;
use project_database::ProjectDatabase;
use sqlx::SqlitePool;
use std::{collections::HashSet, sync::Arc};

struct Fixture {
    _dir: ProjectDatabase,
    pool: SqlitePool,
    assets: AssetLibraryService,
    asset_detail: AssetQueryService,
    prompts: PromptLibraryService,
    profiles: ConsistencyProfileService,
    reference_sets: ReferenceSetService,
    usage: AssetUsageService,
    deletion: AssetDeletionService,
    data: AssetDataService,
}
impl Fixture {
    async fn new() -> Self {
        let dir = ProjectDatabase::new("library.db").await;
        let pool = dir.pool.clone();
        let projects = Arc::new(SqliteProjectRepository::new(pool.clone()));
        for id in ["prj_default", "prj_11111111-1111-4111-8111-111111111111"] {
            projects
                .ensure_default_project(id, id, &dir.path().join(id), Utc::now())
                .await
                .unwrap();
        }
        let profile_repo = Arc::new(SqliteConsistencyProfileRepository::new(pool.clone()));
        let set_repo = Arc::new(SqliteReferenceSetRepository::new(pool.clone()));
        let asset_repo = Arc::new(SqliteAssetRepository::new(pool.clone()));
        let organization = Arc::new(SqliteOrganizationRepository::new(pool.clone()));
        Self {
            usage: AssetUsageService::new(Arc::new(SqliteAssetUsageRepository::new(pool.clone()))),
            deletion: AssetDeletionService::new(
                asset_repo.clone(),
                Arc::new(SqliteAssetDeletionRepository::new(pool.clone())),
                projects.clone(),
                Arc::new(FileSystemAssetStore),
            ),
            data: AssetDataService::new(asset_repo.clone()),
            assets: AssetLibraryService::new(
                Arc::new(SqliteAssetBrowseRepository::new(pool.clone())),
                organization.clone(),
            ),
            asset_detail: AssetQueryService::new(
                asset_repo.clone(),
                Arc::new(FileSystemAssetStore),
                projects.clone(),
            )
            .with_organization_repository(organization),
            prompts: PromptLibraryService::new(
                Arc::new(SqlitePromptLibraryRepository::new(pool.clone())),
                Arc::new(SystemClock),
            ),
            profiles: ConsistencyProfileService::new(
                profile_repo.clone(),
                set_repo.clone(),
                projects.clone(),
                Arc::new(SystemClock),
            ),
            reference_sets: ReferenceSetService::new(
                set_repo,
                profile_repo,
                asset_repo,
                Arc::new(SqliteReferenceAnchorRepository::new(pool.clone())),
                projects,
                Arc::new(SystemClock),
            ),
            _dir: dir,
            pool,
        }
    }
    fn facade(&self) -> LibraryServices<'_> {
        LibraryServices {
            assets: &self.assets,
            asset_detail: &self.asset_detail,
            prompts: &self.prompts,
            profiles: &self.profiles,
            reference_sets: &self.reference_sets,
        }
    }
    fn operations(&self) -> LibraryOperations<'_> {
        LibraryOperations {
            library: self.facade(),
            usage: &self.usage,
            deletion: &self.deletion,
            data: &self.data,
        }
    }
    async fn asset(&self, id: &str, project: &str, media: &str) {
        let now = Utc::now().to_rfc3339();
        let root = self._dir.path().join(project);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("{id}.png"));
        std::fs::write(&path, b"fixture").unwrap();
        sqlx::query("INSERT INTO assets (id,project_id,type,category,name,original_name,storage_path,sha256,mime_type,width,height,file_size,metadata_json,created_at,updated_at) VALUES (?,?,?,?,?,?,?,'sha','image/png',512,512,1,'{}',?,?)")
            .bind(id).bind(project).bind(media).bind(format!("source_{media}")).bind(format!("素材 {id}")).bind(format!("{id}.png")).bind(path.to_string_lossy().as_ref()).bind(&now).bind(&now).execute(&self.pool).await.unwrap();
    }
    async fn profile(&self, title: &str) -> String {
        self.profiles
            .create_prop(CreatePropProfileRequest {
                project_id: "prj_default".into(),
                name: title.into(),
                description: "独立配置".into(),
                canonical_prompt: "wooden prop".into(),
                material_prompt: Some("oak".into()),
                scale_prompt: None,
                default_reference_set_id: None,
            })
            .await
            .unwrap()
            .id
    }
    async fn reference_set(&self, title: &str) -> String {
        self.reference_sets
            .create(CreateReferenceSetRequest {
                project_id: "prj_default".into(),
                name: title.into(),
                purpose: ReferenceSetPurpose::Prop,
                description: "ordered reference".into(),
                owner_profile_type: None,
                owner_profile_id: None,
                items: vec![ReferenceSetItemRequest {
                    asset_id: "ast_library_a".into(),
                    ordinal: 0,
                    role: Some("shape".into()),
                    is_primary: true,
                }],
            })
            .await
            .unwrap()
            .id
    }
}
fn query(category: LibraryCategory) -> LibraryQuery {
    LibraryQuery {
        category,
        keyword: None,
        cursor: None,
        limit: Some(2),
    }
}

impl Fixture {
    async fn task(&self, id: &str, project: &str, status: &str) {
        let now = Utc::now().to_rfc3339();
        for sql in [
            "INSERT OR IGNORE INTO workflows (id,name,category,mode,created_at,updated_at) VALUES ('wfl_library','图片生成','image','default',?,?)",
            "INSERT OR IGNORE INTO workflow_versions (id,workflow_id,version,api_workflow_json,workflow_sha256,created_at) VALUES ('wfv_library','wfl_library','1','{}','sha',?)",
            "INSERT OR IGNORE INTO recipes (id,workflow_version_id,version,schema_version,recipe_yaml,recipe_sha256,created_at) VALUES ('rcp_library','wfv_library','1',1,'schema_version: 1','sha',?)",
        ] {
            let mut q=sqlx::query(sql).bind(&now); if sql.contains("updated_at") {q=q.bind(&now);} q.execute(&self.pool).await.unwrap();
        }
        sqlx::query("INSERT INTO tasks (id,project_id,workflow_id,workflow_version_id,recipe_id,status,created_at,queued_at,started_at,finished_at) VALUES (?,?,'wfl_library','wfv_library','rcp_library',?,?,?,?,?)").bind(id).bind(project).bind(status).bind(&now).bind(&now).bind(&now).bind(if matches!(status,"SUCCEEDED"|"FAILED"|"CANCELLED") {Some(&now)}else{None}).execute(&self.pool).await.unwrap();
    }
    async fn snapshot(&self, task: &str, input: serde_json::Value, resolved: serde_json::Value) {
        sqlx::query("INSERT INTO generation_snapshots (id,task_id,workflow_json,recipe_yaml,user_inputs_json,resolved_inputs_json,created_at) VALUES (?,?,'{}','schema_version: 1',?,?,?)").bind(format!("snp_{task}")).bind(task).bind(input.to_string()).bind(resolved.to_string()).bind(Utc::now().to_rfc3339()).execute(&self.pool).await.unwrap();
    }
    async fn shot(&self, id: &str, asset: Option<&str>) {
        sqlx::query("INSERT INTO shots (id,project_id,ordinal,name,prompt_text,selected_image_asset_id,created_at,updated_at) VALUES (?,'prj_default',(SELECT COUNT(*) FROM shots),'开场近景','draft',?,?,?)").bind(id).bind(asset).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&self.pool).await.unwrap();
    }
    async fn batch(
        &self,
        id: &str,
        status: &str,
        values: serde_json::Value,
        task: Option<&str>,
    ) -> String {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO production_batches (id,project_id,name,status,created_at,updated_at) VALUES (?,'prj_default','验收批次','COMPLETED',?,?)").bind(id).bind(&now).bind(&now).execute(&self.pool).await.unwrap();
        let item = format!("pbi_{id}");
        sqlx::query("INSERT INTO production_batch_items (id,batch_id,ordinal,workflow_version_id,recipe_id,values_json,status,task_id,created_at,updated_at) VALUES (?,?,0,'wfv_library','rcp_library',?,?,?,?,?)").bind(&item).bind(id).bind(values.to_string()).bind(status).bind(task).bind(&now).bind(&now).execute(&self.pool).await.unwrap();
        item
    }
    async fn review(&self, batch: &str, item: &str, task: &str, asset: &str) {
        sqlx::query("INSERT INTO production_item_reviews (id,project_id,production_batch_id,production_batch_item_id,task_id,result_asset_id,version,lineage_key,created_at,updated_at) VALUES ('rev_library','prj_default',?,?,?,?,1,'history',?,?)").bind(batch).bind(item).bind(task).bind(asset).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&self.pool).await.unwrap();
    }
}
fn asset_ref(id: &str) -> ResourceRef {
    ResourceRef::Asset { id: id.into() }
}

#[tokio::test]
async fn phase5_target3_media_details_provenance_and_scope() {
    let f = Fixture::new().await;
    f.task("tsk_source", "prj_default", "SUCCEEDED").await;
    for media in ["image", "video", "audio"] {
        let id = format!("ast_{media}");
        f.asset(&id, "prj_default", media).await;
        sqlx::query(
            "UPDATE assets SET category='generated_'||type, source_task_id='tsk_source' WHERE id=?",
        )
        .bind(&id)
        .execute(&f.pool)
        .await
        .unwrap();
        let LibraryDetail::Asset { asset } = f
            .facade()
            .get("prj_default", &asset_ref(&id))
            .await
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(asset.asset_type, media);
        assert_eq!(asset.source_task_id.as_deref(), Some("tsk_source"));
        assert!(!serde_json::to_string(&asset).unwrap().contains("storage"));
        let relations = f
            .operations()
            .relations_get("prj_default", &asset_ref(&id))
            .await
            .unwrap();
        assert!(relations.iter().any(|r|matches!(&r.location,Some(LibraryRelationLocation::Run {run_ref}) if run_ref.id=="tsk_source")));
        assert!(f
            .operations()
            .relations_get("prj_11111111-1111-4111-8111-111111111111", &asset_ref(&id))
            .await
            .is_err());
    }
    assert_eq!(
        f.facade()
            .image_get("prj_default", &asset_ref("ast_image"))
            .await
            .unwrap(),
        b"fixture"
    );
    assert!(f
        .facade()
        .image_get(
            "prj_11111111-1111-4111-8111-111111111111",
            &asset_ref("ast_image")
        )
        .await
        .is_err());
    assert!(f
        .facade()
        .image_get("prj_default", &asset_ref("ast_video"))
        .await
        .is_err());
    f.shot("sht_library", Some("ast_image")).await;
    assert!(f
        .operations()
        .relations_get("prj_default", &asset_ref("ast_image"))
        .await
        .unwrap()
        .iter()
        .any(|r| r.kind == LibraryRelationKind::ShotSelectedResult));
}

#[tokio::test]
async fn phase5_target4_prompt_edit_and_immutable_versions() {
    let f = Fixture::new().await;
    let p = f
        .prompts
        .create("prj_default", "prompt", "提示词", &[], "original")
        .await
        .unwrap();
    let original = p.versions[0].clone();
    f.operations()
        .resource_edit(
            "prj_default",
            LibraryEditRequest::Prompt {
                id: p.id.clone(),
                text: "revised".into(),
                model_version_id: None,
            },
        )
        .await
        .unwrap();
    let LibraryVersions::Prompt { versions } = f
        .operations()
        .versions_get("prj_default", &ResourceRef::Prompt { id: p.id.clone() })
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(versions.len(), 2);
    assert!(versions[0].version > versions[1].version);
    assert_eq!(versions[1], original);
    assert!(f
        .operations()
        .resource_edit(
            "prj_11111111-1111-4111-8111-111111111111",
            LibraryEditRequest::Prompt {
                id: p.id,
                text: "foreign".into(),
                model_version_id: None
            }
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn phase5_target5_profile_typed_edit_preserves_configuration_and_scope() {
    let f = Fixture::new().await;
    let id = f.profile("道具").await;
    let detail = f
        .operations()
        .resource_edit(
            "prj_default",
            LibraryEditRequest::Profile {
                id: id.clone(),
                name: "新道具".into(),
            },
        )
        .await
        .unwrap();
    let wire = serde_json::to_value(detail).unwrap();
    assert_eq!(wire["profile"]["Prop"]["name"], "新道具");
    assert_eq!(wire["profile"]["Prop"]["material_prompt"], "oak");
    let r = ResourceRef::Profile { id };
    assert!(f
        .operations()
        .relations_get("prj_default", &r)
        .await
        .is_ok());
    assert!(f
        .operations()
        .deletion_inspect("prj_11111111-1111-4111-8111-111111111111", &r)
        .await
        .is_err());
    assert!(serde_json::from_str::<LibraryEditRequest>(
        r#"{"kind":"profile","id":"x","name":"y","arbitraryJson":{}}"#
    )
    .is_err());
}

#[tokio::test]
async fn phase5_target6_reference_set_edit_members_roles_and_usage() {
    let f = Fixture::new().await;
    f.asset("ast_library_a", "prj_default", "image").await;
    f.asset("ast_library_second", "prj_default", "image").await;
    let id = f.reference_set("参考集").await;
    f.operations()
        .resource_edit(
            "prj_default",
            LibraryEditRequest::ReferenceSet {
                id: id.clone(),
                name: "新参考集".into(),
                description: "ordered".into(),
                items: vec![
                    LibraryReferenceMemberEdit {
                        asset_id: "ast_library_second".into(),
                        ordinal: 0,
                        role: Some("material".into()),
                        is_primary: true,
                    },
                    LibraryReferenceMemberEdit {
                        asset_id: "ast_library_a".into(),
                        ordinal: 1,
                        role: Some("shape".into()),
                        is_primary: false,
                    },
                ],
            },
        )
        .await
        .unwrap();
    let LibraryDetail::ReferenceSet { reference_set: s } = f
        .facade()
        .get("prj_default", &ResourceRef::ReferenceSet { id: id.clone() })
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(s.items[0].asset_id, "ast_library_second");
    assert_eq!(s.items[1].role.as_deref(), Some("shape"));
    assert_eq!(s.purpose, ReferenceSetPurpose::Prop);
    assert!(s.owner_profile_id.is_none());
    let relations = f
        .operations()
        .relations_get("prj_default", &ResourceRef::ReferenceSet { id })
        .await
        .unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.kind == LibraryRelationKind::ReferenceSetMember)
            .count(),
        2
    );
}

#[tokio::test]
async fn phase5_target7_create_intents_do_not_create_tasks_or_queues() {
    let f = Fixture::new().await;
    for media in ["image", "video", "audio"] {
        let id = format!("ast_intent_{media}");
        f.asset(&id, "prj_default", media).await;
        let intent = f
            .operations()
            .use_in_creation("prj_default", &asset_ref(&id))
            .await
            .unwrap();
        assert!(matches!(intent,LibraryCreateIntent::Asset {media_kind,..} if media_kind==media));
    }
    let p = f
        .prompts
        .create("prj_default", "prompt", "提示词", &[], "draft")
        .await
        .unwrap();
    assert!(
        matches!(f.operations().use_in_creation("prj_default",&ResourceRef::Prompt {id:p.id}).await.unwrap(),LibraryCreateIntent::Prompt {text,..} if text=="draft")
    );
    let profile = f.profile("道具").await;
    assert!(matches!(
        f.operations()
            .use_in_creation("prj_default", &ResourceRef::Profile { id: profile })
            .await
            .unwrap(),
        LibraryCreateIntent::Context { .. }
    ));
    f.asset("ast_library_a", "prj_default", "image").await;
    let set = f.reference_set("参考集").await;
    assert!(matches!(
        f.operations()
            .use_in_creation("prj_default", &ResourceRef::ReferenceSet { id: set })
            .await
            .unwrap(),
        LibraryCreateIntent::Context { .. }
    ));
    for table in ["tasks", "production_batches", "production_batch_items"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn phase5_target9_persisted_relations_distinguish_inputs_and_outputs() {
    let f = Fixture::new().await;
    f.asset("ast_library_a", "prj_default", "image").await;
    for (n, status) in ["RUNNING", "FAILED", "SUCCEEDED", "CANCELLED"]
        .into_iter()
        .enumerate()
    {
        let task = format!("tsk_input_{n}");
        f.task(&task, "prj_default", status).await;
        f.snapshot(&task,serde_json::json!({}),serde_json::json!({"media":{"assetId":"ast_library_a","sha256":"sha","comfy":{"name":"uploaded.png"}}})).await;
    }
    f.shot("sht_relation", Some("ast_library_a")).await;
    let set = f.reference_set("参考集").await;
    let relations = f
        .operations()
        .relations_get("prj_default", &asset_ref("ast_library_a"))
        .await
        .unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.kind == LibraryRelationKind::GenerationSnapshotInput)
            .count(),
        4
    );
    assert!(relations
        .iter()
        .filter(|r| r.kind == LibraryRelationKind::GenerationSnapshotInput)
        .all(|r| r.blocking));
    assert!(relations
        .iter()
        .any(|r| r.kind == LibraryRelationKind::ShotSelectedResult));
    assert!(relations
        .iter()
        .any(|r| r.kind == LibraryRelationKind::ReferenceSetMember));
    let p = f
        .prompts
        .create("prj_default", "prompt", "精确提示词", &[], "draft")
        .await
        .unwrap();
    sqlx::query("UPDATE generation_snapshots SET prompt_version_id=? WHERE task_id='tsk_input_1'")
        .bind(&p.versions[0].id)
        .execute(&f.pool)
        .await
        .unwrap();
    let relations = f
        .operations()
        .relations_get("prj_default", &ResourceRef::Prompt { id: p.id })
        .await
        .unwrap();
    assert_eq!(relations.len(), 1);
    assert_eq!(
        relations[0].kind,
        LibraryRelationKind::GenerationSnapshotInput
    );
    assert!(
        !f.operations()
            .deletion_inspect("prj_default", &asset_ref("ast_library_a"))
            .await
            .unwrap()
            .allowed
    );
    assert!(f
        .operations()
        .relations_get(
            "prj_11111111-1111-4111-8111-111111111111",
            &ResourceRef::ReferenceSet { id: set }
        )
        .await
        .is_err());
}

#[tokio::test]
async fn phase5_target10_delete_inspection_frozen_policy_and_no_side_effects() {
    let f = Fixture::new().await;
    for id in [
        "ast_snapshot",
        "ast_live",
        "ast_selected",
        "ast_queue",
        "ast_history",
        "ast_library_a",
    ] {
        f.asset(id, "prj_default", "image").await;
    }
    f.task("tsk_input", "prj_default", "FAILED").await;
    f.snapshot(
        "tsk_input",
        serde_json::json!({"image":{"type":"image_asset","assetId":"ast_snapshot"}}),
        serde_json::json!({}),
    )
    .await;
    f.task("tsk_live", "prj_default", "RUNNING").await;
    sqlx::query("UPDATE assets SET category='generated_'||type, source_task_id='tsk_live' WHERE id='ast_live'")
        .execute(&f.pool)
        .await
        .unwrap();
    f.shot("sht_selected", Some("ast_selected")).await;
    f.reference_set("参考集").await;
    f.batch(
        "pb_live",
        "PENDING",
        serde_json::json!({"image":{"type":"image_asset","assetId":"ast_queue"}}),
        None,
    )
    .await;
    f.task("tsk_history", "prj_default", "SUCCEEDED").await;
    f.snapshot("tsk_history", serde_json::json!({}), serde_json::json!({}))
        .await;
    sqlx::query("UPDATE assets SET category='generated_'||type, source_task_id='tsk_history' WHERE id='ast_history'")
        .execute(&f.pool)
        .await
        .unwrap();
    let item = f
        .batch(
            "pb_history",
            "SUCCEEDED",
            serde_json::json!({}),
            Some("tsk_history"),
        )
        .await;
    f.review("pb_history", &item, "tsk_history", "ast_history")
        .await;
    for id in [
        "ast_snapshot",
        "ast_live",
        "ast_selected",
        "ast_queue",
        "ast_library_a",
    ] {
        let i = f
            .operations()
            .deletion_inspect("prj_default", &asset_ref(id))
            .await
            .unwrap();
        assert!(!i.allowed, "{id}");
        assert!(!i.blockers.is_empty());
    }
    let history = f
        .operations()
        .deletion_inspect("prj_default", &asset_ref("ast_history"))
        .await
        .unwrap();
    assert!(history.allowed);
    assert_eq!(history.warnings.len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assets")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        6
    );
    // Classify actual trigger-created rows; inspection never mutates them.
    for (n, decision, revision, comment, allowed) in [
        (0, "PENDING", 0, "", true),
        (1, "PENDING", 0, " \t\n", true),
        (2, "APPROVED", 1, "", false),
        (3, "REJECTED", 1, "needs another take", false),
        (4, "PENDING", 2, "", false),
        (5, "PENDING", 0, "human note", false),
    ] {
        let id = format!("ast_review_{n}");
        f.asset(&id, "prj_default", "image").await;
        sqlx::query("INSERT INTO task_output_assets(task_id,asset_id,output_id,ordinal,created_at) VALUES ('tsk_history',?,'image',?,?)")
            .bind(&id).bind(n).bind(Utc::now().to_rfc3339()).execute(&f.pool).await.unwrap();
        sqlx::query(
            "UPDATE artifact_reviews SET decision=?, revision=?, comment=? WHERE artifact_id=?",
        )
        .bind(decision)
        .bind(revision)
        .bind(comment)
        .bind(&id)
        .execute(&f.pool)
        .await
        .unwrap();
        let inspection = f
            .operations()
            .deletion_inspect("prj_default", &asset_ref(&id))
            .await
            .unwrap();
        assert_eq!(
            inspection.allowed, allowed,
            "{decision}/{revision}/{comment}"
        );
        let kind = if allowed {
            LibraryRelationKind::ArtifactReviewPlaceholder
        } else {
            LibraryRelationKind::ArtifactReviewMeaningful
        };
        assert!(inspection
            .relations
            .iter()
            .any(|r| r.kind == kind && r.blocking == !allowed));
        let row: (String, i64, String) = sqlx::query_as(
            "SELECT decision,revision,comment FROM artifact_reviews WHERE artifact_id=?",
        )
        .bind(&id)
        .fetch_one(&f.pool)
        .await
        .unwrap();
        assert_eq!(row, (decision.into(), revision, comment.into()));
        if !allowed {
            assert!(inspection.blockers.iter().any(|b| b.contains("审核记录")));
        }
    }
    assert!(f
        .operations()
        .deletion_inspect(
            "prj_11111111-1111-4111-8111-111111111111",
            &asset_ref("ast_history")
        )
        .await
        .is_err());
    assert!(f
        .operations()
        .delete("prj_default", &asset_ref("ast_history"), false)
        .await
        .is_err());
}

#[tokio::test]
async fn phase5_target11_delete_revalidates_and_preserves_terminal_history() {
    let f = Fixture::new().await;
    f.asset("ast_race", "prj_default", "image").await;
    assert!(
        f.operations()
            .deletion_inspect("prj_default", &asset_ref("ast_race"))
            .await
            .unwrap()
            .allowed
    );
    f.task("tsk_race", "prj_default", "SUCCEEDED").await;
    f.snapshot(
        "tsk_race",
        serde_json::json!({"image":{"type":"image_asset","assetId":"ast_race"}}),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        f.operations()
            .delete("prj_default", &asset_ref("ast_race"), true)
            .await
            .unwrap_err()
            .code,
        "LIBRARY_DELETE_BLOCKED"
    );
    assert!(f
        .facade()
        .get("prj_default", &asset_ref("ast_race"))
        .await
        .is_ok());
    f.asset("ast_output", "prj_default", "video").await;
    f.task("tsk_output", "prj_default", "SUCCEEDED").await;
    f.snapshot("tsk_output", serde_json::json!({}), serde_json::json!({}))
        .await;
    sqlx::query("UPDATE assets SET category='generated_'||type, source_task_id='tsk_output' WHERE id='ast_output'")
        .execute(&f.pool)
        .await
        .unwrap();
    let item = f
        .batch(
            "pb_output",
            "SUCCEEDED",
            serde_json::json!({}),
            Some("tsk_output"),
        )
        .await;
    f.review("pb_output", &item, "tsk_output", "ast_output")
        .await;
    f.operations()
        .delete("prj_default", &asset_ref("ast_output"), true)
        .await
        .unwrap();
    assert_eq!(
        f.facade()
            .get("prj_default", &asset_ref("ast_output"))
            .await
            .unwrap_err()
            .code,
        "LIBRARY_RESOURCE_NOT_FOUND"
    );
    for sql in [
        "SELECT COUNT(*) FROM tasks WHERE id='tsk_output'",
        "SELECT COUNT(*) FROM generation_snapshots WHERE task_id='tsk_output'",
        "SELECT COUNT(*) FROM production_item_reviews WHERE id='rev_library'",
        "SELECT COUNT(*) FROM production_batches WHERE id='pb_output'",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            1
        );
    }
    // Real task_output_assets inserts create an Artifact review via schema 038.
    // This differs from legacy production_item_reviews: cascade would destroy it.
    f.asset("ast_reviewed", "prj_default", "image").await;
    assert!(
        f.operations()
            .deletion_inspect("prj_default", &asset_ref("ast_reviewed"))
            .await
            .unwrap()
            .allowed
    );
    sqlx::query("INSERT INTO task_output_assets(task_id,asset_id,output_id,ordinal,created_at) VALUES ('tsk_output','ast_reviewed','image',0,?)")
        .bind(Utc::now().to_rfc3339()).execute(&f.pool).await.unwrap();
    let reviewed = f
        .operations()
        .deletion_inspect("prj_default", &asset_ref("ast_reviewed"))
        .await
        .unwrap();
    assert!(
        reviewed.allowed,
        "automatic placeholder must not block deletion"
    );
    sqlx::query("UPDATE artifact_reviews SET decision='APPROVED',revision=1 WHERE artifact_id='ast_reviewed'")
        .execute(&f.pool).await.unwrap();
    let reviewed = f
        .operations()
        .deletion_inspect("prj_default", &asset_ref("ast_reviewed"))
        .await
        .unwrap();
    assert!(!reviewed.allowed);
    assert!(reviewed
        .blockers
        .iter()
        .any(|message| message.contains("审核历史")));
    assert_eq!(
        f.operations()
            .delete("prj_default", &asset_ref("ast_reviewed"), true)
            .await
            .unwrap_err()
            .code,
        "LIBRARY_DELETE_BLOCKED"
    );
    let repo = SqliteAssetRepository::new(f.pool.clone());
    assert!(repo
        .delete_by_ids(
            "prj_default",
            &[ai_studio_lib::domain::AssetId::parse("ast_reviewed").unwrap()]
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM artifact_reviews WHERE artifact_id='ast_reviewed'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    assert!(f
        .facade()
        .get("prj_default", &asset_ref("ast_reviewed"))
        .await
        .is_ok());
    f.asset("ast_placeholder", "prj_default", "image").await;
    sqlx::query("INSERT INTO task_output_assets(task_id,asset_id,output_id,ordinal,created_at) VALUES ('tsk_output','ast_placeholder','image',1,?)")
        .bind(Utc::now().to_rfc3339()).execute(&f.pool).await.unwrap();
    assert!(
        f.operations()
            .deletion_inspect("prj_default", &asset_ref("ast_placeholder"))
            .await
            .unwrap()
            .allowed
    );
    f.operations()
        .delete("prj_default", &asset_ref("ast_placeholder"), true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM artifact_reviews WHERE artifact_id='ast_placeholder'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    for sql in [
        "SELECT COUNT(*) FROM tasks WHERE id='tsk_output'",
        "SELECT COUNT(*) FROM generation_snapshots WHERE task_id='tsk_output'",
        "SELECT COUNT(*) FROM production_batches WHERE id='pb_output'",
        "SELECT COUNT(*) FROM production_item_reviews WHERE id='rev_library'",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            1
        );
    }
    let query = ai_studio_lib::application::task_query_service::TaskQueryService::new(
        Arc::new(SqliteTaskRepository::new(f.pool.clone())),
        Arc::new(SqliteAssetRepository::new(f.pool.clone())),
        Arc::new(SqliteGenerationDefinitionRepository::new(f.pool.clone())),
    );
    assert!(query
        .get("prj_default", "tsk_output")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn phase5_target1_typed_kinds_and_project_isolation() {
    let f = Fixture::new().await;
    f.asset("ast_library_a", "prj_default", "image").await;
    f.asset(
        "ast_library_b",
        "prj_11111111-1111-4111-8111-111111111111",
        "image",
    )
    .await;
    let prompt = f
        .prompts
        .create("prj_default", "prompt", "提示词", &[], "draft")
        .await
        .unwrap();
    let refs = vec![
        ResourceRef::Asset {
            id: "ast_library_a".into(),
        },
        ResourceRef::Prompt { id: prompt.id },
        ResourceRef::Profile {
            id: f.profile("道具").await,
        },
        ResourceRef::ReferenceSet {
            id: f.reference_set("参考集").await,
        },
    ];
    let list = f
        .facade()
        .list("prj_default", query(LibraryCategory::All))
        .await
        .unwrap();
    assert_eq!(list.items.len(), 4);
    assert_eq!(list.coverage, LibraryCoverage::RecentSummary);
    for r in refs {
        assert!(list.items.iter().any(|i| i.resource_ref == r));
        assert!(f.facade().get("prj_default", &r).await.is_ok());
        assert_eq!(
            f.facade()
                .get("prj_11111111-1111-4111-8111-111111111111", &r)
                .await
                .unwrap_err()
                .code,
            "LIBRARY_RESOURCE_NOT_FOUND"
        );
    }
    let b = f
        .facade()
        .list(
            "prj_11111111-1111-4111-8111-111111111111",
            query(LibraryCategory::All),
        )
        .await
        .unwrap();
    assert_eq!(b.items.len(), 1);
    assert_eq!(b.items[0].resource_ref.id(), "ast_library_b");
    assert!(serde_json::from_str::<ResourceRef>(r#"{"kind":"resource","id":"x"}"#).is_err());
    assert!(
        serde_json::from_str::<ResourceRef>(r#"{"kind":"asset","id":"x","projectId":"b"}"#)
            .is_err()
    );
}

#[tokio::test]
async fn phase5_target2_pagination_search_and_honest_coverage() {
    let f = Fixture::new().await;
    f.asset("ast_library_a", "prj_default", "image").await;
    for n in 0..15 {
        f.asset(
            &format!("ast_library_{n:02}"),
            "prj_default",
            if n % 2 == 0 { "image" } else { "video" },
        )
        .await;
        f.prompts
            .create(
                "prj_default",
                "prompt",
                &format!("提示词 {n:02}"),
                &[],
                "draft",
            )
            .await
            .unwrap();
        f.profile(&format!("道具 {n:02}")).await;
        f.reference_set(&format!("参考集 {n:02}")).await;
    }
    for (category, expected) in [
        (LibraryCategory::Media, 16),
        (LibraryCategory::Images, 9),
        (LibraryCategory::Videos, 7),
        (LibraryCategory::Prompts, 15),
    ] {
        let mut q = query(category);
        let mut seen = HashSet::new();
        loop {
            let page = f.facade().list("prj_default", q).await.unwrap();
            assert_eq!(page.coverage, LibraryCoverage::KeysetPage);
            for item in page.items {
                assert!(
                    seen.insert(item.resource_ref.id().to_owned()),
                    "duplicate page item"
                );
            }
            match page.next_cursor {
                Some(cursor) => {
                    let mut wrong = query(category);
                    wrong.cursor = Some(cursor.clone());
                    wrong.keyword = Some("changed".into());
                    assert_eq!(
                        f.facade()
                            .list("prj_default", wrong)
                            .await
                            .unwrap_err()
                            .code,
                        "LIBRARY_QUERY_INVALID"
                    );
                    let mut foreign = query(category);
                    foreign.cursor = Some(cursor.clone());
                    assert!(f
                        .facade()
                        .list("prj_11111111-1111-4111-8111-111111111111", foreign)
                        .await
                        .is_err());
                    q = query(category);
                    q.cursor = Some(cursor);
                }
                None => break,
            }
        }
        assert_eq!(seen.len(), expected);
    }
    for category in [LibraryCategory::Profiles, LibraryCategory::ReferenceSets] {
        let full = f
            .facade()
            .list("prj_default", query(category))
            .await
            .unwrap();
        assert_eq!(full.coverage, LibraryCoverage::CompleteCategory);
        assert_eq!(full.items.len(), 15);
        assert!(full.next_cursor.is_none());
        let mut search = query(category);
        search.keyword = Some(" 14 ".into());
        assert_eq!(
            f.facade()
                .list("prj_default", search)
                .await
                .unwrap()
                .items
                .len(),
            1
        );
    }
    let all = f
        .facade()
        .list("prj_default", query(LibraryCategory::All))
        .await
        .unwrap();
    assert_eq!(all.coverage, LibraryCoverage::RecentSummary);
    assert_eq!(all.items.len(), 48);
    assert!(all.next_cursor.is_none());
    let mut search = query(LibraryCategory::Prompts);
    search.keyword = Some("14".into());
    assert_eq!(
        f.facade()
            .list("prj_default", search)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let mut search = query(LibraryCategory::Images);
    search.keyword = Some("ast_library_14".into());
    assert_eq!(
        f.facade()
            .list("prj_default", search)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn phase5_target8_typed_detail_preserves_versions_config_and_member_roles() {
    let f = Fixture::new().await;
    f.asset("ast_library_a", "prj_default", "image").await;
    let p = f
        .prompts
        .create("prj_default", "prompt", "提示词", &[], "version one")
        .await
        .unwrap();
    f.prompts
        .add_version("prj_default", &p.id, "version two")
        .await
        .unwrap();
    let refs = [
        ResourceRef::Asset {
            id: "ast_library_a".into(),
        },
        ResourceRef::Prompt { id: p.id },
        ResourceRef::Profile {
            id: f.profile("道具").await,
        },
        ResourceRef::ReferenceSet {
            id: f.reference_set("参考集").await,
        },
    ];
    for r in refs {
        let detail = f.facade().get("prj_default", &r).await.unwrap();
        let wire = serde_json::to_value(&detail).unwrap();
        assert!(!wire.to_string().contains("storage_path"));
        match detail {
            LibraryDetail::Asset { asset } => {
                assert_eq!(asset.asset_type, "image");
                assert_eq!(asset.width, Some(512));
            }
            LibraryDetail::Prompt { prompt } => {
                assert_eq!(prompt.versions.len(), 2);
                assert!(prompt.versions.iter().any(|v| v.text == "version one"));
            }
            LibraryDetail::Profile { profile } => {
                assert_eq!(wire["profile"]["Prop"]["material_prompt"], "oak");
                assert_eq!(profile.name(), "道具");
            }
            LibraryDetail::ReferenceSet { reference_set } => {
                assert_eq!(wire["kind"], "reference-set");
                assert_eq!(reference_set.items[0].role.as_deref(), Some("shape"));
                assert_eq!(reference_set.items[0].ordinal, 0);
                assert!(reference_set.items[0].is_primary);
            }
        }
    }
    let max: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(max, 42);
}
