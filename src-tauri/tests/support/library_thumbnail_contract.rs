use super::*;

#[tokio::test]
async fn managed_thumbnail_adapter_preserves_scope_types_missing_and_bytes_only() {
    let fixture = Fixture::new().await;
    let other = "prj_11111111-1111-4111-8111-111111111111";
    for (id, media) in [
        ("ast_thumb_image", "image"),
        ("ast_thumb_video", "video"),
        ("ast_thumb_audio", "audio"),
        ("ast_thumb_missing", "image"),
    ] {
        fixture.asset(id, "prj_default", media).await;
    }
    let root = fixture._dir.path().join("prj_default");
    let bytes = b"owned-managed-thumbnail-only";
    std::fs::write(root.join("thumbnail.png"), bytes).unwrap();
    for id in ["ast_thumb_image", "ast_thumb_video", "ast_thumb_audio"] {
        sqlx::query("UPDATE assets SET thumbnail_path='thumbnail.png' WHERE id=?")
            .bind(id)
            .execute(&fixture.pool)
            .await
            .unwrap();
    }
    let facade = fixture.facade();
    for id in ["ast_thumb_image", "ast_thumb_video"] {
        let resource = ResourceRef::Asset { id: id.into() };
        let result = facade
            .thumbnail_get("prj_default", &resource)
            .await
            .unwrap();
        assert_eq!(result, bytes); // Full media fixture bytes are different.
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("thumbnail_path"));
        assert_eq!(
            facade
                .thumbnail_get(other, &resource)
                .await
                .unwrap_err()
                .code,
            "LIBRARY_RESOURCE_NOT_FOUND"
        );
    }
    assert_eq!(
        facade
            .thumbnail_get(
                "prj_default",
                &ResourceRef::Asset {
                    id: "ast_thumb_audio".into()
                }
            )
            .await
            .unwrap_err()
            .code,
        "ASSET_TYPE_MISMATCH"
    );
    assert_eq!(
        facade
            .thumbnail_get(
                "prj_default",
                &ResourceRef::Prompt {
                    id: "prompt".into()
                }
            )
            .await
            .unwrap_err()
            .code,
        "LIBRARY_QUERY_INVALID"
    );
    assert_eq!(
        facade
            .thumbnail_get(
                "prj_default",
                &ResourceRef::Asset {
                    id: "ast_thumb_missing".into()
                }
            )
            .await
            .unwrap_err()
            .code,
        "ASSET_THUMBNAIL_UNAVAILABLE"
    );
    std::fs::remove_file(root.join("thumbnail.png")).unwrap();
    let error = facade
        .thumbnail_get(
            "prj_default",
            &ResourceRef::Asset {
                id: "ast_thumb_image".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(error.details.technical_details.is_none());
    assert!(!serde_json::to_string(&error)
        .unwrap()
        .contains(root.to_str().unwrap()));
}
