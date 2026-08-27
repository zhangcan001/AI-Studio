//! DEV-057 Agent-D: Backup 15, compatibility, isolation, and capacity gates.

use ai_studio_lib::application::ports::ScriptDraftPageQuery;
use ai_studio_lib::application::project_backup_service::ProjectBackupService;
use ai_studio_lib::application::project_manifest_service::ProjectManifestService;
use ai_studio_lib::application::script_draft_service::{
    AppendScriptDraftRevisionRequest, CreateScriptDraftRequest, ScriptDraftService,
    ScriptSourceCreateRequest,
};
use ai_studio_lib::domain::script_draft::{
    canonical_json, canonical_sha256, validate_structure, DraftEpisode, DraftId, DraftNodeId,
    DraftNodeOrigin, DraftReviewState, DraftRevisionId, DraftRevisionKind, DraftScene, DraftShot,
    DraftStructureV1, SourceId,
};
use ai_studio_lib::infrastructure::database::{
    SqliteScriptDraftRepository, SqliteScriptSourceRepository,
};
use ai_studio_lib::infrastructure::filesystem::AppDataDirs;
use ai_studio_lib::infrastructure::time::SystemClock;
use ai_studio_lib::initialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{collections::HashMap, fs, io::Read, path::Path, sync::Arc, time::Instant};
use tempfile::{tempdir, TempDir};
use uuid::Uuid;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

const CREATED_AT: &str = "2026-08-27T00:00:00Z";

fn id(prefix: &str) -> String {
    format!("{prefix}{}", Uuid::new_v4())
}

async fn database() -> (TempDir, AppDataDirs, SqlitePool) {
    let directory = tempdir().expect("DEV-057 temp directory should exist");
    let dirs = AppDataDirs::initialize(directory.path().join("AIStudioData")).unwrap();
    let pool = initialize(&dirs.database)
        .await
        .expect("DEV-057 database should initialize through the real migrator");
    (directory, dirs, pool)
}

async fn insert_project(pool: &SqlitePool, id: &str, root: &Path) {
    fs::create_dir_all(root).unwrap();
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES (?, 'DEV-057 Project', 'Backup 15 fixture', ?, ?, ?)",
    )
    .bind(id)
    .bind(root.to_string_lossy().to_string())
    .bind(CREATED_AT)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn empty_structure(draft_id: &str, source_id: &str, revision_id: &str) -> DraftStructureV1 {
    DraftStructureV1::new(
        DraftId::parse(draft_id).unwrap(),
        SourceId::parse(source_id).unwrap(),
        DraftRevisionId::parse(revision_id).unwrap(),
    )
}

