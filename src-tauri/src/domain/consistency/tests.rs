use super::binding::{BindingRole, InheritanceMode, ShotProfileBinding, ShotReferenceSetBinding};
use super::ids::{generate_consistency_id, validate_consistency_id, ConsistencyIdKind};
use super::profile::{
    CharacterProfile, ConsistencyProfileRecord, CostumeVariant, ProfileRevision,
    ProfileRevisionStatus, ProfileType, PropProfile, SceneProfile, StyleProfile,
};
use super::reference_set::{ReferenceSet, ReferenceSetItem, ReferenceSetPurpose};
use super::validation::{
    validate_metadata_json, validate_optional_text, validate_profile_binding,
    validate_profile_name, validate_prompt_fragment, validate_reference_set,
    validate_reference_set_binding, validate_reference_set_items, MAX_DESCRIPTION_CHARS,
    MAX_METADATA_BYTES, MAX_PROFILE_NAME_CHARS, MAX_PROMPT_FRAGMENT_CHARS,
    MAX_REFERENCE_ROLE_CHARS,
};
use crate::application::ports::{
    ConsistencyProfileRepository, ReferenceSetRepository, ShotConsistencyRepository,
};
use chrono::{DateTime, TimeZone, Utc};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const PROJECT_ID: &str = "prj_550e8400-e29b-41d4-a716-446655440000";
const CHARACTER_ID: &str = "cp_550e8400-e29b-41d4-a716-446655440000";
const SCENE_ID: &str = "scp_550e8400-e29b-41d4-a716-446655440000";
const PROP_ID: &str = "pp_550e8400-e29b-41d4-a716-446655440000";
const STYLE_ID: &str = "stp_550e8400-e29b-41d4-a716-446655440000";
const COSTUME_ID: &str = "cv_550e8400-e29b-41d4-a716-446655440000";
const REFERENCE_SET_ID: &str = "rs_550e8400-e29b-41d4-a716-446655440000";
const REVISION_ID: &str = "prv_550e8400-e29b-41d4-a716-446655440000";
const PROFILE_BINDING_ID: &str = "spb_550e8400-e29b-41d4-a716-446655440000";
const REFERENCE_BINDING_ID: &str = "srb_550e8400-e29b-41d4-a716-446655440000";
const SHOT_ID: &str = "shot_550e8400-e29b-41d4-a716-446655440000";

fn timestamp() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 0)
        .single()
        .expect("test timestamp should be valid")
}

fn assert_round_trip<T>(value: &T)
where
    T: Debug + DeserializeOwned + Eq + Serialize,
{
    let encoded = serde_json::to_string(value).expect("domain value should serialize");
    let decoded: T = serde_json::from_str(&encoded).expect("domain value should deserialize");
    assert_eq!(&decoded, value);
}

fn reference_set_item(asset_id: &str, ordinal: i64, is_primary: bool) -> ReferenceSetItem {
    ReferenceSetItem {
        reference_set_id: REFERENCE_SET_ID.to_owned(),
        asset_id: asset_id.to_owned(),
        ordinal,
        role: Some("FULL_BODY".to_owned()),
        is_primary,
        created_at: timestamp(),
    }
}

