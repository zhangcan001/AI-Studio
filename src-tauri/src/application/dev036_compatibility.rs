//! DEV-036 Agent-D deterministic template-engine E2E tests.
//!
//! Kept inside the application module so the tests exercise the real parser
//! and renderer without widening the production library's public API.  The
//! fixture is intentionally in-memory: no database, network, ComfyUI, or GPU
//! is involved.

use super::project_backup_service::ProjectBackupService;
use super::prompt_template_service::{
    analyze_prompt_template, parse_prompt_template, render_prompt_template,
    PROMPT_TEMPLATE_CONTEXT_MISSING, PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING,
    PROMPT_TEMPLATE_RESULT_TOO_LARGE, PROMPT_TEMPLATE_SYNTAX_ERROR,
    PROMPT_TEMPLATE_UNKNOWN_VARIABLE,
};
use crate::domain::{
    PromptAnchor, PromptAnchorContext, PromptAnchorKind, PromptProjectContext, PromptShotContext,
    PromptStructureContext, PromptTemplateContext,
};
use crate::infrastructure::database::initialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use tempfile::tempdir;
use zip::ZipArchive;

fn context(shot_number: u32) -> PromptTemplateContext {
    PromptTemplateContext::new(
        PromptProjectContext {
            id: "project-dev036".to_owned(),
            name: "地藏经".to_owned(),
            description: None,
        },
        PromptShotContext {
            id: format!("shot-{shot_number:03}"),
            name: format!("镜头 {shot_number}"),
            number: shot_number,
            base_prompt: "电影级画面".to_owned(),
        },
    )
    .with_series(PromptStructureContext {
        id: "series-1".to_owned(),
        name: "第一季".to_owned(),
        description: String::new(),
        number: 1,
    })
    .with_episode(PromptStructureContext {
        id: "episode-1".to_owned(),
        name: "第一集".to_owned(),
        description: String::new(),
        number: 1,
    })
    .with_scene(PromptStructureContext {
        id: "scene-1".to_owned(),
        name: "忉利天宫".to_owned(),
        description: "佛陀于忉利天为母说法".to_owned(),
        number: 1,
    })
    .with_anchors(PromptAnchorContext::from_selected([
        (
            PromptAnchorKind::Character,
            PromptAnchor {
                id: "anchor-character".to_owned(),
                name: "释迦牟尼佛".to_owned(),
                description: "成熟庄严佛相，金色袈裟".to_owned(),
            },
        ),
        (
            PromptAnchorKind::Style,
            PromptAnchor {
                id: "anchor-style".to_owned(),
                name: "电影写实".to_owned(),
                description: "庄严神圣，体积光".to_owned(),
            },
        ),
    ]))
}

#[test]
fn dev036_parser_and_renderer_cover_production_context_and_anchors() {
    let mut custom_values = BTreeMap::new();
    custom_values.insert("camera".to_owned(), "中景缓慢推进".to_owned());
    let template = "项目：{{project.name}}\n系列：{{series.name}}\n第{{episode.number}}集：{{episode.name}}\n场景：{{scene.name}}\n场景设定：{{scene.description}}\n镜头：{{shot.name}}\n角色：{{anchors.character.context}}\n风格：{{anchors.style.context}}\n摄影：{{custom.camera}}";
    let rendered = render_prompt_template(template, &context(1).with_custom_values(custom_values))
        .expect("DEV-036 fixture should render");

    for expected in [
        "项目：地藏经",
        "系列：第一季",
        "第1集：第一集",
        "场景：忉利天宫",
        "场景设定：佛陀于忉利天为母说法",
        "镜头：镜头 1",
        "角色：释迦牟尼佛：成熟庄严佛相，金色袈裟",
        "风格：电影写实：庄严神圣，体积光",
        "摄影：中景缓慢推进",
    ] {
        assert!(
            rendered.contains(expected),
            "rendered prompt missing {expected}"
        );
    }

    let analysis = analyze_prompt_template(template).expect("template analysis should succeed");
    assert!(analysis.is_template);
    assert!(analysis.requires_structure);
    assert!(analysis.custom_variables.contains(&"camera".to_owned()));
    assert_eq!(analysis.variables[0], "project.name");
}

#[test]
fn dev036_parser_rejects_syntax_and_unknown_variables_deterministically() {
    for (text, code) in [
        ("{{scene.name", PROMPT_TEMPLATE_SYNTAX_ERROR),
        ("{{ }}", PROMPT_TEMPLATE_SYNTAX_ERROR),
        ("{{scene/name}}", PROMPT_TEMPLATE_SYNTAX_ERROR),
        ("{{scene.location}}", PROMPT_TEMPLATE_UNKNOWN_VARIABLE),
    ] {
        assert_eq!(parse_prompt_template(text).unwrap_err().code(), code);
    }
}

#[test]
fn dev036_missing_context_custom_values_and_optional_anchors_have_explicit_contracts() {
    let mut without_scene = context(1);
    without_scene.scene = None;
    assert_eq!(
        render_prompt_template("{{scene.name}}", &without_scene)
            .unwrap_err()
            .code(),
        PROMPT_TEMPLATE_CONTEXT_MISSING
    );
    assert_eq!(
        render_prompt_template("{{custom.camera}}", &context(1))
            .unwrap_err()
            .code(),
        PROMPT_TEMPLATE_CUSTOM_VALUE_MISSING
    );
    assert_eq!(
        render_prompt_template("{{anchors.prop.context}}", &context(1))
            .expect("optional anchor context should be empty"),
        ""
    );
}

