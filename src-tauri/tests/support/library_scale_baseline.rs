//! Opt-in observed benchmark, not a timing-sensitive CI correctness test.
use super::*;
use ai_studio_lib::application::ports::{
    AssetCategoryFilter, AssetCreatedOrder, AssetLibraryQuery, AssetMediaTypeFilter,
    AssetSourceFilter,
};
use serde_json::{json, Value};
use std::time::Instant;

pub(super) async fn seed(f: &Fixture, n: i64) {
    // Owned migrated DB only; no actual large media files are needed for list reads.
    sqlx::query("WITH RECURSIVE s(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM s WHERE x<?) INSERT INTO assets (id,project_id,type,category,name,original_name,storage_path,sha256,mime_type,width,height,file_size,metadata_json,created_at,updated_at) SELECT printf('ast_scale_%06d',x),'prj_default',CASE x%10 WHEN 0 THEN 'audio' WHEN 1 THEN 'video' ELSE 'image' END,'source_'||CASE x%10 WHEN 0 THEN 'audio' WHEN 1 THEN 'video' ELSE 'image' END,printf('Resource %06d ',x)||CASE WHEN x%7=0 THEN 'long-name-description' ELSE 'short' END,printf('file%06d',x),'owned-fixture','sha','image/png',512,512,1,'{}','2026-10-01T00:00:00+00:00','2026-10-01T00:00:00+00:00' FROM s").bind(n).execute(&f.pool).await.unwrap();
    f.task("tsk_scale_source", "prj_default", "SUCCEEDED").await;
    sqlx::query("UPDATE assets SET category='generated_'||type,source_task_id='tsk_scale_source' WHERE CAST(substr(id,11) AS INTEGER)%2=0").execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO asset_tags VALUES ('tag_scale','prj_default','Tag A','tag a','2026-10-01T00:00:00+00:00','2026-10-01T00:00:00+00:00'),('tag_other','prj_default','Tag B','tag b','2026-10-01T00:00:00+00:00','2026-10-01T00:00:00+00:00'),('tag_third','prj_default','Tag C','tag c','2026-10-01T00:00:00+00:00','2026-10-01T00:00:00+00:00')").execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO asset_favorites SELECT id,project_id,created_at FROM assets WHERE CAST(substr(id,11) AS INTEGER)%3=0").execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO asset_tag_links SELECT id,CASE WHEN CAST(substr(id,11) AS INTEGER)%2=0 THEN 'tag_scale' ELSE 'tag_other' END,project_id,created_at FROM assets").execute(&f.pool).await.unwrap();
    sqlx::query("WITH RECURSIVE s(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM s WHERE x<?) INSERT INTO prompt_entries (id,project_id,kind,name,normalized_name,tags_json,created_at,updated_at) SELECT printf('prm_scale_%06d',x),'prj_default','prompt',printf('Prompt %06d',x),printf('prompt %06d',x),'[\"baseline\"]','2026-10-01T00:00:00+00:00','2026-10-01T00:00:00+00:00' FROM s").bind(n).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO prompt_versions (id,prompt_id,version,text,created_at) SELECT 'prv_'||id,id,1,'fixture prompt text',created_at FROM prompt_entries").execute(&f.pool).await.unwrap();
}

pub(super) fn media_query(shape: &str) -> AssetLibraryQuery {
    AssetLibraryQuery {
        project_id: "prj_default".into(),
        category: AssetCategoryFilter::All,
        keyword: matches!(shape, "keyword" | "combined").then(|| "0000".into()),
        media_type: if shape == "combined" {
            AssetMediaTypeFilter::Image
        } else {
            AssetMediaTypeFilter::All
        },
        source_kind: AssetSourceFilter::All,
        favorite_only: matches!(shape, "favorite" | "combined"),
        tag_id: matches!(shape, "tag" | "combined").then(|| "tag_scale".into()),
        created_order: AssetCreatedOrder::Newest,
        cursor: None,
        limit: 30,
    }
}
fn stats(mut times: Vec<f64>, bytes: usize) -> Value {
    times.sort_by(f64::total_cmp);
    json!({"repetitions":times.len(),"p50Ms":times[times.len()/2],"p95Ms":times[(times.len()*95).div_ceil(100)-1],"approxResponseBytes":bytes})
}

