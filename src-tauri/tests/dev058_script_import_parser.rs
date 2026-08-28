//! DEV-058 integration gates for the Draft-only import boundary.

use ai_studio_lib::application::script_draft_service::{
    AppendScriptDraftRevisionRequest, ScriptDraftService, ScriptSourceCreateRequest,
};
use ai_studio_lib::application::script_import_parser::reconcile;
use ai_studio_lib::application::script_import_parser::{
    parse_source, ParseCancellationToken, ScriptParseOptions, SCRIPT_IMPORT_PARSER_VERSION,
};
use ai_studio_lib::application::script_import_service::ScriptImportService;
use ai_studio_lib::domain::script_draft::{
    DraftNodeOrigin, DraftReviewState, DraftRevisionKind, ScriptFormat,
};
use ai_studio_lib::infrastructure::database::{
    initialize, SqliteScriptDraftRepository, SqliteScriptSourceRepository,
};
use ai_studio_lib::infrastructure::time::SystemClock;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

async fn fixture() -> (
    tempfile::TempDir,
    SqlitePool,
    Arc<ScriptDraftService>,
    ScriptImportService,
) {
    let directory = tempdir().expect("temporary directory should exist");
    let pool = initialize(&directory.path().join("dev058.db"))
        .await
        .expect("database should initialize");
    sqlx::query(
        "INSERT INTO projects (id, name, description, root_path, created_at, updated_at)
         VALUES ('dev058-project', 'DEV-058', NULL, 'C:/dev058', ?, ?)",
    )
    .bind("2026-01-01T00:00:00Z")
    .bind("2026-01-01T00:00:00Z")
    .execute(&pool)
    .await
    .expect("project fixture should insert");
    let drafts = Arc::new(ScriptDraftService::new(
        Arc::new(SqliteScriptSourceRepository::new(pool.clone())),
        Arc::new(SqliteScriptDraftRepository::new(pool.clone())),
        Arc::new(SystemClock),
    ));
    let imports = ScriptImportService::new(drafts.clone());
    (directory, pool, drafts, imports)
}

#[tokio::test]
async fn preview_create_reparse_and_same_source_noop_are_draft_only() {
    let (_directory, pool, drafts, imports) = fixture().await;
    let source = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Txt,
            source_text: "第一集\n\n场景1\n\n张三：你好".as_bytes().to_vec(),
            original_filename: Some("story.txt".to_owned()),
        })
        .await
        .unwrap();

    let preview = imports
        .preview_source(
            "dev058-project",
            &source.id,
            ScriptParseOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        preview.document.parser_version,
        SCRIPT_IMPORT_PARSER_VERSION
    );
    assert_eq!(
        preview.document.source_storage_ref,
        format!("sqlite:script_sources/{}", source.id)
    );
    assert!(preview.can_persist);
    assert_eq!(preview.statistics.episode_count, 1);
    assert_eq!(preview.statistics.scene_count, 1);
    assert_eq!(preview.statistics.shot_count, 1);

    let created = imports
        .create_draft_from_source("dev058-project", &source.id, ScriptParseOptions::default())
        .await
        .unwrap();
    assert_eq!(created.metadata.revision, 1);
    assert_eq!(created.metadata.revision_kind, DraftRevisionKind::Parsed);

    let retry = imports
        .reparse_draft(
            "dev058-project",
            &created.metadata.draft_id,
            &source.id,
            1,
            ScriptParseOptions::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(retry.revision.metadata.revision, 1);
    assert_eq!(revision_count(&pool, &created.metadata.draft_id).await, 1);

    let formal_tables = [
        "production_series",
        "production_episodes",
        "production_scenes",
        "shots",
        "character_profiles",
        "scene_profiles",
        "prop_profiles",
        "style_profiles",
        "reference_sets",
        "shot_profile_bindings",
        "production_batches",
        "production_batch_items",
        "tasks",
        "assets",
    ];
    for table in formal_tables {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "parser touched formal table {table}");
    }
}

