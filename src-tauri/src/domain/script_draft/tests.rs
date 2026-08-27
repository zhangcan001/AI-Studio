use super::*;
use serde_json::json;

fn source_bytes() -> &'static [u8] {
    "第一行\n第二行".as_bytes()
}

fn draft_with_one_shot() -> DraftStructureV1 {
    let draft_id = DraftId::new();
    let source_id = SourceId::new();
    let episode_id = DraftNodeId::new();
    let scene_id = DraftNodeId::new();
    let shot_id = DraftNodeId::new();
    DraftStructureV1 {
        schema_version: DRAFT_SCHEMA_VERSION,
        contract_version: DRAFT_CONTRACT_VERSION,
        draft_id,
        source_id,
        revision_id: DraftRevisionId::new(),
        status: DraftStatus::Draft,
        episodes: vec![DraftEpisode {
            draft_node_id: episode_id.clone(),
            parent_draft_node_id: None,
            ordinal: 0,
            name: "第一集".to_owned(),
            description: None,
            source_spans: vec![SourceSpan::new(0, 3)],
            diagnostics: Vec::new(),
            review_state: DraftReviewState::PendingReview,
            origin: DraftNodeOrigin::Imported,
            original_suggestion: None,
            current_value: None,
            scenes: vec![DraftScene {
                draft_node_id: scene_id.clone(),
                parent_draft_node_id: Some(episode_id.clone()),
                ordinal: 0,
                name: "室内".to_owned(),
                description: None,
                source_spans: vec![SourceSpan::new(0, 3)],
                diagnostics: Vec::new(),
                review_state: DraftReviewState::PendingReview,
                origin: DraftNodeOrigin::Imported,
                original_suggestion: None,
                current_value: None,
                scene_mention: None,
                shots: vec![DraftShot {
                    draft_node_id: shot_id,
                    parent_draft_node_id: Some(scene_id.clone()),
                    parent_scene_draft_id: scene_id.clone(),
                    ordinal: 0,
                    name: "开场".to_owned(),
                    purpose: Some("建立空间".to_owned()),
                    description: None,
                    character_mentions: Vec::new(),
                    scene_mention: None,
                    prop_mentions: Vec::new(),
                    action: Some("人物走入".to_owned()),
                    dialogue: None,
                    camera_suggestion: Some("中景".to_owned()),
                    lighting_suggestion: None,
                    duration_suggestion: Some(2.0),
                    image_prompt_draft: Some("室内晨光".to_owned()),
                    video_prompt_draft: None,
                    source_spans: vec![SourceSpan::new(0, 3)],
                    diagnostics: Vec::new(),
                    review_state: DraftReviewState::AiSuggested,
                    origin: DraftNodeOrigin::Ai,
                    original_suggestion: Some("开场".to_owned()),
                    current_value: None,
                }],
            }],
        }],
        diagnostics: Vec::new(),
        metadata: Default::default(),
    }
}

#[test]
fn ids_are_backend_owned_and_strictly_typed() {
    let source = SourceId::new();
    let draft = DraftId::new();
    let revision = DraftRevisionId::new();
    let node = DraftNodeId::new();
    let diagnostic = DiagnosticId::new();
    assert!(source.as_str().starts_with("scr_"));
    assert!(draft.as_str().starts_with("drf_"));
    assert!(revision.as_str().starts_with("drev_"));
    assert!(node.as_str().starts_with("dnode_"));
    assert!(diagnostic.as_str().starts_with("diag_"));
    assert!(DraftId::parse(source.as_str()).is_err());
    assert!(DraftNodeId::parse("dnode_not-an-id").is_err());
    assert_eq!(
        serde_json::to_string(&source).unwrap(),
        format!("\"{}\"", source)
    );
}

#[test]
fn format_storage_contract_is_screaming_snake_case() {
    assert_eq!(
        serde_json::to_string(&ScriptFormat::Txt).unwrap(),
        "\"TXT\""
    );
    assert_eq!(
        serde_json::to_string(&ScriptFormat::Markdown).unwrap(),
        "\"MARKDOWN\""
    );
    assert_eq!(
        serde_json::from_str::<ScriptFormat>("\"JSON\"").unwrap(),
        ScriptFormat::Json
    );
    assert!(ScriptFormat::try_from_storage("markdown").is_err());
}

#[test]
fn source_checksum_uses_raw_utf8_bytes() {
    let raw = "aé".as_bytes();
    let source = ScriptDocument::from_raw_bytes(
        SourceId::new(),
        raw,
        ScriptFormat::Txt,
        "managed://source/scr",
    )
    .unwrap();
    assert_eq!(source.source_length, raw.len() as u64);
    assert_eq!(source.source_checksum, source_checksum(raw));
    assert!(
        ScriptDocument::from_raw_bytes(SourceId::new(), &[0xff], ScriptFormat::Txt, "ref").is_err()
    );
}

#[test]
fn source_spans_are_zero_based_end_exclusive_and_utf8_safe() {
    let raw = "甲乙".as_bytes();
    assert!(SourceSpan::new(0, 3).validate(raw).is_ok());
    assert!(SourceSpan::new(0, 2).validate(raw).is_err());
    assert_eq!(
        SourceSpan::new(0, 7).validate(raw).unwrap_err().code(),
        "SOURCE_SPAN_OUT_OF_BOUNDS"
    );
    assert!(SourceSpan::new(3, 2).validate(raw).is_err());
    assert!(SourceSpan::new(3, 3).validate(raw).is_ok());
}