#[test]
fn dev036_renderer_is_plain_text_and_canonicalizes_line_endings() {
    let rendered = render_prompt_template(
        "  a\r\nb\r  {{custom.value}}",
        &context(1).with_custom_values(
            [("value".to_owned(), "{{shot.name}}".to_owned())]
                .into_iter()
                .collect(),
        ),
    )
    .expect("plain text substitution should succeed");
    assert_eq!(rendered, "a\nb\n  {{shot.name}}");
}

#[test]
fn dev036_renderer_handles_500_shots_without_live_context_queries() {
    let template = "{{scene.name}} - {{shot.name}} - {{custom.style}}";
    for shot_number in 1..=500 {
        let rendered = render_prompt_template(
            template,
            &context(shot_number).with_custom_values(
                [("style".to_owned(), "电影写实".to_owned())]
                    .into_iter()
                    .collect(),
            ),
        )
        .expect("500-shot deterministic render should succeed");
        assert_eq!(
            rendered,
            format!("忉利天宫 - 镜头 {shot_number} - 电影写实")
        );
    }
}

#[test]
fn dev036_renderer_enforces_expanded_prompt_limit() {
    let values = [("value".to_owned(), "x".repeat(4096))]
        .into_iter()
        .collect();
    let error = render_prompt_template(
        &"{{custom.value}}".repeat(17),
        &context(1).with_custom_values(values),
    )
    .unwrap_err();
    assert_eq!(error.code(), PROMPT_TEMPLATE_RESULT_TOO_LARGE);
}

#[tokio::test]
async fn dev036_backup_v17_preserves_template_source_and_frozen_stage_snapshot() {
    let directory = tempdir().expect("temporary directory should be created");
    let pool = initialize(&directory.path().join("app.db"))
        .await
        .expect("DEV-036 SQLite migration should succeed");
    sqlx::query(
        "INSERT INTO projects
         (id, name, description, root_path, created_at, updated_at)
         VALUES ('dev036-backup', '地藏经', '', 'C:/dev036-backup', ?, ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup project should insert");
    sqlx::query(
        "INSERT INTO prompt_entries
         (id, project_id, kind, name, normalized_name, tags_json, created_at, updated_at)
         VALUES ('dev036-entry', 'dev036-backup', 'prompt', '模板', '模板', '[]', ?, ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup prompt entry should insert");
    sqlx::query(
        "INSERT INTO prompt_versions (id, prompt_id, version, text, created_at)
         VALUES ('dev036-version', 'dev036-entry', 1, '{{scene.name}} {{shot.name}}', ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup prompt version should insert");
    sqlx::query(
        "INSERT INTO shots
         (id, project_id, ordinal, name, prompt_text, prompt_entry_id, prompt_version_id,
          selected_image_asset_id, selected_video_asset_id, created_at, updated_at)
         VALUES ('dev036-backup-shot', 'dev036-backup', 0, '佛陀端坐', '',
                 'dev036-entry', 'dev036-version', NULL, NULL, ?, ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup shot should insert");
    sqlx::query(
        "INSERT INTO shot_stage_prompts
         (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
         VALUES ('dev036-backup-shot', 'image', '忉利天宫 佛陀端坐',
                  'dev036-entry', 'dev036-version', ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup frozen prompt should insert");
    sqlx::query(
        "INSERT INTO shot_stage_prompts
         (shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at)
         VALUES ('dev036-backup-shot', 'video', '忉利天宫 佛陀端坐视频',
                 'dev036-entry', 'dev036-version', ?)",
    )
    .bind("2026-08-18T00:00:00Z")
    .execute(&pool)
    .await
    .expect("backup video prompt should insert");

    let service = ProjectBackupService::new(
        pool,
        directory.path().join("projects"),
        directory.path().join("cache"),
    );
    let archive_path = directory.path().join("dev036-v17.zip");
    service
        .export("dev036-backup", archive_path.clone())
        .await
        .expect("v17 backup should export");
    let inspection = service
        .inspect(archive_path.clone())
        .await
        .expect("v17 backup should inspect");
    assert_eq!(inspection.prompt_entries, 1);
    assert_eq!(inspection.shots, 1);

    let file = File::open(archive_path).expect("backup archive should open");
    let mut archive = ZipArchive::new(file).expect("backup archive should parse");
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .expect("manifest should exist")
        .read_to_string(&mut manifest)
        .expect("manifest should be readable");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&manifest).expect("manifest should be JSON")
            ["version"],
        17
    );
    let mut project = String::new();
    archive
        .by_name("project.json")
        .expect("project document should exist")
        .read_to_string(&mut project)
        .expect("project document should be readable");
    let document: serde_json::Value =
        serde_json::from_str(&project).expect("project document should be JSON");
    assert_eq!(
        document["promptVersions"][0]["text"],
        "{{scene.name}} {{shot.name}}"
    );
    assert_eq!(
        document["shotStagePrompts"][0]["promptText"],
        "忉利天宫 佛陀端坐"
    );
}
