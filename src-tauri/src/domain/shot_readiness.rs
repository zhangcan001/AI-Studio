use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShotReadinessStatus {
    Ready,
    Incomplete,
    Blocked,
}

impl ShotReadinessStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Incomplete => "INCOMPLETE",
            Self::Blocked => "BLOCKED",
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, String> {
        match value {
            "READY" => Ok(Self::Ready),
            "INCOMPLETE" => Ok(Self::Incomplete),
            "BLOCKED" => Ok(Self::Blocked),
            other => Err(format!("unknown shot readiness status: {other}")),
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, String> {
        Self::try_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessCheckState {
    Pass,
    Warning,
    Incomplete,
    Blocker,
}

impl ReadinessCheckState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warning => "WARNING",
            Self::Incomplete => "INCOMPLETE",
            Self::Blocker => "BLOCKER",
        }
    }

    pub const fn severity(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Warning => 1,
            Self::Incomplete => 2,
            Self::Blocker => 3,
        }
    }

    pub fn try_from_str(value: &str) -> Result<Self, String> {
        match value {
            "PASS" => Ok(Self::Pass),
            "WARNING" => Ok(Self::Warning),
            "INCOMPLETE" => Ok(Self::Incomplete),
            "BLOCKER" => Ok(Self::Blocker),
            other => Err(format!("unknown readiness check state: {other}")),
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, String> {
        Self::try_from_str(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessGateKey {
    Character,
    Scene,
    Reference,
    Prompt,
    Workflow,
    Output,
    ComfyCapability,
}

impl ReadinessGateKey {
    pub const ALL: [Self; 7] = [
        Self::Character,
        Self::Scene,
        Self::Reference,
        Self::Prompt,
        Self::Workflow,
        Self::Output,
        Self::ComfyCapability,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Character => "CHARACTER",
            Self::Scene => "SCENE",
            Self::Reference => "REFERENCE",
            Self::Prompt => "PROMPT",
            Self::Workflow => "WORKFLOW",
            Self::Output => "OUTPUT",
            Self::ComfyCapability => "COMFY_CAPABILITY",
        }
    }

    pub const fn all() -> &'static [Self; 7] {
        &Self::ALL
    }

    pub fn try_from_str(value: &str) -> Result<Self, String> {
        match value {
            "CHARACTER" => Ok(Self::Character),
            "SCENE" => Ok(Self::Scene),
            "REFERENCE" => Ok(Self::Reference),
            "PROMPT" => Ok(Self::Prompt),
            "WORKFLOW" => Ok(Self::Workflow),
            "OUTPUT" => Ok(Self::Output),
            "COMFY_CAPABILITY" => Ok(Self::ComfyCapability),
            other => Err(format!("unknown readiness gate key: {other}")),
        }
    }

    pub fn try_from_db(value: &str) -> Result<Self, String> {
        Self::try_from_str(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessCheck {
    pub key: ReadinessGateKey,
    pub state: ReadinessCheckState,
    pub code: String,
    pub message: String,
    pub source: String,
    pub entity_ids: Vec<String>,
    pub fix_action: Option<String>,
}

impl ReadinessCheck {
    pub fn new(
        key: ReadinessGateKey,
        state: ReadinessCheckState,
        code: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            key,
            state,
            code: code.into(),
            message: message.into(),
            source: source.into(),
            entity_ids: Vec::new(),
            fix_action: None,
        }
    }

    pub fn with_entity(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_ids.push(entity_id.into());
        self
    }

    pub fn with_fix_action(mut self, action: impl Into<String>) -> Self {
        self.fix_action = Some(action.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessGateResult {
    pub key: ReadinessGateKey,
    pub state: ReadinessCheckState,
    pub checks: Vec<ReadinessCheck>,
}

impl ReadinessGateResult {
    pub fn new(key: ReadinessGateKey, checks: Vec<ReadinessCheck>) -> Self {
        let state = checks
            .iter()
            .map(|check| check.state)
            .max_by_key(|state| state.severity())
            .unwrap_or(ReadinessCheckState::Pass);
        Self { key, state, checks }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotReadiness {
    pub project_id: String,
    pub shot_id: String,
    pub stage: String,
    pub status: ShotReadinessStatus,
    pub score: i32,
    pub gates: Vec<ReadinessGateResult>,
    pub context_hash: String,
    pub evaluated_at: DateTime<Utc>,
    pub comfy_checked_at: Option<DateTime<Utc>>,
    pub cached: bool,
}

impl ShotReadiness {
    pub fn from_gates(
        project_id: impl Into<String>,
        shot_id: impl Into<String>,
        stage: impl Into<String>,
        context_hash: impl Into<String>,
        gates: Vec<ReadinessGateResult>,
        evaluated_at: DateTime<Utc>,
        comfy_checked_at: Option<DateTime<Utc>>,
        cached: bool,
        context_partial: bool,
    ) -> Self {
        let mut score = 100;
        let mut status = ShotReadinessStatus::Ready;
        for gate in &gates {
            match gate.state {
                ReadinessCheckState::Blocker => {
                    score -= 35;
                    status = ShotReadinessStatus::Blocked;
                }
                ReadinessCheckState::Incomplete => {
                    score -= 15;
                    if status != ShotReadinessStatus::Blocked {
                        status = ShotReadinessStatus::Incomplete;
                    }
                }
                ReadinessCheckState::Warning => score -= 5,
                ReadinessCheckState::Pass => {}
            }
        }
        if context_partial {
            status = ShotReadinessStatus::Blocked;
        }
        Self {
            project_id: project_id.into(),
            shot_id: shot_id.into(),
            stage: stage.into(),
            status,
            score: score.clamp(0, 100),
            gates,
            context_hash: context_hash.into(),
            evaluated_at,
            comfy_checked_at,
            cached,
        }
    }

    pub fn gate(&self, key: ReadinessGateKey) -> Option<&ReadinessGateResult> {
        self.gates.iter().find(|gate| gate.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(key: ReadinessGateKey, state: ReadinessCheckState) -> ReadinessGateResult {
        ReadinessGateResult::new(
            key,
            vec![ReadinessCheck::new(key, state, "TEST", "test", "test")],
        )
    }

    #[test]
    fn score_is_calculated_once_per_gate_and_clamped() {
        let gates = vec![
            gate(ReadinessGateKey::Character, ReadinessCheckState::Warning),
            gate(ReadinessGateKey::Scene, ReadinessCheckState::Incomplete),
            gate(ReadinessGateKey::Reference, ReadinessCheckState::Blocker),
            gate(ReadinessGateKey::Prompt, ReadinessCheckState::Blocker),
            gate(ReadinessGateKey::Workflow, ReadinessCheckState::Blocker),
        ];
        let result = ShotReadiness::from_gates(
            "p",
            "s",
            "image",
            "hash",
            gates,
            Utc::now(),
            None,
            true,
            false,
        );
        assert_eq!(result.score, 0);
        assert_eq!(result.status, ShotReadinessStatus::Blocked);
    }

    #[test]
    fn partial_context_cannot_be_ready() {
        let result = ShotReadiness::from_gates(
            "p",
            "s",
            "image",
            "hash",
            vec![gate(ReadinessGateKey::Character, ReadinessCheckState::Pass)],
            Utc::now(),
            None,
            true,
            true,
        );
        assert_eq!(result.status, ShotReadinessStatus::Blocked);
    }

    #[test]
    fn warning_is_ready_but_incomplete_and_blocker_are_not() {
        let ready = ShotReadiness::from_gates(
            "p",
            "s",
            "image",
            "hash",
            vec![gate(
                ReadinessGateKey::Character,
                ReadinessCheckState::Warning,
            )],
            Utc::now(),
            None,
            true,
            false,
        );
        assert_eq!(ready.status, ShotReadinessStatus::Ready);
        assert_eq!(ready.score, 95);

        let incomplete = ShotReadiness::from_gates(
            "p",
            "s",
            "image",
            "hash",
            vec![gate(
                ReadinessGateKey::Character,
                ReadinessCheckState::Incomplete,
            )],
            Utc::now(),
            None,
            true,
            false,
        );
        assert_eq!(incomplete.status, ShotReadinessStatus::Incomplete);

        let blocked = ShotReadiness::from_gates(
            "p",
            "s",
            "image",
            "hash",
            vec![gate(
                ReadinessGateKey::Character,
                ReadinessCheckState::Blocker,
            )],
            Utc::now(),
            None,
            true,
            false,
        );
        assert_eq!(blocked.status, ShotReadinessStatus::Blocked);
    }
}
