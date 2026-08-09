use crate::application::ports::{TaskUpdatePayload, TaskUpdateSink, TASK_UPDATED_EVENT};
use crate::domain::Task;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct TauriTaskUpdateSink {
    app_handle: AppHandle,
}

impl TauriTaskUpdateSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl TaskUpdateSink for TauriTaskUpdateSink {
    fn publish(&self, task: &Task) {
        let payload = TaskUpdatePayload::from_task(task);
        if let Err(error) = self.app_handle.emit(TASK_UPDATED_EVENT, payload) {
            tracing::warn!(task_id = %task.id, error_type = std::any::type_name_of_val(&error), "failed to publish task update event");
        }
    }
}
