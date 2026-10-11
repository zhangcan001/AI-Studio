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
    for field in [
        "project", "category", "keyword", "favorite", "tag", "source",
    ] {
        let mut changed = q.clone();
        changed.cursor = Some(cursor.clone());
        let mut project = "prj_default";
        match field {
            "project" => project = "prj_11111111-1111-4111-8111-111111111111",
            "category" => changed.category = LibraryCategory::Images,
            "keyword" => changed.keyword = Some("different".into()),
            "favorite" => changed.favorite_only = Some(true),
            "tag" => changed.tag_id = Some("tag_scale".into()),
            "source" => changed.source_kind = Some(LibrarySourceKind::Source),
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

#[tokio::test]
async fn phase6a_source_kind_filters_across_pages_without_leak_or_dup() {
    let f = Fixture::new().await;
    library_scale_baseline::seed(&f, 96).await;
    let mut q = query(LibraryCategory::Media);
    q.limit = Some(30);
    q.source_kind = Some(LibrarySourceKind::Source);
    let mut seen = HashSet::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        q.cursor = cursor.clone();
        let page = f.facade().list("prj_default", q.clone()).await.unwrap();
        pages += 1;
        assert_eq!(page.coverage, LibraryCoverage::KeysetPage);
        for item in &page.items {
            assert!(
                seen.insert(item.resource_ref.id().to_owned()),
                "duplicate {}",
                item.resource_ref.id()
            );
        }
        // Every returned media id must be an odd seed (= source_* after baseline flip of evens).
        for item in &page.items {
            let n: i64 = item
                .resource_ref
                .id()
                .trim_start_matches("ast_scale_")
                .parse()
                .unwrap();
            assert_ne!(n % 2, 0, "generated leak {}", item.resource_ref.id());
        }
        match page.next_cursor {
            Some(next) => {
                assert_eq!(next.source_kind, LibrarySourceKind::Source);
                cursor = Some(next);
            }
            None => break,
        }
        assert!(pages <= 5, "too many pages");
    }
    assert!(pages >= 2, "expected multiple source pages, got {pages}");
    assert!(!seen.is_empty());

    let mut generated = query(LibraryCategory::Images);
    generated.limit = Some(30);
    generated.source_kind = Some(LibrarySourceKind::Generated);
    let page = f.facade().list("prj_default", generated).await.unwrap();
    assert!(!page.items.is_empty());
    for item in &page.items {
        let n: i64 = item
            .resource_ref
            .id()
            .trim_start_matches("ast_scale_")
            .parse()
            .unwrap();
        assert_eq!(
            n % 2,
            0,
            "source leak into generated images {}",
            item.resource_ref.id()
        );
    }

    // Five classifications: explicit rows cover source image/video/audio,
    // generated video, and historical generated image (server-side, not UI-only).
    for (id, media, category) in [
        ("ast_cls_source_image", "image", "source_image"),
        ("ast_cls_source_video", "video", "source_video"),
        ("ast_cls_source_audio", "audio", "source_audio"),
        ("ast_cls_generated_video", "video", "generated_video"),
        ("ast_cls_generated_image", "image", "generated_image"),
    ] {
        f.asset(id, "prj_default", media).await;
        sqlx::query("UPDATE assets SET category=?, name=? WHERE id=?")
            .bind(category)
            .bind(format!("Class {category}"))
            .bind(id)
            .execute(&f.pool)
            .await
            .unwrap();
        if category.starts_with("generated_") {
            sqlx::query("UPDATE assets SET source_task_id='tsk_scale_source' WHERE id=?")
                .bind(id)
                .execute(&f.pool)
                .await
                .unwrap();
        }
    }
    for (category, source, expect_id) in [
        (
            LibraryCategory::Images,
            LibrarySourceKind::Source,
            "ast_cls_source_image",
        ),
        (
            LibraryCategory::Images,
            LibrarySourceKind::Generated,
            "ast_cls_generated_image",
        ),
        (
            LibraryCategory::Videos,
            LibrarySourceKind::Source,
            "ast_cls_source_video",
        ),
        (
            LibraryCategory::Videos,
            LibrarySourceKind::Generated,
            "ast_cls_generated_video",
        ),
        (
            LibraryCategory::Audio,
            LibrarySourceKind::Source,
            "ast_cls_source_audio",
        ),
    ] {
        let mut shaped = query(category);
        shaped.limit = Some(100);
        shaped.source_kind = Some(source);
        shaped.keyword = Some("Class ".into());
        let page = f.facade().list("prj_default", shaped).await.unwrap();
        assert!(
            page.items.iter().any(|i| i.resource_ref.id() == expect_id),
            "{category:?}/{source:?} missing {expect_id}: {:?}",
            page.items
                .iter()
                .map(|i| i.resource_ref.id())
                .collect::<Vec<_>>()
        );
        assert!(
            !page
                .items
                .iter()
                .any(|i| i.resource_ref.id().starts_with("ast_cls_")
                    && i.resource_ref.id() != expect_id),
            "{category:?}/{source:?} leaked other class ids"
        );
    }

    let mut invalid = query(LibraryCategory::Prompts);
    invalid.source_kind = Some(LibrarySourceKind::Source);
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
async fn phase6a_source_plus_favorite_tag_keyword_stable_keyset() {
    let f = Fixture::new().await;
    library_scale_baseline::seed(&f, 96).await;
    sqlx::query("DELETE FROM asset_favorites WHERE CAST(substr(asset_id,11) AS INTEGER)%2=0")
        .execute(&f.pool)
        .await
        .unwrap();
    let mut q = query(LibraryCategory::Media);
    q.limit = Some(10);
    q.source_kind = Some(LibrarySourceKind::Source);
    q.favorite_only = Some(true);
    q.tag_id = Some("tag_other".into());
    q.keyword = Some("Resource".into());
    let first = f.facade().list("prj_default", q.clone()).await.unwrap();
    assert!(!first.items.is_empty());
    assert_eq!(
        first.next_cursor.as_ref().map(|c| c.source_kind),
        Some(LibrarySourceKind::Source)
    );
    for item in &first.items {
        let n: i64 = item
            .resource_ref
            .id()
            .trim_start_matches("ast_scale_")
            .parse()
            .unwrap();
        assert_ne!(n % 2, 0);
        assert_eq!(n % 3, 0);
    }
    if let Some(cursor) = first.next_cursor.clone() {
        q.cursor = Some(cursor);
        let second = f.facade().list("prj_default", q).await.unwrap();
        let overlap: HashSet<_> = first.items.iter().map(|i| i.resource_ref.id()).collect();
        for item in &second.items {
            assert!(!overlap.contains(item.resource_ref.id()));
        }
    }
}