fn large_structure(draft_id: &str, source_id: &str, revision_id: &str) -> DraftStructureV1 {
    let mut structure = empty_structure(draft_id, source_id, revision_id);
    structure.episodes = (0..100)
        .map(|episode_ordinal| {
            let episode_id = DraftNodeId::new();
            let scenes = (0..10)
                .map(|scene_ordinal| {
                    let scene_id = DraftNodeId::new();
                    let shots = (0..5)
                        .map(|shot_ordinal| DraftShot {
                            draft_node_id: DraftNodeId::new(),
                            parent_draft_node_id: Some(scene_id.clone()),
                            parent_scene_draft_id: scene_id.clone(),
                            ordinal: shot_ordinal,
                            name: format!("Shot {episode_ordinal}-{scene_ordinal}-{shot_ordinal}"),
                            purpose: Some("capacity benchmark".to_owned()),
                            description: None,
                            characters: Vec::new(),
                            scene_suggestion: None,
                            props: Vec::new(),
                            action: Some("hold".to_owned()),
                            dialogue: Vec::new(),
                            camera_suggestion: None,
                            lighting_suggestion: None,
                            duration_suggestion: Some(1.0),
                            image_prompt_draft: Some("draft image".to_owned()),
                            video_prompt_draft: Some("draft video".to_owned()),
                            source_spans: Vec::new(),
                            diagnostics: Vec::new(),
                            review_state: DraftReviewState::PendingReview,
                            origin: DraftNodeOrigin::Imported,
                            original_suggestion: None,
                            current_value: None,
                        })
                        .collect();
                    DraftScene {
                        draft_node_id: scene_id,
                        parent_draft_node_id: Some(episode_id.clone()),
                        ordinal: scene_ordinal,
                        name: format!("Scene {episode_ordinal}-{scene_ordinal}"),
                        description: None,
                        source_spans: Vec::new(),
                        diagnostics: Vec::new(),
                        review_state: DraftReviewState::PendingReview,
                        origin: DraftNodeOrigin::Imported,
                        original_suggestion: None,
                        current_value: None,
                        location_suggestion: None,
                        time_suggestion: None,
                        shots,
                    }
                })
                .collect();
            DraftEpisode {
                draft_node_id: episode_id,
                parent_draft_node_id: None,
                ordinal: episode_ordinal,
                name: format!("Episode {episode_ordinal}"),
                description: None,
                source_spans: Vec::new(),
                diagnostics: Vec::new(),
                review_state: DraftReviewState::PendingReview,
                origin: DraftNodeOrigin::Imported,
                original_suggestion: None,
                current_value: None,
                scenes,
            }
        })
        .collect();
    structure
}

async fn insert_source(pool: &SqlitePool, project_id: &str, source_id: &str, text: &str) {
    sqlx::query(
        "INSERT INTO script_sources
         (id, project_id, format, original_filename, source_checksum, source_bytes, source_text,
          schema_version, created_at)
         VALUES (?, ?, 'TXT', 'script.txt', ?, ?, ?, 1, ?)",
    )
    .bind(source_id)
    .bind(project_id)
    .bind(sha256(text.as_bytes()))
    .bind(text.len() as i64)
    .bind(text)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
}

fn draft_service(pool: &SqlitePool) -> ScriptDraftService {
    ScriptDraftService::new(
        Arc::new(SqliteScriptSourceRepository::new(pool.clone())),
        Arc::new(SqliteScriptDraftRepository::new(pool.clone())),
        Arc::new(SystemClock),
    )
}

async fn revision_count(pool: &SqlitePool, project_id: &str, draft_id: &DraftId) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM script_import_drafts WHERE project_id = ? AND draft_id = ?",
    )
    .bind(project_id)
    .bind(draft_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_revision(
    pool: &SqlitePool,
    project_id: &str,
    source_id: &str,
    draft_id: &str,
    revision: i64,
    revision_id: &str,
    previous_revision_id: Option<&str>,
    structure: &DraftStructureV1,
) -> String {
    let payload_json = canonical_json(structure).unwrap();
    let payload_checksum = sha256(payload_json.as_bytes());
    sqlx::query(
        "INSERT INTO script_import_drafts
         (id, draft_id, project_id, source_id, revision, previous_revision_id,
          schema_version, revision_kind, parser_version, contract_version,
          provider_kind, provider_model, provider_metadata_json,
          payload_checksum, summary_json, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 1, 'PARSED', 'dev057-fixture', 1,
                 'offline', NULL, ?, ?, ?, ?, ?)",
    )
    .bind(revision_id)
    .bind(draft_id)
    .bind(project_id)
    .bind(source_id)
    .bind(revision)
    .bind(previous_revision_id)
    .bind(r#"{"providerKind":"offline","modelLabel":null,"promptContractVersion":null,"inputChecksum":null,"outputChecksum":null,"metadata":{}}"#)
    .bind(payload_checksum)
    .bind(
        canonical_json(&json!({
            "episodes": structure.counts().episodes,
            "scenes": structure.counts().scenes,
            "shots": structure.counts().shots,
        }))
        .unwrap(),
    )
    .bind(&payload_json)
    .bind(CREATED_AT)
    .execute(pool)
    .await
    .unwrap();
    payload_json
}

fn rewrite_legacy_archive(source: &Path, destination: &Path, version: u32) {
    let mut input = ZipArchive::new(fs::File::open(source).unwrap()).unwrap();
    let mut entries = Vec::new();
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.push((name, bytes));
    }
    let file = fs::File::create(destination).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = FileOptions::default();
    for (name, bytes) in entries {
        let bytes = if name == "manifest.json" {
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value["version"] = json!(version);
            serde_json::to_vec(&value).unwrap()
        } else if name == "project.json" {
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value.as_object_mut().unwrap().remove("scriptSources");
            value
                .as_object_mut()
                .unwrap()
                .remove("scriptDraftRevisions");
            serde_json::to_vec(&value).unwrap()
        } else {
            bytes
        };
        writer.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut writer, &bytes).unwrap();
    }
    writer.finish().unwrap();
}

