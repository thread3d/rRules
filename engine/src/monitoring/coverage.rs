use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub timestamp: String,
    pub rule_id: String,
    pub evaluated: bool,
    pub fired: bool,
    pub skipped_reason: Option<String>,
}

impl CoverageReport {
    pub fn new(rule_id: impl Into<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        CoverageReport {
            timestamp,
            rule_id: rule_id.into(),
            evaluated: false,
            fired: false,
            skipped_reason: None,
        }
    }

    pub fn with_evaluated(mut self, evaluated: bool) -> Self {
        self.evaluated = evaluated;
        self
    }

    pub fn with_fired(mut self, fired: bool) -> Self {
        self.fired = fired;
        self
    }

    pub fn with_skipped_reason(mut self, reason: impl Into<String>) -> Self {
        self.skipped_reason = Some(reason.into());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize coverage report")
    }
}

impl Default for CoverageReport {
    fn default() -> Self {
        Self::new("")
    }
}