#[tokio::test]
async fn reparse_preserves_human_value_and_updates_source_span() {
    let (_directory, _pool, drafts, imports) = fixture().await;
    let source_a = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Txt,
            source_text: "第一集\n\n场景1\n\n张三：你好".as_bytes().to_vec(),
            original_filename: Some("a.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_b = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Txt,
            source_text: "第一集\n\n场景1\n\n张三：你好\n".as_bytes().to_vec(),
            original_filename: Some("b.txt".to_owned()),
        })
        .await
        .unwrap();
    let created = imports
        .create_draft_from_source(
            "dev058-project",
            &source_a.id,
            ScriptParseOptions::default(),
        )
        .await
        .unwrap();
    let mut edited = created.structure.clone();
    let episode = edited.episodes.first_mut().unwrap();
    episode.origin = DraftNodeOrigin::Human;
    episode.review_state = DraftReviewState::Edited;
    episode.current_value = Some("用户修改后的内容".to_owned());
    drafts
        .append_revision(AppendScriptDraftRevisionRequest {
            project_id: "dev058-project".to_owned(),
            draft_id: created.metadata.draft_id.clone(),
            expected_revision: 1,
            structure: edited,
            revision_kind: DraftRevisionKind::UserEdit,
            parser_version: SCRIPT_IMPORT_PARSER_VERSION.to_owned(),
            provider_metadata: None,
        })
        .await
        .unwrap();

    let reparsed = imports
        .reparse_draft(
            "dev058-project",
            &created.metadata.draft_id,
            &source_b.id,
            2,
            ScriptParseOptions::default(),
            None,
        )
        .await
        .unwrap();
    let episode = &reparsed.revision.structure.episodes[0];
    assert_eq!(episode.current_value.as_deref(), Some("用户修改后的内容"));
    assert_eq!(episode.review_state, DraftReviewState::Edited);
    assert_eq!(episode.origin, DraftNodeOrigin::Human);
    assert!(episode
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "REPARSE_HUMAN_EDIT_PRESERVED"));
    assert_eq!(reparsed.revision.metadata.source_id, source_b.id);
    assert_eq!(reparsed.revision.metadata.revision, 3);
}

#[tokio::test]
async fn cancellation_is_fail_closed_before_any_revision_write() {
    let (_directory, pool, drafts, imports) = fixture().await;
    let source = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Markdown,
            source_text: b"# Episode\n\n## Scene\n\ntext".to_vec(),
            original_filename: Some("story.md".to_owned()),
        })
        .await
        .unwrap();
    let cancellation = ParseCancellationToken::default();
    cancellation.cancel();
    let error = match imports
        .preview_source(
            "dev058-project",
            &source.id,
            ScriptParseOptions::default(),
            Some(&cancellation),
        )
        .await
    {
        Ok(_) => panic!("cancelled preview must fail"),
        Err(error) => error,
    };
    assert!(error.message.starts_with("SCRIPT_PARSE_CANCELLED"));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM script_import_drafts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn reparse_blocker_does_not_append_a_revision() {
    let (_directory, pool, drafts, imports) = fixture().await;
    let source_a = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Txt,
            source_text: "第一集\n\n场景1\n\n张三：你好".as_bytes().to_vec(),
            original_filename: Some("valid.txt".to_owned()),
        })
        .await
        .unwrap();
    let source_b = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Json,
            source_text: br#"{"schemaVersion":2,"title":"bad","episodes":[]}"#.to_vec(),
            original_filename: Some("invalid.json".to_owned()),
        })
        .await
        .unwrap();
    let created = imports
        .create_draft_from_source(
            "dev058-project",
            &source_a.id,
            ScriptParseOptions::default(),
        )
        .await
        .unwrap();

    let error = match imports
        .reparse_draft(
            "dev058-project",
            &created.metadata.draft_id,
            &source_b.id,
            1,
            ScriptParseOptions::default(),
            None,
        )
        .await
    {
        Ok(_) => panic!("a blocker must fail closed"),
        Err(error) => error,
    };
    assert!(error.message.contains("UNKNOWN_JSON_SCHEMA"));
    assert_eq!(revision_count(&pool, &created.metadata.draft_id).await, 1);
}

#[test]
fn reconcile_reports_retained_added_removed_and_changed_nodes() {
    let source_id = ai_studio_lib::domain::SourceId::new();
    let old = parse_source(
        &source_id,
        "dev058-project",
        ScriptFormat::Json,
        Some("diff.json"),
        diff_json("old", true, false).as_bytes(),
        &ScriptParseOptions::default(),
        None,
    )
    .unwrap()
    .structure
    .unwrap();
    let next = parse_source(
        &source_id,
        "dev058-project",
        ScriptFormat::Json,
        Some("diff.json"),
        diff_json("new", false, true).as_bytes(),
        &ScriptParseOptions::default(),
        None,
    )
    .unwrap()
    .structure
    .unwrap();
    let old_keep_id = old.episodes[0].scenes[0].shots[0].draft_node_id.clone();
    let result = reconcile::reconcile(&old, next, true);

    assert_eq!(result.diff.retained_nodes, 3);
    assert_eq!(result.diff.added_nodes, 1);
    assert_eq!(result.diff.removed_nodes, 1);
    assert_eq!(result.diff.changed_nodes, 3);
    assert_eq!(
        result.structure.episodes[0].scenes[0].shots[0].draft_node_id,
        old_keep_id
    );
}

