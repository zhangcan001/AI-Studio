use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCursor {
    pub created_at: DateTime<Utc>,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<PageCursor>,
}

impl PageCursor {
    pub fn for_item(created_at: DateTime<Utc>, id: impl Into<String>) -> Self {
        Self {
            created_at,
            id: id.into(),
        }
    }
}
