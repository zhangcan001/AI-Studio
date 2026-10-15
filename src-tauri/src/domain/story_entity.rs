use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryEntityKind {
    Character,
    Scene,
    Prop,
}

impl StoryEntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Scene => "scene",
            Self::Prop => "prop",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "character" => Ok(Self::Character),
            "scene" => Ok(Self::Scene),
            "prop" => Ok(Self::Prop),
            _ => Err(format!("unsupported story entity kind: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntity {
    pub id: String,
    pub project_id: String,
    pub kind: StoryEntityKind,
    pub name: String,
    pub description: String,
    pub cover_asset_id: Option<String>,
    pub image_asset_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityUpsert {
    pub name: String,
    pub description: String,
    pub cover_asset_id: Option<String>,
    pub image_asset_ids: Vec<String>,
}