#[tokio::test]
async fn preview_handles_the_frozen_5000_node_shape() {
    let (_directory, _pool, drafts, imports) = fixture().await;
    let raw = five_thousand_node_json().into_bytes();
    let source_bytes = raw.len();
    let source = drafts
        .create_source(ScriptSourceCreateRequest {
            project_id: "dev058-project".to_owned(),
            format: ScriptFormat::Json,
            source_text: raw.clone(),
            original_filename: Some("large.json".to_owned()),
        })
        .await
        .unwrap();

    let parse_started = Instant::now();
    let parsed = parse_source(
        &source.id,
        "dev058-project",
        ScriptFormat::Json,
        Some("large.json"),
        &raw,
        &ScriptParseOptions::default(),
        None,
    )
    .unwrap();
    let parse_milliseconds = parse_started.elapsed().as_millis();
    let source_block_count = parsed.source_blocks.len();
    let structure = parsed.structure.unwrap();
    let validate_started = Instant::now();
    structure
        .validate(
            &raw,
            ai_studio_lib::domain::script_draft::DRAFT_SCHEMA_VERSION,
        )
        .unwrap();
    let validate_milliseconds = validate_started.elapsed().as_millis();
    let reconcile_started = Instant::now();
    let reconciled = reconcile::reconcile(&structure, structure.clone(), true);
    let reconcile_milliseconds = reconcile_started.elapsed().as_millis();
    assert_eq!(reconciled.structure.counts().shots, 5_000);

    let started = Instant::now();
    let preview = imports
        .preview_source(
            "dev058-project",
            &source.id,
            ScriptParseOptions::default(),
            None,
        )
        .await
        .unwrap();
    let total_milliseconds = started.elapsed().as_millis();
    let counts = preview.structure.as_ref().unwrap().counts();
    assert_eq!(counts.episodes, 100);
    assert_eq!(counts.scenes, 1_000);
    assert_eq!(counts.shots, 5_000);
    assert!(preview.can_persist);
    assert_eq!(preview.statistics.shot_count, 5_000);
    eprintln!(
        "DEV-058 5000-node preview: source_bytes={source_bytes} source_blocks={source_block_count} parse_ms={parse_milliseconds} reconcile_ms={reconcile_milliseconds} validate_ms={validate_milliseconds} total_ms={total_milliseconds}"
    );
}

fn diff_json(action: &str, include_removed: bool, include_added: bool) -> String {
    let mut shots =
        format!("{{\"sourceId\":\"shot-keep\",\"name\":\"Keep\",\"action\":\"{action}\"}}");
    if include_removed {
        shots.push_str(", {\"sourceId\":\"shot-remove\",\"name\":\"Remove\"}");
    }
    if include_added {
        shots.push_str(", {\"sourceId\":\"shot-add\",\"name\":\"Add\"}");
    }
    format!(
        "{{\"schemaVersion\":1,\"title\":\"Diff\",\"episodes\":[{{\"sourceId\":\"episode-1\",\"name\":\"Episode\",\"scenes\":[{{\"sourceId\":\"scene-1\",\"name\":\"Scene\",\"shots\":[{shots}]}}]}}]}}"
    )
}

fn five_thousand_node_json() -> String {
    let mut episodes = String::new();
    for episode_index in 0..100 {
        if episode_index > 0 {
            episodes.push(',');
        }
        episodes.push_str(&format!(
            "{{\"sourceId\":\"episode-{episode_index}\",\"name\":\"Episode {episode_index}\",\"scenes\":["
        ));
        for scene_index in 0..10 {
            if scene_index > 0 {
                episodes.push(',');
            }
            episodes.push_str(&format!(
                "{{\"sourceId\":\"episode-{episode_index}-scene-{scene_index}\",\"name\":\"Scene {scene_index}\",\"shots\":["
            ));
            for shot_index in 0..5 {
                if shot_index > 0 {
                    episodes.push(',');
                }
                episodes.push_str(&format!(
                    "{{\"sourceId\":\"episode-{episode_index}-scene-{scene_index}-shot-{shot_index}\",\"name\":\"Shot {shot_index}\"}}"
                ));
            }
            episodes.push_str("]}");
        }
        episodes.push_str("]}");
    }
    format!("{{\"schemaVersion\":1,\"title\":\"Large draft\",\"episodes\":[{episodes}]}}")
}

async fn revision_count(pool: &SqlitePool, draft_id: &ai_studio_lib::domain::DraftId) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM script_import_drafts WHERE draft_id = ?")
        .bind(draft_id.to_string())
        .fetch_one(pool)
        .await
        .unwrap()
}
