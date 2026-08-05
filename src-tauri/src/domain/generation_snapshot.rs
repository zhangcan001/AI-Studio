use super::task::{TaskDomainError, TaskId};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{error::Error, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SnapshotId(String);

impl SnapshotId {
    pub fn new() -> Self {
        Self(format!("snp_{}", Uuid::new_v4()))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, TaskDomainError> {
        let value = value.into();
        if value.starts_with("snp_") && value.len() > "snp_".len() {
            Ok(Self(value))
        } else {
            Err(TaskDomainError::invalid_id("snapshot", value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationSnapshot {
    pub id: SnapshotId,
    pub task_id: TaskId,
    pub workflow_json: Value,
    pub recipe_yaml: String,
    pub user_inputs_json: Value,
    pub resolved_inputs_json: Value,
    pub created_at: DateTime<Utc>,
}

impl GenerationSnapshot {
    pub fn new(
        task_id: TaskId,
        workflow_json: Value,
        recipe_yaml: impl Into<String>,
        user_inputs_json: Value,
        resolved_inputs_json: Value,
        created_at: DateTime<Utc>,
    ) -> Result<Self, SnapshotDomainError> {
        let snapshot = Self {
            id: SnapshotId::new(),
            task_id,
            workflow_json,
            recipe_yaml: recipe_yaml.into(),
            user_inputs_json,
            resolved_inputs_json,
            created_at,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn validate(&self) -> Result<(), SnapshotDomainError> {
        if self.recipe_yaml.trim().is_empty() {
            return Err(SnapshotDomainError::Invalid(
                "recipe_yaml must not be empty".to_owned(),
            ));
        }
        if !self.workflow_json.is_object() {
            return Err(SnapshotDomainError::Invalid(
                "workflow_json must be a JSON object".to_owned(),
            ));
        }
        if !self.user_inputs_json.is_object() || !self.resolved_inputs_json.is_object() {
            return Err(SnapshotDomainError::Invalid(
                "snapshot input JSON values must be objects".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotDomainError {
    Invalid(String),
}

impl fmt::Display for SnapshotDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for SnapshotDomainError {}

#[cfg(test)]
mod tests {
    use super::GenerationSnapshot;
    use crate::domain::Task;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    #[test]
    fn snapshot_captures_compiled_workflow_and_both_input_views() {
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );
        let snapshot = GenerationSnapshot::new(
            task.id,
            json!({"3": {"inputs": {}, "class_type": "Node"}}),
            "schema_version: 1",
            json!({"seed": "random"}),
            json!({"seed": 123}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(),
        )
        .expect("snapshot should be valid");

        assert!(snapshot.id.as_str().starts_with("snp_"));
        assert_eq!(snapshot.user_inputs_json["seed"], "random");
        assert_eq!(snapshot.resolved_inputs_json["seed"], 123);
    }

    #[test]
    fn snapshot_rejects_invalid_payload_shapes() {
        let task = Task::new(
            "project-1",
            "workflow-1",
            "workflow-version-1",
            "recipe-1",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        );

        assert!(GenerationSnapshot::new(
            task.id.clone(),
            json!([]),
            "recipe",
            json!({}),
            json!({}),
            task.created_at,
        )
        .is_err());
        assert!(GenerationSnapshot::new(
            task.id,
            json!({}),
            "recipe",
            json!(null),
            json!({}),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        )
        .is_err());
    }
}