#[tokio::test]
async fn backup15_roundtrip_remaps_sources_revisions_and_previous_links() {
    let (directory, dirs, pool) = database().await;
    let project_id = "dev057-backup15";
    insert_project(&pool, project_id, &directory.path().join("source-project")).await;
    let source_ids = [id("scr_"), id("scr_"), id("scr_")];
    let draft_id = id("drf_");
    let revision_ids = [id("drev_"), id("drev_"), id("drev_")];
    let source_texts = [
        "DEV057 source A — preserved once",
        "DEV057 source B — preserved once",
        "DEV057 source C — preserved once",
    ];
    for (source_id, source_text) in source_ids.iter().zip(source_texts) {
        insert_source(&pool, project_id, source_id, source_text).await;
    }
    let mut payloads = Vec::new();
    for (index, revision_id) in revision_ids.iter().enumerate() {
        let structure = empty_structure(&draft_id, &source_ids[index], revision_id);
        payloads.push(
            insert_revision(
                &pool,
                project_id,
                &source_ids[index],
                &draft_id,
                (index + 1) as i64,
                revision_id,
                index
                    .checked_sub(1)
                    .map(|previous| revision_ids[previous].as_str()),
                &structure,
            )
            .await,
        );
    }

    let script_service = draft_service(&pool);
    let source_metadata = script_service.list_sources(project_id).await.unwrap();
    assert_eq!(source_metadata.len(), 3);
    assert!(source_metadata
        .iter()
        .all(|source| source.original_filename.as_deref() == Some("script.txt")));
    let history = script_service
        .history(
            project_id,
            &DraftId::parse(&draft_id).unwrap(),
            ScriptDraftPageQuery {
                cursor: None,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(history.items.len(), 3);
    assert!(history
        .items
        .iter()
        .all(|item| !item.payload_checksum.is_empty() && !item.summary_json.is_empty()));

    let service =
        ProjectBackupService::new(pool.clone(), dirs.projects.clone(), dirs.cache.clone());
    let archive = directory.path().join("backup15.zip");
    service.export(project_id, archive.clone()).await.unwrap();
    let document = read_zip_json(&archive, "project.json");
    assert_eq!(read_zip_json(&archive, "manifest.json")["version"], 15);
    assert_eq!(document["scriptSources"].as_array().unwrap().len(), 3);
    assert_eq!(
        document["scriptDraftRevisions"].as_array().unwrap().len(),
        3
    );
    for source_text in source_texts {
        assert!(document["scriptSources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["sourceText"] == source_text));
    }
    let document_text = serde_json::to_string(&document).unwrap();
    for source_text in source_texts {
        assert_eq!(document_text.matches(source_text).count(), 1);
    }

    let preview = service.inspect(archive).await.unwrap();
    assert_eq!(preview.project_name, "DEV-057 Project");
    let restored = service.restore(&preview.inspection_id).await.unwrap();
    let restored_sources: Vec<(String, Option<String>, String, i64, String)> = sqlx::query_as(
        "SELECT id, original_filename, source_checksum, source_bytes, source_text
         FROM script_sources WHERE project_id = ?",
    )
    .bind(&restored.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(restored_sources.len(), 3);
    let restored_source_ids: HashMap<String, String> = restored_sources
        .iter()
        .map(|(id, filename, checksum, bytes, text)| {
            assert_eq!(filename.as_deref(), Some("script.txt"));
            assert_eq!(*bytes, text.len() as i64);
            assert_eq!(*checksum, sha256(text.as_bytes()));
            (checksum.clone(), id.clone())
        })
        .collect();
    for (source_id, source_text) in source_ids.iter().zip(source_texts) {
        let checksum = sha256(source_text.as_bytes());
        let restored_id = restored_source_ids.get(&checksum).unwrap();
        assert_ne!(restored_id, source_id);
        let restored_source = restored_sources
            .iter()
            .find(|row| row.0 == *restored_id)
            .unwrap();
        assert_eq!(restored_source.4, source_text);
    }

    let restored_rows: Vec<(String, String, String, i64, Option<String>, String, String)> =
        sqlx::query_as(
            "SELECT id, draft_id, source_id, revision, previous_revision_id,
                    payload_checksum, payload_json
             FROM script_import_drafts WHERE project_id = ? ORDER BY revision",
        )
        .bind(&restored.id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(restored_rows.len(), 3);
    for (index, row) in restored_rows.iter().enumerate() {
        assert_ne!(row.0, revision_ids[index]);
        assert_ne!(row.1, draft_id);
        let expected_checksum = sha256(source_texts[index].as_bytes());
        assert_eq!(
            row.2.as_str(),
            restored_source_ids
                .get(&expected_checksum)
                .expect("each revision source must be restored")
                .as_str()
        );
        assert_eq!(row.3, (index + 1) as i64);
        assert_eq!(
            row.4.as_deref(),
            index
                .checked_sub(1)
                .map(|previous| restored_rows[previous].0.as_str())
        );
        assert_eq!(row.5, sha256(row.6.as_bytes()));
        assert_ne!(row.6, payloads[index]);
        let structure: DraftStructureV1 = serde_json::from_str(&row.6).unwrap();
        assert_eq!(structure.draft_id.to_string(), row.1);
        assert_eq!(structure.source_id.to_string(), row.2);
        assert_eq!(structure.revision_id.to_string(), row.0);
    }
}

#[tokio::test]
async fn backup14_13_12_and_manifest2_import_compatibility_hold() {
    let (directory, dirs, pool) = database().await;
    let project_id = "dev057-compat";
    insert_project(&pool, project_id, &directory.path().join("compat-project")).await;
    let service =
        ProjectBackupService::new(pool.clone(), dirs.projects.clone(), dirs.cache.clone());
    let current = directory.path().join("backup15.zip");
    service.export(project_id, current.clone()).await.unwrap();
    for version in [14, 13, 12] {
        let legacy = directory.path().join(format!("backup{version}.zip"));
        rewrite_legacy_archive(&current, &legacy, version);
        let preview = service.inspect(legacy).await.unwrap();
        let restored = service.restore(&preview.inspection_id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM script_import_drafts WHERE project_id = ?",
            )
            .bind(&restored.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    let manifest_path = directory.path().join("manifest-v2.json");
    ProjectManifestService::new(pool.clone())
        .export(project_id, manifest_path.clone())
        .await
        .unwrap();
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let manifest_json: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest_json["version"], 2);
    for excluded in ["scriptSources", "scriptDrafts", "sourceText", "payloadJson"] {
        assert!(
            manifest_json.get(excluded).is_none(),
            "Manifest must exclude {excluded}"
        );
    }
    assert_eq!(
        ProjectManifestService::parse(&manifest_bytes)
            .unwrap()
            .version,
        2
    );
}

#[tokio::test]
async fn reparsed_revisions_can_cross_sources_without_losing_provenance() {
    let (directory, _dirs, pool) = database().await;
    let project_id = "dev057-reparse";
    insert_project(&pool, project_id, &directory.path().join("reparse-project")).await;
    let service = draft_service(&pool);

    let source_a = service
        .create_source(ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "第一版剧本".as_bytes().to_vec(),
            original_filename: Some("a.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_b = service
        .create_source(ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "第二版剧本".as_bytes().to_vec(),
            original_filename: Some("b.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_c = service
        .create_source(ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "第三版剧本".as_bytes().to_vec(),
            original_filename: Some("c.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_d = service
        .create_source(ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "第四版剧本".as_bytes().to_vec(),
            original_filename: Some("d.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_e = service
        .create_source(ScriptSourceCreateRequest {
            project_id: project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "第五版剧本".as_bytes().to_vec(),
            original_filename: Some("e.txt".to_owned()),
        })
        .await
        .unwrap();
    assert_ne!(source_a.id, source_b.id);
    assert_ne!(source_b.id, source_c.id);
    assert_ne!(source_c.id, source_d.id);
    assert_ne!(source_d.id, source_e.id);

    let revision1 = service
        .create_draft(CreateScriptDraftRequest {
            project_id: project_id.to_owned(),
            source_id: source_a.id.clone(),
            structure: DraftStructureV1::new(
                DraftId::new(),
                source_a.id.clone(),
                DraftRevisionId::new(),
            ),
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();
    assert_eq!(revision1.metadata.revision, 1);
    assert_eq!(revision1.metadata.source_id, source_a.id);
    assert_eq!(revision1.metadata.revision_kind, DraftRevisionKind::Parsed);

    let mut structure_b = revision1.structure.clone();
    structure_b.source_id = source_b.id.clone();
    let revision2 = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 1,
            structure: structure_b,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();
    assert_eq!(revision2.metadata.draft_id, revision1.metadata.draft_id);
    assert_eq!(revision2.metadata.revision, 2);
    assert_eq!(revision2.metadata.source_id, source_b.id);
    assert_eq!(
        revision2.metadata.revision_kind,
        DraftRevisionKind::Reparsed
    );
    assert_eq!(
        revision2.metadata.previous_revision_id,
        Some(revision1.metadata.id.clone())
    );

    let mut structure_c = revision2.structure.clone();
    structure_c.source_id = source_c.id.clone();
    let revision3 = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 2,
            structure: structure_c,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();
    assert_eq!(revision3.metadata.revision, 3);
    assert_eq!(revision3.metadata.source_id, source_c.id);
    assert_eq!(
        revision3.metadata.previous_revision_id,
        Some(revision2.metadata.id.clone())
    );
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        3
    );

    let same_source_retry = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 3,
            structure: revision3.structure.clone(),
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();
    assert_eq!(same_source_retry.metadata.id, revision3.metadata.id);
    assert_eq!(same_source_retry.metadata.revision, 3);
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        3
    );

    let mut structure_d = revision3.structure.clone();
    structure_d.source_id = source_d.id.clone();
    let revision4 = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 3,
            structure: structure_d,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();
    assert_eq!(revision4.metadata.revision, 4);
    assert_eq!(revision4.metadata.source_id, source_d.id);
    assert_eq!(
        revision4.metadata.previous_revision_id,
        Some(revision3.metadata.id.clone())
    );
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        4
    );

    let other_project_id = "dev057-reparse-other";
    insert_project(
        &pool,
        other_project_id,
        &directory.path().join("reparse-other-project"),
    )
    .await;
    let other_source = service
        .create_source(ScriptSourceCreateRequest {
            project_id: other_project_id.to_owned(),
            format: ai_studio_lib::domain::script_draft::ScriptFormat::Txt,
            source_text: "其他项目剧本".as_bytes().to_vec(),
            original_filename: Some("other.txt".to_owned()),
        })
        .await
        .unwrap();
    let mut cross_project_structure = revision4.structure.clone();
    cross_project_structure.source_id = other_source.id;
    let cross_project_error = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 4,
            structure: cross_project_structure,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap_err();
    assert!(cross_project_error.message.starts_with("SOURCE_NOT_FOUND"));
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        4
    );

    let mut missing_source_structure = revision4.structure.clone();
    missing_source_structure.source_id = SourceId::new();
    let missing_source_error = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 4,
            structure: missing_source_structure,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap_err();
    assert!(missing_source_error.message.starts_with("SOURCE_NOT_FOUND"));
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        4
    );

    let mut stale_structure = revision4.structure.clone();
    stale_structure.source_id = source_e.id;
    let stale_error = service
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: project_id.to_owned(),
            draft_id: revision1.metadata.draft_id.clone(),
            expected_revision: 3,
            structure: stale_structure,
            revision_kind: DraftRevisionKind::Reparsed,
            parser_version: "dev057-parser".to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap_err();
    assert!(stale_error.message.starts_with("DRAFT_REVISION_CONFLICT"));
    assert_eq!(
        revision_count(&pool, project_id, &revision1.metadata.draft_id).await,
        4
    );

    for (revision, expected_source, expected_previous) in [
        (&revision1, &source_a.id, None),
        (
            &revision2,
            &source_b.id,
            Some(revision1.metadata.id.clone()),
        ),
        (
            &revision3,
            &source_c.id,
            Some(revision2.metadata.id.clone()),
        ),
        (
            &revision4,
            &source_d.id,
            Some(revision3.metadata.id.clone()),
        ),
    ] {
        let loaded = service
            .get_revision(
                project_id,
                &revision1.metadata.draft_id,
                revision.metadata.revision,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.metadata.id, revision.metadata.id);
        assert_eq!(loaded.metadata.draft_id, revision1.metadata.draft_id);
        assert_eq!(loaded.metadata.source_id, *expected_source);
        assert_eq!(loaded.metadata.previous_revision_id, expected_previous);
        assert_eq!(
            loaded.metadata.payload_checksum,
            revision.metadata.payload_checksum
        );
        assert_eq!(loaded.structure, revision.structure);
    }
}

#[tokio::test]
async fn delete_cascade_and_restore_have_no_production_or_comfy_side_effects() {
    let (directory, _dirs, pool) = database().await;
    let project_id = "dev057-delete";
    insert_project(&pool, project_id, &directory.path().join("delete-project")).await;
    let source_id = id("scr_");
    let draft_id = id("drf_");
    let revision_id = id("drev_");
    insert_source(&pool, project_id, &source_id, "cascade source").await;
    let structure = empty_structure(&draft_id, &source_id, &revision_id);
    insert_revision(
        &pool,
        project_id,
        &source_id,
        &draft_id,
        1,
        &revision_id,
        None,
        &structure,
    )
    .await;
    let before: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
          (SELECT COUNT(*) FROM production_series WHERE project_id = ?),
          (SELECT COUNT(*) FROM production_episodes WHERE series_id IN (SELECT id FROM production_series WHERE project_id = ?)),
          (SELECT COUNT(*) FROM production_scenes WHERE episode_id IN (SELECT id FROM production_episodes WHERE series_id IN (SELECT id FROM production_series WHERE project_id = ?))),
          (SELECT COUNT(*) FROM shots WHERE project_id = ?),
          (SELECT COUNT(*) FROM shot_scene_assignments WHERE scene_id IN (SELECT id FROM production_scenes WHERE episode_id IN (SELECT id FROM production_episodes WHERE series_id IN (SELECT id FROM production_series WHERE project_id = ?)))),
          (SELECT COUNT(*) FROM shot_profile_bindings WHERE shot_id IN (SELECT id FROM shots WHERE project_id = ?)),
          (SELECT COUNT(*) FROM production_batches WHERE project_id = ?),
          (SELECT COUNT(*) FROM production_batch_items WHERE batch_id IN (SELECT id FROM production_batches WHERE project_id = ?)),
          (SELECT COUNT(*) FROM tasks WHERE project_id = ?),
          (SELECT COUNT(*) FROM assets WHERE project_id = ?),
          (SELECT COUNT(*) FROM workflows)",
    )
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0));

    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM script_sources WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM script_import_drafts WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM script_import_drafts WHERE draft_id = ?"
        )
        .bind(draft_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM script_sources WHERE id = ?")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn capacity_5000_draft_benchmark_reports_measured_stages() {
    let (directory, _dirs, pool) = database().await;
    let project_id = "dev057-benchmark";
    insert_project(
        &pool,
        project_id,
        &directory.path().join("benchmark-project"),
    )
    .await;
    let source_id = id("scr_");
    let draft_id = id("drf_");
    let revision_id = id("drev_");
    let source = "benchmark source";
    insert_source(&pool, project_id, &source_id, source).await;
    let structure = large_structure(&draft_id, &source_id, &revision_id);
    assert_eq!(structure.counts().episodes, 100);
    assert_eq!(structure.counts().scenes, 1000);
    assert_eq!(structure.counts().shots, 5000);

    let serialize_start = Instant::now();
    let payload_json = canonical_json(&structure).unwrap();
    let serialize_ms = serialize_start.elapsed().as_millis();
    let validate_start = Instant::now();
    validate_structure(&structure, source.as_bytes(), 1).unwrap();
    let validate_ms = validate_start.elapsed().as_millis();
    let hash_start = Instant::now();
    let payload_checksum = canonical_sha256(&structure).unwrap();
    let hash_ms = hash_start.elapsed().as_millis();
    let insert_start = Instant::now();
    let stored_payload = insert_revision(
        &pool,
        project_id,
        &source_id,
        &draft_id,
        1,
        &revision_id,
        None,
        &structure,
    )
    .await;
    let sqlite_insert_ms = insert_start.elapsed().as_millis();
    let load_start = Instant::now();
    let loaded: (String, String) = sqlx::query_as(
        "SELECT payload_checksum, payload_json FROM script_import_drafts WHERE id = ?",
    )
    .bind(&revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let sqlite_load_ms = load_start.elapsed().as_millis();
    let deserialize_start = Instant::now();
    let decoded: DraftStructureV1 = serde_json::from_str(&loaded.1).unwrap();
    let deserialize_ms = deserialize_start.elapsed().as_millis();
    assert_eq!(stored_payload, loaded.1);
    assert_eq!(payload_checksum, loaded.0);
    assert_eq!(decoded.counts(), structure.counts());
    let load_deserialize_ms = sqlite_load_ms + deserialize_ms;
    let payload_bytes = payload_json.len();
    eprintln!(
        "DEV057_BENCHMARK payload_bytes={payload_bytes} serialize_ms={serialize_ms} validate_ms={validate_ms} hash_ms={hash_ms} sqlite_insert_ms={sqlite_insert_ms} sqlite_load_ms={sqlite_load_ms} deserialize_ms={deserialize_ms}"
    );
    if payload_bytes > 64 * 1024 * 1024 || load_deserialize_ms > 2000 {
        panic!(
            "DEV-057 BLOCKED — real metrics exceeded threshold: payload_bytes={payload_bytes}, load_deserialize_ms={load_deserialize_ms}, sqlite_load_ms={sqlite_load_ms}, deserialize_ms={deserialize_ms}; DRAFT_NODE_INDEX not added"
        );
    } else {
        eprintln!("DRAFT_NODE_INDEX=NOT_NEEDED_V1");
    }
}

fn read_zip_json(path: &Path, entry_name: &str) -> Value {
    let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut entry = archive.by_name(entry_name).unwrap();
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
