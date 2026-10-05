use super::*;

#[tokio::test]
async fn m3_later_page_media_prompt_filters_and_scope() {
    let f = Fixture::new().await;
    library_scale_baseline::seed(&f, 96).await;
    let mut q = query(LibraryCategory::Images);
    q.limit = Some(30);
    let first = f.facade().list("prj_default", q.clone()).await.unwrap();
    assert!(!first
        .items
        .iter()
        .any(|x| x.resource_ref.id() == "ast_scale_000006"));
    sqlx::query("DELETE FROM asset_favorites WHERE asset_id!='ast_scale_000006'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM asset_tag_links WHERE asset_id!='ast_scale_000006'")
        .execute(&f.pool)
        .await
        .unwrap();
    for shape in ["keyword", "favorite", "tag", "combined"] {
        let mut search = q.clone();
        if matches!(shape, "keyword" | "combined") {
            search.keyword = Some("  Resource 000006  ".into());
        }
        if matches!(shape, "favorite" | "combined") {
            search.favorite_only = Some(true);
        }
        if matches!(shape, "tag" | "combined") {
            search.tag_id = Some(" tag_scale ".into());
        }
        let page = f.facade().list("prj_default", search).await.unwrap();
        assert_eq!(page.items.len(), 1, "{shape}");
        assert_eq!(page.items[0].resource_ref.id(), "ast_scale_000006");
    }
    let mut prompt = query(LibraryCategory::Prompts);
    prompt.limit = Some(30);
    assert!(!f
        .facade()
        .list("prj_default", prompt.clone())
        .await
        .unwrap()
        .items
        .iter()
        .any(|x| x.resource_ref.id() == "prm_scale_000002"));
    prompt.keyword = Some("Prompt 000002".into());
    assert_eq!(
        f.facade().list("prj_default", prompt).await.unwrap().items[0]
            .resource_ref
            .id(),
        "prm_scale_000002"
    );
    let foreign = "prj_11111111-1111-4111-8111-111111111111";
    f.asset("ast_foreign", foreign, "image").await;
    q.tag_id = Some("tag_scale".into());
    assert!(f.facade().list(foreign, q).await.unwrap().items.is_empty());
    let page = f
        .facade()
        .list("prj_default", query(LibraryCategory::All))
        .await
        .unwrap();
    assert_eq!(page.coverage, LibraryCoverage::RecentSummary);
    assert!(page.next_cursor.is_none());
    assert!(page.items.len() <= 48);
    let mut invalid = query(LibraryCategory::All);
    invalid.favorite_only = Some(true);
    assert_eq!(
        f.facade()
            .list("prj_default", invalid)
            .await
            .unwrap_err()
            .code,
        "LIBRARY_QUERY_INVALID"
    );
}

#[tokio::test]
async fn m3_cursor_binds_every_normalized_filter_and_rejects_mismatch() {
    let f = Fixture::new().await;
    library_scale_baseline::seed(&f, 96).await;
    let mut q = query(LibraryCategory::Media);
    q.limit = Some(30);
    q.keyword = Some(" Resource ".into());
    q.favorite_only = Some(false);
    q.tag_id = Some(" ".into());
    let cursor = f
        .facade()
        .list("prj_default", q.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    assert_eq!(cursor.keyword.as_deref(), Some("Resource"));
    assert!(!cursor.favorite_only);
    assert!(cursor.tag_id.is_none());
    for field in ["project", "category", "keyword", "favorite", "tag"] {
        let mut changed = q.clone();
        changed.cursor = Some(cursor.clone());
        let mut project = "prj_default";
        match field {
            "project" => project = "prj_11111111-1111-4111-8111-111111111111",
            "category" => changed.category = LibraryCategory::Images,
            "keyword" => changed.keyword = Some("different".into()),
            "favorite" => changed.favorite_only = Some(true),
            "tag" => changed.tag_id = Some("tag_scale".into()),
            _ => unreachable!(),
        }
        assert_eq!(
            f.facade().list(project, changed).await.unwrap_err().code,
            "LIBRARY_QUERY_INVALID",
            "{field}"
        );
    }
    q.cursor = Some(cursor);
    q.keyword = Some("Resource".into());
    q.favorite_only = None;
    q.tag_id = None;
    assert_eq!(
        f.facade().list("prj_default", q).await.unwrap().items.len(),
        30
    );
    let mut tagged = query(LibraryCategory::Media);
    tagged.limit = Some(10);
    tagged.favorite_only = Some(true);
    tagged.tag_id = Some("tag_scale".into());
    let cursor = f
        .facade()
        .list("prj_default", tagged.clone())
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    assert!(cursor.favorite_only);
    assert_eq!(cursor.tag_id.as_deref(), Some("tag_scale"));
    tagged.cursor = Some(cursor);
    tagged.tag_id = Some("tag_other".into());
    assert_eq!(
        f.facade()
            .list("prj_default", tagged)
            .await
            .unwrap_err()
            .code,
        "LIBRARY_QUERY_INVALID"
    );
}
