//! Shared result types for audit analysis tools.

use serde::{Deserialize, Serialize};

use crate::core::manifest::ClaimStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_id: String,
    pub metrics: Vec<ToolMetric>,
    pub labels: Vec<String>,
    pub claims: Vec<ClaimCandidate>,
    pub gaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolMetric {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimCandidate {
    pub id: String,
    pub label: String,
    pub value_display: String,
    pub value_usd: Option<f64>,
    pub status: ClaimStatus,
    pub caveat: String,
}

impl ToolResult {
    pub fn new(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            metrics: Vec::new(),
            labels: Vec::new(),
            claims: Vec::new(),
            gaps: Vec::new(),
        }
    }

    pub fn metric(mut self, name: impl Into<String>, value: f64, unit: impl Into<String>) -> Self {
        self.metrics.push(ToolMetric {
            name: name.into(),
            value,
            unit: unit.into(),
            note: None,
        });
        self
    }

    pub fn metric_note(
        mut self,
        name: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        self.metrics.push(ToolMetric {
            name: name.into(),
            value,
            unit: unit.into(),
            note: Some(note.into()),
        });
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn gap(mut self, gap: impl Into<String>) -> Self {
        self.gaps.push(gap.into());
        self
    }
}

pub fn parse_usd_field(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("n/a") {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_usd_field_handles_na() {
        assert!(parse_usd_field("N/A").is_none());
        assert_eq!(parse_usd_field("42.5").unwrap(), 42.5);
    }
}
