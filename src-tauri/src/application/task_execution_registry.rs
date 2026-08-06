use crate::domain::TaskId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::watch;

#[derive(Clone, Default)]
pub struct TaskExecutionRegistry {
    entries: Arc<Mutex<HashMap<TaskId, watch::Sender<bool>>>>,
}

impl TaskExecutionRegistry {
    pub fn register(&self, task_id: TaskId) -> (watch::Receiver<bool>, TaskExecutionGuard) {
        let (sender, receiver) = watch::channel(false);
        lock_entries(&self.entries).insert(task_id.clone(), sender);
        (
            receiver,
            TaskExecutionGuard {
                registry: self.clone(),
                task_id,
            },
        )
    }

    pub fn signal_cancel(&self, task_id: &TaskId) -> bool {
        lock_entries(&self.entries)
            .get(task_id)
            .map(|sender| sender.send(true).is_ok())
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, task_id: &TaskId) -> bool {
        lock_entries(&self.entries).contains_key(task_id)
    }

    fn remove(&self, task_id: &TaskId) {
        lock_entries(&self.entries).remove(task_id);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        lock_entries(&self.entries).len()
    }
}

pub struct TaskExecutionGuard {
    registry: TaskExecutionRegistry,
    task_id: TaskId,
}

impl Drop for TaskExecutionGuard {
    fn drop(&mut self) {
        self.registry.remove(&self.task_id);
    }
}

fn lock_entries<'a>(
    entries: &'a Mutex<HashMap<TaskId, watch::Sender<bool>>>,
) -> MutexGuard<'a, HashMap<TaskId, watch::Sender<bool>>> {
    entries
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::TaskExecutionRegistry;
    use crate::domain::TaskId;

    #[tokio::test]
    async fn registration_is_available_before_worker_and_is_removed_by_guard() {
        let registry = TaskExecutionRegistry::default();
        let task_id = TaskId::new();
        let (mut receiver, guard) = registry.register(task_id.clone());

        assert!(registry.contains(&task_id));
        assert_eq!(registry.len(), 1);
        assert!(registry.signal_cancel(&task_id));
        receiver
            .changed()
            .await
            .expect("cancel signal should arrive");
        assert!(*receiver.borrow());

        drop(guard);
        assert!(!registry.contains(&task_id));
        assert_eq!(registry.len(), 0);
        assert!(!registry.signal_cancel(&task_id));
    }
}