fn reference_set() -> ReferenceSet {
    ReferenceSet {
        id: REFERENCE_SET_ID.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "Character references".to_owned(),
        purpose: ReferenceSetPurpose::Character,
        description: "A stable ordered set".to_owned(),
        owner_profile_type: Some(ProfileType::Character),
        owner_profile_id: Some(CHARACTER_ID.to_owned()),
        active_revision_id: Some(REVISION_ID.to_owned()),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn character_profile() -> CharacterProfile {
    CharacterProfile {
        id: CHARACTER_ID.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "Mira".to_owned(),
        description: "The lead character".to_owned(),
        canonical_prompt: "young heroine".to_owned(),
        negative_prompt: "blurry".to_owned(),
        default_style_profile_id: Some(STYLE_ID.to_owned()),
        default_reference_set_id: Some(REFERENCE_SET_ID.to_owned()),
        active_revision_id: Some(REVISION_ID.to_owned()),
        metadata_json: "{\"species\":\"human\"}".to_owned(),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn scene_profile() -> SceneProfile {
    SceneProfile {
        id: SCENE_ID.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "Rooftop".to_owned(),
        description: "A city rooftop".to_owned(),
        environment_prompt: "rooftop at dusk".to_owned(),
        lighting_prompt: Some("soft rim light".to_owned()),
        negative_prompt: Some("crowd".to_owned()),
        default_style_profile_id: Some(STYLE_ID.to_owned()),
        default_reference_set_id: Some(REFERENCE_SET_ID.to_owned()),
        active_revision_id: Some(REVISION_ID.to_owned()),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn prop_profile() -> PropProfile {
    PropProfile {
        id: PROP_ID.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "Lantern".to_owned(),
        description: "A brass lantern".to_owned(),
        canonical_prompt: "antique brass lantern".to_owned(),
        material_prompt: Some("brushed brass".to_owned()),
        scale_prompt: Some("hand-sized".to_owned()),
        default_reference_set_id: Some(REFERENCE_SET_ID.to_owned()),
        active_revision_id: Some(REVISION_ID.to_owned()),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn style_profile() -> StyleProfile {
    StyleProfile {
        id: STYLE_ID.to_owned(),
        project_id: PROJECT_ID.to_owned(),
        name: "Ink anime".to_owned(),
        style_prompt: "clean ink anime illustration".to_owned(),
        color_prompt: Some("violet and warm gold".to_owned()),
        line_prompt: Some("precise linework".to_owned()),
        negative_prompt: Some("photorealistic".to_owned()),
        output_notes: Some("Keep the face readable".to_owned()),
        active_revision_id: Some(REVISION_ID.to_owned()),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn costume_variant() -> CostumeVariant {
    CostumeVariant {
        id: COSTUME_ID.to_owned(),
        character_profile_id: CHARACTER_ID.to_owned(),
        name: "Travel coat".to_owned(),
        prompt_fragment: "dark travel coat".to_owned(),
        reference_set_id: Some(REFERENCE_SET_ID.to_owned()),
        is_default: true,
        ordinal: 0,
        active_revision_id: Some(REVISION_ID.to_owned()),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn profile_revision() -> ProfileRevision {
    ProfileRevision {
        id: REVISION_ID.to_owned(),
        profile_type: ProfileType::Character,
        profile_id: CHARACTER_ID.to_owned(),
        revision_number: 1,
        content_json: "{\"name\":\"Mira\"}".to_owned(),
        content_sha256: "a".repeat(64),
        status: ProfileRevisionStatus::Active,
        created_at: timestamp(),
        created_by: Some("test".to_owned()),
    }
}

fn profile_binding(
    role: BindingRole,
    profile_type: ProfileType,
    costume_variant_id: Option<&str>,
    ordinal: i64,
) -> ShotProfileBinding {
    ShotProfileBinding {
        id: PROFILE_BINDING_ID.to_owned(),
        shot_id: SHOT_ID.to_owned(),
        role,
        profile_type,
        profile_id: CHARACTER_ID.to_owned(),
        costume_variant_id: costume_variant_id.map(str::to_owned),
        ordinal,
        inheritance_mode: InheritanceMode::Explicit,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn reference_binding(ordinal: i64) -> ShotReferenceSetBinding {
    ShotReferenceSetBinding {
        id: REFERENCE_BINDING_ID.to_owned(),
        shot_id: SHOT_ID.to_owned(),
        role: BindingRole::ShotReference,
        reference_set_id: REFERENCE_SET_ID.to_owned(),
        ordinal,
        required: true,
        inheritance_mode: InheritanceMode::Inherited,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

#[test]
fn consistency_ids_have_all_frozen_prefixes_and_validate() {
    let kinds = [
        (ConsistencyIdKind::CharacterProfile, "cp_"),
        (ConsistencyIdKind::SceneProfile, "scp_"),
        (ConsistencyIdKind::PropProfile, "pp_"),
        (ConsistencyIdKind::StyleProfile, "stp_"),
        (ConsistencyIdKind::CostumeVariant, "cv_"),
        (ConsistencyIdKind::ReferenceSet, "rs_"),
        (ConsistencyIdKind::ProfileRevision, "prv_"),
        (ConsistencyIdKind::ShotProfileBinding, "spb_"),
        (ConsistencyIdKind::ShotReferenceSetBinding, "srb_"),
    ];

    for (kind, prefix) in kinds {
        let id = generate_consistency_id(kind);
        assert!(id.starts_with(prefix));
        assert_eq!(kind.prefix(), prefix);
        assert_eq!(id.len(), prefix.len() + UUID.len());
        validate_consistency_id(kind, &id).expect("generated id should validate");
    }
}

#[test]
fn consistency_id_validation_rejects_invalid_and_path_like_values() {
    let invalid = [
        "",
        " ",
        "scp_550e8400-e29b-41d4-a716-446655440000",
        "cp_not-a-uuid",
        "cp_550e8400e29b41d4a716446655440000",
        "cp_550e8400-e29b-41d4-a716-446655440000/child",
        r"cp_550e8400-e29b-41d4-a716-446655440000\child",
        "cp_550e8400-e29b-41d4-a716-446655440000:child",
        "../cp_550e8400-e29b-41d4-a716-446655440000",
    ];

    for value in invalid {
        let error = validate_consistency_id(ConsistencyIdKind::CharacterProfile, value)
            .expect_err("invalid consistency id should be rejected");
        assert!(error.to_string().contains("INVALID_CONSISTENCY_ID"));
    }
}

#[test]
fn consistency_enums_use_uppercase_json_and_database_values() {
    let profile_types = [
        (ProfileType::Character, "CHARACTER"),
        (ProfileType::Scene, "SCENE"),
        (ProfileType::Prop, "PROP"),
        (ProfileType::Style, "STYLE"),
    ];
    for (value, expected) in profile_types {
        assert_eq!(value.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(ProfileType::try_from_db(expected).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<ProfileType>(&format!("\"{expected}\"")).unwrap(),
            value
        );
        assert!(ProfileType::try_from_db(&expected.to_lowercase()).is_err());
    }

    let statuses = [
        (ProfileRevisionStatus::Active, "ACTIVE"),
        (ProfileRevisionStatus::Archived, "ARCHIVED"),
    ];
    for (value, expected) in statuses {
        assert_eq!(value.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(ProfileRevisionStatus::try_from_db(expected).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<ProfileRevisionStatus>(&format!("\"{expected}\"")).unwrap(),
            value
        );
    }

    let purposes = [
        (ReferenceSetPurpose::Character, "CHARACTER"),
        (ReferenceSetPurpose::Costume, "COSTUME"),
        (ReferenceSetPurpose::Scene, "SCENE"),
        (ReferenceSetPurpose::Prop, "PROP"),
        (ReferenceSetPurpose::Style, "STYLE"),
        (ReferenceSetPurpose::Shot, "SHOT"),
    ];
    for (value, expected) in purposes {
        assert_eq!(value.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(ReferenceSetPurpose::try_from_db(expected).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<ReferenceSetPurpose>(&format!("\"{expected}\"")).unwrap(),
            value
        );
    }

    let roles = [
        (BindingRole::Character, "CHARACTER"),
        (BindingRole::Scene, "SCENE"),
        (BindingRole::Prop, "PROP"),
        (BindingRole::Style, "STYLE"),
        (BindingRole::ShotReference, "SHOT_REFERENCE"),
    ];
    for (value, expected) in roles {
        assert_eq!(value.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(BindingRole::try_from_db(expected).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<BindingRole>(&format!("\"{expected}\"")).unwrap(),
            value
        );
    }

    let inheritance_modes = [
        (InheritanceMode::Explicit, "EXPLICIT"),
        (InheritanceMode::Inherited, "INHERITED"),
        (InheritanceMode::Replace, "REPLACE"),
        (InheritanceMode::Remove, "REMOVE"),
    ];
    for (value, expected) in inheritance_modes {
        assert_eq!(value.as_str(), expected);
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(InheritanceMode::try_from_db(expected).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<InheritanceMode>(&format!("\"{expected}\"")).unwrap(),
            value
        );
    }
}

#[test]
fn profile_name_validation_trims_only_for_validation() {
    let original = "  Mira  ".to_owned();
    let copy = original.clone();
    validate_profile_name(&original).unwrap();
    assert_eq!(original, copy, "validation must not mutate the input");
    assert!(validate_profile_name("   ").is_err());
    assert!(validate_profile_name("").is_err());
    assert!(validate_profile_name(&"界".repeat(MAX_PROFILE_NAME_CHARS)).is_ok());
    assert!(validate_profile_name(&"界".repeat(MAX_PROFILE_NAME_CHARS + 1)).is_err());
    assert!(validate_profile_name("角色🙂").is_ok());
}

#[test]
fn text_prompt_and_metadata_limits_are_explicit() {
    assert!(validate_optional_text("description", None).is_ok());
    assert!(
        validate_optional_text("description", Some(&"a".repeat(MAX_DESCRIPTION_CHARS))).is_ok()
    );
    assert!(
        validate_optional_text("description", Some(&"a".repeat(MAX_DESCRIPTION_CHARS + 1)))
            .is_err()
    );
    assert!(validate_prompt_fragment("prompt", &"a".repeat(MAX_PROMPT_FRAGMENT_CHARS)).is_ok());
    assert!(
        validate_prompt_fragment("prompt", &"a".repeat(MAX_PROMPT_FRAGMENT_CHARS + 1)).is_err()
    );

    let mut exact_metadata = String::from("{\"data\":\"");
    exact_metadata.push_str(&"a".repeat(MAX_METADATA_BYTES - exact_metadata.len() - 2));
    exact_metadata.push_str("\"}");
    assert_eq!(exact_metadata.len(), MAX_METADATA_BYTES);
    validate_metadata_json(&exact_metadata).unwrap();
    let oversized = format!("{exact_metadata}x");
    assert!(validate_metadata_json(&oversized).is_err());

    for invalid in ["[]", "1", "true", "null", "\"scalar\"", "{malformed"] {
        assert!(
            validate_metadata_json(invalid).is_err(),
            "accepted {invalid}"
        );
    }
    validate_metadata_json("{}").unwrap();
}

#[test]
fn reference_set_and_item_validation_enforces_order_uniqueness_primary_and_owner() {
    let valid_items = vec![
        reference_set_item("ast-one", 0, true),
        reference_set_item("ast-two", 1, false),
    ];
    validate_reference_set_items(&valid_items).unwrap();

    let mut invalid = valid_items.clone();
    invalid[0].asset_id.clear();
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut invalid = valid_items.clone();
    invalid[1].asset_id = invalid[0].asset_id.clone();
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut invalid = valid_items.clone();
    invalid[1].ordinal = invalid[0].ordinal;
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut invalid = valid_items.clone();
    invalid[1].ordinal = 2;
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut invalid = valid_items.clone();
    invalid[0].ordinal = -1;
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut invalid = valid_items.clone();
    invalid[1].is_primary = true;
    assert!(validate_reference_set_items(&invalid).is_err());

    let mut no_primary = valid_items.clone();
    no_primary[0].is_primary = false;
    validate_reference_set_items(&no_primary).unwrap();

    let mut empty_role = valid_items.clone();
    empty_role[0].role = Some("  ".to_owned());
    assert!(validate_reference_set_items(&empty_role).is_err());
    let mut long_role = valid_items;
    long_role[0].role = Some("a".repeat(MAX_REFERENCE_ROLE_CHARS + 1));
    assert!(validate_reference_set_items(&long_role).is_err());

    validate_reference_set(&reference_set()).unwrap();
    let mut missing_owner_id = reference_set();
    missing_owner_id.owner_profile_id = None;
    assert!(validate_reference_set(&missing_owner_id).is_err());
    let mut missing_owner_type = reference_set();
    missing_owner_type.owner_profile_type = None;
    assert!(validate_reference_set(&missing_owner_type).is_err());
    let mut empty_owner_id = reference_set();
    empty_owner_id.owner_profile_id = Some(" ".to_owned());
    assert!(validate_reference_set(&empty_owner_id).is_err());
}

#[test]
fn binding_validation_enforces_role_profile_costume_and_ordinal_contracts() {
    validate_profile_binding(&profile_binding(
        BindingRole::Character,
        ProfileType::Character,
        Some(COSTUME_ID),
        0,
    ))
    .unwrap();
    validate_profile_binding(&profile_binding(
        BindingRole::Scene,
        ProfileType::Scene,
        None,
        0,
    ))
    .unwrap();
    validate_profile_binding(&profile_binding(
        BindingRole::Prop,
        ProfileType::Prop,
        None,
        1,
    ))
    .unwrap();
    validate_profile_binding(&profile_binding(
        BindingRole::Style,
        ProfileType::Style,
        None,
        0,
    ))
    .unwrap();

    assert!(validate_profile_binding(&profile_binding(
        BindingRole::Character,
        ProfileType::Scene,
        None,
        0,
    ))
    .is_err());
    assert!(validate_profile_binding(&profile_binding(
        BindingRole::ShotReference,
        ProfileType::Character,
        None,
        0,
    ))
    .is_err());
    assert!(validate_profile_binding(&profile_binding(
        BindingRole::Scene,
        ProfileType::Scene,
        Some(COSTUME_ID),
        0,
    ))
    .is_err());
    assert!(validate_profile_binding(&profile_binding(
        BindingRole::Character,
        ProfileType::Character,
        None,
        -1,
    ))
    .is_err());

    validate_reference_set_binding(&reference_binding(0)).unwrap();
    assert!(validate_reference_set_binding(&reference_binding(-1)).is_err());
}

#[test]
fn all_required_domain_structs_round_trip_through_json() {
    let character = character_profile();
    let scene = scene_profile();
    let prop = prop_profile();
    let style = style_profile();
    let costume = costume_variant();
    let revision = profile_revision();
    let set = reference_set();
    let item = reference_set_item("ast-one", 0, true);
    let profile_binding = profile_binding(
        BindingRole::Character,
        ProfileType::Character,
        Some(COSTUME_ID),
        0,
    );
    let reference_binding = reference_binding(0);

    for record in [
        ConsistencyProfileRecord::Character(character.clone()),
        ConsistencyProfileRecord::Scene(scene.clone()),
        ConsistencyProfileRecord::Prop(prop.clone()),
        ConsistencyProfileRecord::Style(style.clone()),
    ] {
        assert_round_trip(&record);
    }
    assert_round_trip(&character);
    assert_round_trip(&scene);
    assert_round_trip(&prop);
    assert_round_trip(&style);
    assert_round_trip(&costume);
    assert_round_trip(&revision);
    assert_round_trip(&set);
    assert_round_trip(&item);
    assert_round_trip(&profile_binding);
    assert_round_trip(&reference_binding);
}

#[test]
fn profile_record_accessors_expose_common_identity_contract_and_costume_is_child_only() {
    let records = [
        ConsistencyProfileRecord::Character(character_profile()),
        ConsistencyProfileRecord::Scene(scene_profile()),
        ConsistencyProfileRecord::Prop(prop_profile()),
        ConsistencyProfileRecord::Style(style_profile()),
    ];
    let expected = [
        (ProfileType::Character, CHARACTER_ID, "Mira"),
        (ProfileType::Scene, SCENE_ID, "Rooftop"),
        (ProfileType::Prop, PROP_ID, "Lantern"),
        (ProfileType::Style, STYLE_ID, "Ink anime"),
    ];

    for (record, (profile_type, id, name)) in records.iter().zip(expected) {
        assert_eq!(record.profile_type(), profile_type);
        assert_eq!(record.id(), id);
        assert_eq!(record.project_id(), PROJECT_ID);
        assert_eq!(record.name(), name);
        assert_eq!(record.created_at(), timestamp());
        assert_eq!(record.updated_at(), timestamp());
    }

    let costume = costume_variant();
    assert_eq!(costume.character_profile_id, CHARACTER_ID);
    assert!(serde_json::to_string(&costume)
        .unwrap()
        .contains("character_profile_id"));
}

fn assert_send_sync<T: Send + Sync + ?Sized>() {}

#[test]
fn repository_ports_are_send_sync_and_object_safe() {
    assert_send_sync::<dyn ConsistencyProfileRepository>();
    assert_send_sync::<dyn ReferenceSetRepository>();
    assert_send_sync::<dyn ShotConsistencyRepository>();

    let _: Option<&dyn ConsistencyProfileRepository> = None;
    let _: Option<&dyn ReferenceSetRepository> = None;
    let _: Option<&dyn ShotConsistencyRepository> = None;
}