#[tokio::test]
#[ignore = "observed Windows benchmark: run explicitly with --ignored --exact; not a CI timing gate"]
async fn observed_library_scale_baseline() {
    let mut datasets = vec![];
    for n in [1000, 10000] {
        let f = Fixture::new().await;
        seed(&f, n).await;
        let mut observations = serde_json::Map::new();
        for shape in ["cold", "warm", "keyword", "favorite", "tag", "combined"] {
            let mut times = vec![];
            let mut bytes = 0;
            if shape != "cold" {
                f.assets.list_page(media_query(shape)).await.unwrap();
            }
            // Cold = newly opened pool, NOT an OS disk-cache eviction.
            for _ in 0..21 {
                let pool = if shape == "cold" {
                    Some(
                        sqlx::sqlite::SqlitePoolOptions::new()
                            .max_connections(1)
                            .connect_with(
                                sqlx::sqlite::SqliteConnectOptions::new()
                                    .filename(f._dir.database_path()),
                            )
                            .await
                            .unwrap(),
                    )
                } else {
                    None
                };
                let service = pool.as_ref().map(|p| {
                    AssetLibraryService::new(
                        Arc::new(SqliteAssetBrowseRepository::new(p.clone())),
                        Arc::new(SqliteOrganizationRepository::new(p.clone())),
                    )
                });
                let start = Instant::now();
                let result = service
                    .as_ref()
                    .unwrap_or(&f.assets)
                    .list_page(media_query(shape))
                    .await
                    .unwrap();
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                bytes = serde_json::to_vec(&result).unwrap().len();
                if let Some(pool) = pool {
                    pool.close().await;
                }
            }
            observations.insert(shape.into(), stats(times, bytes));
        }
        let mut times = vec![];
        let mut bytes = 0;
        for _ in 0..21 {
            let start = Instant::now();
            let result = f
                .prompts
                .list("prj_default", None, Some("0000"), None, None, Some(30))
                .await
                .unwrap();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            bytes = serde_json::to_vec(&result).unwrap().len();
        }
        observations.insert("promptKeyword".into(), stats(times, bytes));
        let mut pagination = vec![];
        for pages in [1, 5, 20] {
            let start = Instant::now();
            let mut cursor = None;
            let mut count = 0;
            let mut bytes = 0;
            for _ in 0..pages {
                let mut q = query(LibraryCategory::Prompts);
                q.limit = Some(30);
                q.cursor = cursor;
                let page = f.facade().list("prj_default", q).await.unwrap();
                count += page.items.len();
                bytes += serde_json::to_vec(&page).unwrap().len();
                cursor = page.next_cursor;
            }
            pagination.push(json!({"pages":pages,"durationMs":start.elapsed().as_secs_f64()*1000.0,"items":count,"approxResponseBytes":bytes}));
        }
        let mut plans = vec![];
        for sql in ["EXPLAIN QUERY PLAN SELECT id FROM assets WHERE project_id='prj_default' AND (name LIKE '%0000%' OR COALESCE(original_name,'') LIKE '%0000%') AND EXISTS (SELECT 1 FROM asset_favorites f WHERE f.asset_id=assets.id AND f.project_id=assets.project_id) AND EXISTS (SELECT 1 FROM asset_tag_links l WHERE l.asset_id=assets.id AND l.project_id=assets.project_id AND l.tag_id='tag_scale') ORDER BY created_at DESC,id DESC LIMIT 31", "EXPLAIN QUERY PLAN SELECT e.id,(SELECT COUNT(*) FROM prompt_versions v WHERE v.prompt_id=e.id) FROM prompt_entries e WHERE e.project_id='prj_default' AND (e.name LIKE '%0000%' COLLATE NOCASE OR e.tags_json LIKE '%0000%' COLLATE NOCASE) ORDER BY e.updated_at DESC,e.id DESC LIMIT 31"] {use sqlx::Row;let rows=sqlx::query(sql).fetch_all(&f.pool).await.unwrap();plans.push(rows.iter().map(|r|r.get::<String,_>("detail")).collect::<Vec<_>>());}
        datasets.push(json!({"mediaCount":n,"promptCount":n,"queries":observations,"promptPagination":pagination,"queryPlans":plans}));
    }
    let output = std::env::var("AI_STUDIO_BENCHMARK_OUTPUT").expect("owned output path required");
    std::fs::write(output,serde_json::to_vec_pretty(&json!({"environment":"Windows single runner; no GPU generation","coldDefinition":"fresh pool; OS cache not evicted","warmup":"one warmup before each warm query shape; 21 measured repetitions","layer":"existing AssetLibraryService + SQLite repository (filters not yet exposed by parent Product facade)","actualNativeIPC":"NOT_AVAILABLE","datasets":datasets})).unwrap()).unwrap();
}
