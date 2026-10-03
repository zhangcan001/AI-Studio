//! Acceptance-only actual SQL events scoped to explicitly named SQLite workers.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_subscriber::{layer::Context, Layer};
#[derive(Default)]
struct Statement {
    sql: String,
    summary: String,
    elapsed: f64,
}
impl Visit for Statement {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "db.statement" => self.sql = value.into(),
            "summary" => self.summary = value.into(),
            _ => {}
        }
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "elapsed_secs" {
            self.elapsed = value;
        }
    }
    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}
#[derive(Clone, Default)]
pub struct Counter(Arc<Mutex<Option<Vec<Value>>>>);
impl<S: Subscriber> Layer<S> for Counter {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if event.metadata().target() != "sqlx::query" {
            return;
        }
        if std::thread::current().name() != Some("phase12-query-worker") {
            return;
        }
        let mut slot = self.0.lock().unwrap();
        let Some(events) = slot.as_mut() else {
            return;
        };
        let mut statement = Statement::default();
        event.record(&mut statement);
        let text = if statement.sql.is_empty() {
            statement.summary
        } else {
            statement.sql
        };
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let kind = normalized
            .split_whitespace()
            .next()
            .unwrap_or("UNKNOWN")
            .to_uppercase();
        let category = if normalized.contains("assets") || normalized.contains("task_output_assets")
        {
            "ASSET"
        } else if normalized.contains("workflow_versions") || normalized.contains("recipes") {
            "DEFINITION"
        } else if normalized.contains("production_batches")
            || normalized.contains("production_batch_items")
        {
            "QUEUE"
        } else if normalized.contains("production_runs") || normalized.contains("production_stage")
        {
            "PRODUCTION"
        } else if normalized.contains("tasks") {
            "TASK"
        } else {
            "OTHER"
        };
        // Never retain SQL text or parameter values; only statement identity/work.
        events.push(
            json!({"fingerprint":format!("{:x}",Sha256::digest(normalized.as_bytes())),
            "kind":kind,"category":category,"elapsedMs":statement.elapsed*1000.0}),
        );
    }
}
impl Counter {
    pub fn begin(&self) {
        *self.0.lock().unwrap() = Some(Vec::new());
    }
    pub fn end(&self) -> Value {
        let events = self.0.lock().unwrap().take().unwrap();
        let mut fingerprints = BTreeMap::<String, usize>::new();
        let mut categories = BTreeMap::<String, usize>::new();
        for event in &events {
            *fingerprints
                .entry(event["fingerprint"].as_str().unwrap().into())
                .or_default() += 1;
            *categories
                .entry(event["category"].as_str().unwrap().into())
                .or_default() += 1;
        }
        json!({"totalSql":events.len(),"fingerprints":fingerprints,"categories":categories,"events":events})
    }
}
