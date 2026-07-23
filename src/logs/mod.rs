use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
    Level,
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsEvent {
    pub app_name: String,
    pub app_version: String,
    pub target: String,
    pub level: String,
    pub module_path: String,
    pub location: String,
    pub thread_name: String,
    pub thread_id: String,
    pub dynamic_fields: HashMap<String, Value>,
    pub date: i64,
}

struct FieldVisitor {
    fields: HashMap<String, Value>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{:?}", value)),
        );
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields.insert(field.name().to_string(), Value::Number(serde_json::Number::from_f64(value).unwrap_or(0.into())));
    }
}

pub struct JsonLogLayer {
    app_name: String,
    app_version: String,
}

impl JsonLogLayer {
    pub fn new(app_name: String, app_version: String) -> Self {
        Self {
            app_name,
            app_version,
        }
    }
}

impl<S> Layer<S> for JsonLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor {
            fields: HashMap::new(),
        };
        event.record(&mut visitor);

        let metadata = event.metadata();
        let level = match *metadata.level() {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        };

        let module_path = metadata.module_path().unwrap_or("").to_string();
        let target = metadata.target().to_string();
        
        let location = if let Some(file) = metadata.file() {
            let line = metadata.line().map(|l| l.to_string()).unwrap_or_default();
            format!("{}:{}", file, line)
        } else {
            String::new()
        };

        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unknown").to_string();
        let thread_id = format!("{:?}", thread.id());

        let log_event = LogsEvent {
            app_name: self.app_name.clone(),
            app_version: self.app_version.clone(),
            target,
            level: level.to_string(),
            module_path,
            location,
            thread_name,
            thread_id,
            dynamic_fields: visitor.fields,
            date: Utc::now().timestamp(),
        };

        if let Ok(json) = serde_json::to_string(&log_event) {
            println!("{}", json);
        }
    }
}
