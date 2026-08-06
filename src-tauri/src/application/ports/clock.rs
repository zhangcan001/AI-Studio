use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct MonotonicEventClock {
    source: Arc<dyn Clock>,
    last: Mutex<Option<DateTime<Utc>>>,
}

impl MonotonicEventClock {
    pub fn new(source: Arc<dyn Clock>) -> Self {
        Self {
            source,
            last: Mutex::new(None),
        }
    }
}

impl Clock for MonotonicEventClock {
    fn now(&self) -> DateTime<Utc> {
        let candidate = self.source.now();
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = match *last {
            Some(previous) if candidate <= previous => previous + Duration::microseconds(1),
            _ => candidate,
        };
        *last = Some(next);
        next
    }
}
