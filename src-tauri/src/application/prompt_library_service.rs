use crate::application::ports::{
    Clock, PromptEntryRecord, PromptLibraryRepository, PromptVersionRecord, RepositoryError,
};
use serde::Serialize;
use std::{collections::HashSet, error::Error, fmt, sync::Arc};
use uuid::Uuid;

const MAX_NAME_CHARS: usize = 120;
const MAX_TAGS: usize = 20;
const MAX_TAG_CHARS: usize = 32;
const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersionView {
    pub id: String,
    pub prompt_id: String,
    pub version: i64,
    pub text: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptEntryView {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub name: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version_count: i64,
    pub versions: Vec<PromptVersionView>,
}

pub struct PromptLibraryService {
    repository: Arc<dyn PromptLibraryRepository>,
    clock: Arc<dyn Clock>,
}

impl PromptLibraryService {
    pub fn new(repository: Arc<dyn PromptLibraryRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }

    pub async fn list(
        &self,
        project_id: &str,
        kind: Option<&str>,
        keyword: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<PromptEntryView>, PromptLibraryError> {
        validate_project_id(project_id)?;
        let kind = kind.map(validate_kind).transpose()?;
        let keyword = keyword.map(normalize_keyword).transpose()?;
        let tag = tag.map(normalize_tag_query).transpose()?;
        let records = self
            .repository
            .list(
                project_id,
                kind.as_deref(),
                keyword.as_deref(),
                tag.as_deref(),
            )
            .await?;
        records
            .into_iter()
            .filter_map(|record| match record_to_view(record, Vec::new()) {
                Ok(view) => {
                    let keyword_matches = keyword
                        .as_deref()
                        .map(|value| {
                            view.name.to_lowercase().contains(value)
                                || view
                                    .tags
                                    .iter()
                                    .any(|item| item.to_lowercase().contains(value))
                        })
                        .unwrap_or(true);
                    let tag_matches = tag
                        .as_deref()
                        .map(|value| view.tags.iter().any(|item| item.to_lowercase() == value))
                        .unwrap_or(true);
                    (keyword_matches && tag_matches).then_some(Ok(view))
                }
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    pub async fn get(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<PromptEntryView, PromptLibraryError> {
        validate_project_id(project_id)?;
        validate_id(prompt_id, "PROMPT")?;
        let record = self
            .repository
            .find_by_id(project_id, prompt_id)
            .await?
            .ok_or_else(|| PromptLibraryError::NotFound(prompt_id.to_owned()))?;
        let versions = self.repository.list_versions(project_id, prompt_id).await?;
        record_to_view(record, versions)
    }

    pub async fn create(
        &self,
        project_id: &str,
        kind: &str,
        name: &str,
        tags: &[String],
        text: &str,
    ) -> Result<PromptEntryView, PromptLibraryError> {
        validate_project_id(project_id)?;
        let kind = validate_kind(kind)?;
        let (name, normalized_name) =
            canonical_prompt_name(name).map_err(PromptLibraryError::InvalidInput)?;
        let tags = canonical_prompt_tags(tags).map_err(PromptLibraryError::InvalidInput)?;
        let text = canonical_prompt_text(text).map_err(PromptLibraryError::InvalidInput)?;
        let now = self.clock.now().to_rfc3339();
        let entry = PromptEntryRecord {
            id: format!("prm_{}", Uuid::new_v4()),
            project_id: project_id.to_owned(),
            kind: kind.to_owned(),
            name,
            normalized_name,
            tags_json: serde_json::to_string(&tags)
                .map_err(|error| PromptLibraryError::InvalidInput(error.to_string()))?,
            created_at: now.clone(),
            updated_at: now.clone(),
            version_count: 1,
        };
        let first_version = PromptVersionRecord {
            id: format!("prv_{}", Uuid::new_v4()),
            prompt_id: entry.id.clone(),
            version: 1,
            text,
            created_at: now,
        };
        self.repository.create(&entry, &first_version).await?;
        record_to_view(entry, vec![first_version])
    }

    pub async fn add_version(
        &self,
        project_id: &str,
        prompt_id: &str,
        text: &str,
    ) -> Result<PromptVersionView, PromptLibraryError> {
        validate_project_id(project_id)?;
        validate_id(prompt_id, "PROMPT")?;
        let text = canonical_prompt_text(text).map_err(PromptLibraryError::InvalidInput)?;
        let version = self
            .repository
            .append_version(
                project_id,
                prompt_id,
                &format!("prv_{}", Uuid::new_v4()),
                &text,
                &self.clock.now().to_rfc3339(),
            )
            .await?;
        Ok(version_to_view(version))
    }

    pub async fn update_metadata(
        &self,
        project_id: &str,
        prompt_id: &str,
        name: &str,
        tags: &[String],
    ) -> Result<PromptEntryView, PromptLibraryError> {
        validate_project_id(project_id)?;
        validate_id(prompt_id, "PROMPT")?;
        let (name, normalized_name) =
            canonical_prompt_name(name).map_err(PromptLibraryError::InvalidInput)?;
        let tags = canonical_prompt_tags(tags).map_err(PromptLibraryError::InvalidInput)?;
        let tags_json = serde_json::to_string(&tags)
            .map_err(|error| PromptLibraryError::InvalidInput(error.to_string()))?;
        let record = self
            .repository
            .update_metadata(
                project_id,
                prompt_id,
                &name,
                &normalized_name,
                &tags_json,
                &self.clock.now().to_rfc3339(),
            )
            .await?
            .ok_or_else(|| PromptLibraryError::NotFound(prompt_id.to_owned()))?;
        let versions = self.repository.list_versions(project_id, prompt_id).await?;
        record_to_view(record, versions)
    }

    pub async fn delete(
        &self,
        project_id: &str,
        prompt_id: &str,
    ) -> Result<(), PromptLibraryError> {
        validate_project_id(project_id)?;
        validate_id(prompt_id, "PROMPT")?;
        if !self.repository.delete(project_id, prompt_id).await? {
            return Err(PromptLibraryError::NotFound(prompt_id.to_owned()));
        }
        Ok(())
    }
}

fn record_to_view(
    record: PromptEntryRecord,
    versions: Vec<PromptVersionRecord>,
) -> Result<PromptEntryView, PromptLibraryError> {
    let tags: Vec<String> = serde_json::from_str(&record.tags_json).map_err(|error| {
        PromptLibraryError::Repository(RepositoryError::serialization(
            "prompt tags",
            error.to_string(),
        ))
    })?;
    Ok(PromptEntryView {
        id: record.id,
        project_id: record.project_id,
        kind: record.kind,
        name: record.name,
        tags,
        created_at: record.created_at,
        updated_at: record.updated_at,
        version_count: record.version_count,
        versions: versions.into_iter().map(version_to_view).collect(),
    })
}

fn version_to_view(version: PromptVersionRecord) -> PromptVersionView {
    PromptVersionView {
        id: version.id,
        prompt_id: version.prompt_id,
        version: version.version,
        text: version.text,
        created_at: version.created_at,
    }
}

pub(crate) fn canonical_prompt_name(value: &str) -> Result<(String, String), String> {
    let name = value.trim();
    if name.is_empty() || name.contains(['\r', '\n']) {
        return Err("提示词名称必须是 1–120 个字符的单行文本。".to_owned());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err("提示词名称最多 120 个字符。".to_owned());
    }
    Ok((name.to_owned(), name.to_lowercase()))
}

pub(crate) fn canonical_prompt_tags(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > MAX_TAGS {
        return Err("标签最多 20 个。".to_owned());
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let tag = value.trim();
        if tag.is_empty() || tag.contains(['\r', '\n']) || tag.chars().count() > MAX_TAG_CHARS {
            return Err("每个标签必须是 1–32 个字符的单行文本。".to_owned());
        }
        let normalized = tag.to_lowercase();
        if seen.insert(normalized) {
            result.push(tag.to_owned());
        }
    }
    Ok(result)
}

pub(crate) fn canonical_prompt_text(value: &str) -> Result<String, String> {
    let text = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_owned();
    if text.as_bytes().len() > MAX_TEXT_BYTES {
        return Err("提示词正文最多 64 KiB（UTF-8）。".to_owned());
    }
    Ok(text)
}

fn normalize_keyword(value: &str) -> Result<String, PromptLibraryError> {
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        return Err(PromptLibraryError::InvalidInput(
            "搜索词不能为空。".to_owned(),
        ));
    }
    Ok(value)
}

fn normalize_tag_query(value: &str) -> Result<String, PromptLibraryError> {
    let tag = canonical_prompt_tags(&[value.to_owned()])
        .map_err(PromptLibraryError::InvalidInput)
        .and_then(|tags| {
            tags.into_iter()
                .next()
                .ok_or_else(|| PromptLibraryError::InvalidInput("标签不能为空。".to_owned()))
        })?;
    Ok(tag.to_lowercase())
}

fn validate_kind(value: &str) -> Result<&str, PromptLibraryError> {
    match value {
        "prompt" | "snippet" => Ok(value),
        _ => Err(PromptLibraryError::InvalidInput(
            "提示词类型只能是 prompt 或 snippet。".to_owned(),
        )),
    }
}

fn validate_project_id(value: &str) -> Result<(), PromptLibraryError> {
    if value.trim().is_empty() {
        return Err(PromptLibraryError::InvalidInput(
            "项目 ID 不能为空。".to_owned(),
        ));
    }
    Ok(())
}

fn validate_id(value: &str, kind: &str) -> Result<(), PromptLibraryError> {
    if value.trim().is_empty() {
        return Err(PromptLibraryError::InvalidInput(format!(
            "{kind} ID 不能为空。"
        )));
    }
    Ok(())
}

#[derive(Debug)]
pub enum PromptLibraryError {
    InvalidInput(String),
    NotFound(String),
    Repository(RepositoryError),
}

impl fmt::Display for PromptLibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::NotFound(message) => formatter.write_str(message),
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PromptLibraryError {}

impl From<RepositoryError> for PromptLibraryError {
    fn from(value: RepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_prompt_name, canonical_prompt_tags, canonical_prompt_text};

    #[test]
    fn normalizes_names_tags_and_line_endings() {
        assert_eq!(
            canonical_prompt_name("  中文 Prompt  ").unwrap(),
            ("中文 Prompt".to_owned(), "中文 prompt".to_owned())
        );
        assert_eq!(
            canonical_prompt_tags(&[" 人物 ".to_owned(), "人物".to_owned(), "Style".to_owned()])
                .unwrap(),
            vec!["人物".to_owned(), "Style".to_owned()]
        );
        assert_eq!(
            canonical_prompt_text(" a\r\nb\r ").unwrap(),
            "a\nb".to_owned()
        );
    }

    #[test]
    fn rejects_invalid_limits() {
        assert!(canonical_prompt_name(&"x".repeat(121)).is_err());
        assert!(canonical_prompt_tags(&["x".repeat(33)]).is_err());
        assert!(canonical_prompt_text(&"x".repeat(64 * 1024 + 1)).is_err());
    }

    #[tokio::test]
    async fn persists_chinese_versions_tags_search_and_project_isolation() {
        use super::PromptLibraryService;
        use crate::application::ports::PromptLibraryRepository;
        use crate::infrastructure::database::{initialize, SqlitePromptLibraryRepository};
        use crate::infrastructure::time::SystemClock;
        use std::sync::Arc;
        use tempfile::tempdir;

        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("prompts.db"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('prompt-project-1', '一', 'C:/one', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'), ('prompt-project-2', '二', 'C:/two', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        let repository: Arc<dyn PromptLibraryRepository> =
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
        let service = PromptLibraryService::new(repository, Arc::new(SystemClock));
        let first = service
            .create(
                "prompt-project-1",
                "prompt",
                "  中文起点 ",
                &[" 人物 ".to_owned(), "人物".to_owned(), "Kera2".to_owned()],
                "人物\r\n柔光",
            )
            .await
            .unwrap();
        assert_eq!(first.name, "中文起点");
        assert_eq!(first.tags, vec!["人物", "Kera2"]);
        let second = service
            .add_version("prompt-project-1", &first.id, "人物\n硬光")
            .await
            .unwrap();
        assert_eq!(second.version, 2);
        let found = service.get("prompt-project-1", &first.id).await.unwrap();
        assert_eq!(found.versions.len(), 2);
        assert_eq!(
            service
                .list(
                    "prompt-project-1",
                    Some("prompt"),
                    Some("中文"),
                    Some("人物")
                )
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(service.get("prompt-project-2", &first.id).await.is_err());
        service
            .update_metadata(
                "prompt-project-1",
                &first.id,
                "重命名",
                &["新标签".to_owned()],
            )
            .await
            .unwrap();
        service.delete("prompt-project-1", &first.id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM prompt_versions WHERE prompt_id = ?"
            )
            .bind(&first.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn stores_snippets_without_touching_generation_tables() {
        use crate::application::ports::PromptLibraryRepository;
        use crate::infrastructure::database::{initialize, SqlitePromptLibraryRepository};
        use crate::infrastructure::time::SystemClock;
        use std::sync::Arc;
        use tempfile::tempdir;

        let directory = tempdir().unwrap();
        let pool = initialize(&directory.path().join("snippets.db"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES ('snippet-project', '片段', 'C:/snippet', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        let repository: Arc<dyn PromptLibraryRepository> =
            Arc::new(SqlitePromptLibraryRepository::new(pool.clone()));
        let service = super::PromptLibraryService::new(repository, Arc::new(SystemClock));
        let snippet = service
            .create(
                "snippet-project",
                "snippet",
                "电影光线",
                &[],
                "cinematic light",
            )
            .await
            .unwrap();
        assert_eq!(snippet.kind, "snippet");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
}