#[test]
fn canonical_json_and_hash_are_stable_for_map_insertion_order() {
    let first = json!({"z": 1, "a": {"y": true, "b": 2}});
    let second = json!({"a": {"b": 2, "y": true}, "z": 1});
    assert_eq!(
        canonical_json(&first).unwrap(),
        canonical_json(&second).unwrap()
    );
    assert_eq!(
        canonical_sha256(&first).unwrap(),
        canonical_sha256(&second).unwrap()
    );
}

#[test]
fn structure_validation_checks_parent_ids_ordinals_and_spans() {
    let mut structure = draft_with_one_shot();
    assert!(validate_structure(&structure, source_bytes(), 1).is_ok());
    structure.episodes[0].scenes[0].shots[0].parent_scene_draft_id = DraftNodeId::new();
    let error = validate_structure(&structure, source_bytes(), 1).unwrap_err();
    assert_eq!(error.code(), "INVALID_PARENT_DRAFT_NODE");
}

#[test]
fn structure_validation_freezes_duplicate_and_ordinal_codes() {
    let mut duplicate = draft_with_one_shot();
    let duplicate_id = duplicate.episodes[0].draft_node_id.clone();
    duplicate.episodes[0].scenes[0].draft_node_id = duplicate_id;
    assert_eq!(
        validate_structure(&duplicate, source_bytes(), 1)
            .unwrap_err()
            .code(),
        "DRAFT_NODE_ID_DUPLICATE"
    );

    let mut invalid_ordinal = draft_with_one_shot();
    invalid_ordinal.episodes[0].ordinal = 1;
    assert_eq!(
        validate_structure(&invalid_ordinal, source_bytes(), 1)
            .unwrap_err()
            .code(),
        "DRAFT_ORDINAL_INVALID"
    );
}

#[test]
fn structure_validation_returns_capacity_error_before_other_node_checks() {
    let mut structure = draft_with_one_shot();
    let template = structure.episodes[0].clone();
    structure.episodes = (0..=MAX_EPISODES)
        .map(|ordinal| {
            let mut episode = template.clone();
            episode.ordinal = ordinal as u32;
            episode
        })
        .collect();
    assert_eq!(
        validate_structure(&structure, source_bytes(), 1)
            .unwrap_err()
            .code(),
        DRAFT_CAPACITY_EXCEEDED
    );
}

#[test]
fn capacity_and_schema_errors_are_stable_and_do_not_contain_payload() {
    let mut structure = draft_with_one_shot();
    structure.schema_version = 99;
    let error = validate_structure(&structure, source_bytes(), 1).unwrap_err();
    assert_eq!(error.code(), "DRAFT_SCHEMA_VERSION_UNSUPPORTED");
    assert!(!error.to_string().contains("第一集"));

    let payload = json!({"schemaVersion": 99, "contractVersion": 1, "secret": "source_text"});
    let error = validate_payload_root(&payload, 1).unwrap_err();
    assert_eq!(error.code(), "DRAFT_SCHEMA_VERSION_UNSUPPORTED");
    assert!(!error.to_string().contains("source_text"));
}

#[test]
fn entity_mentions_have_explicit_identity_text_and_confidence_contract() {
    let mut structure = draft_with_one_shot();
    structure.episodes[0].scenes[0].shots[0]
        .character_mentions
        .push(EntityMention {
            id: "mention-1".to_owned(),
            entity_type: EntityType::Character,
            text: "阿明".to_owned(),
            normalized_text: "阿明".to_owned(),
            source_spans: vec![SourceSpan::new(0, 3)],
            confidence: Some(0.8),
            candidate_profile_ids: Vec::new(),
            evidence: Vec::new(),
            selected_profile_id: None,
            confirmed: false,
        });
    assert!(validate_structure(&structure, source_bytes(), 1).is_ok());
    structure.episodes[0].scenes[0].shots[0].character_mentions[0].confidence = Some(1.5);
    assert_eq!(
        validate_structure(&structure, source_bytes(), 1)
            .unwrap_err()
            .code(),
        "INVALID_ENTITY_MENTION_CONFIDENCE"
    );
}

#[test]
fn payload_root_must_match_database_schema_and_contract() {
    let payload = json!({"schemaVersion": 1, "contractVersion": 1});
    assert_eq!(
        validate_payload_root(&payload, 2).unwrap_err().code(),
        "DRAFT_DB_SCHEMA_VERSION_MISMATCH"
    );
    let payload = json!({"schemaVersion": 1, "contractVersion": 2});
    assert_eq!(
        validate_payload_root(&payload, 1).unwrap_err().code(),
        "DRAFT_CONTRACT_VERSION_UNSUPPORTED"
    );
}

#[test]
fn debug_and_domain_errors_do_not_include_source_text_or_full_payload() {
    let diagnostic = Diagnostic::new(
        DiagnosticSeverity::Error,
        "TEST",
        "secret source text that must not be logged",
    );
    assert!(!format!("{diagnostic:?}").contains("secret source text"));
    let error = DraftValidationError::new("TEST", "payload", "invalid payload");
    assert!(!error.to_string().contains("source text"));
}
