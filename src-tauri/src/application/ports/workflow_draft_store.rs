/// Immutable draft source storage, separate from normalized graphs and runtime packages.
/// Synchronous to match the existing short-lived onboarding registry critical section.
pub trait WorkflowDraftStore: Send + Sync {
    fn save_raw(&self, draft_id: &str, bytes: &[u8]) -> Result<(), String>;
    fn read_raw(&self, draft_id: &str) -> Result<Vec<u8>, String>;
}
