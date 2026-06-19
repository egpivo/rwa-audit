//! Audit tool trait and registry.

mod types;

pub mod exchange;
pub mod io;
pub mod registry;

pub use types::{ClaimCandidate, ToolMetric, ToolResult};

use anyhow::{bail, Result};

use self::exchange::{metric_equivalence_check, surface_compression};
use self::registry::{
    activity_surface, classify_surface_type, sender_volume_coverage, workflow_signature,
    ActivityDailyRow, AssetSurfaceInput, SenderVolume,
};

pub trait AuditTool {
    fn id(&self) -> &'static str;
    fn run(&self) -> Result<ToolResult>;
}

pub const TOOL_ACTIVITY_SURFACE: &str = "activity_surface";
pub const TOOL_SENDER_VOLUME_COVERAGE: &str = "sender_volume_coverage";
pub const TOOL_WORKFLOW_SIGNATURE: &str = "workflow_signature";
pub const TOOL_CLASSIFY_SURFACE: &str = "classify_surface_type";
pub const TOOL_SURFACE_COMPRESSION: &str = "surface_compression";
pub const TOOL_METRIC_EQUIVALENCE: &str = "metric_equivalence_check";

pub fn list_tool_ids() -> &'static [&'static str] {
    &[
        TOOL_ACTIVITY_SURFACE,
        TOOL_SENDER_VOLUME_COVERAGE,
        TOOL_WORKFLOW_SIGNATURE,
        TOOL_CLASSIFY_SURFACE,
        TOOL_SURFACE_COMPRESSION,
        TOOL_METRIC_EQUIVALENCE,
    ]
}

pub fn run_tool(id: &str, input: ToolInput) -> Result<ToolResult> {
    match id {
        TOOL_ACTIVITY_SURFACE => Ok(activity_surface(&input.activity_rows)),
        TOOL_SENDER_VOLUME_COVERAGE => {
            let coverage = input
                .sender_volumes
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("sender_volume_coverage requires sender_volumes"))?;
            Ok(sender_volume_coverage(coverage, input.coverage_fraction))
        }
        TOOL_WORKFLOW_SIGNATURE => Ok(workflow_signature(
            &input.activity_rows,
            input.sender_volumes.as_deref(),
            input.top_sender_rank,
        )),
        TOOL_CLASSIFY_SURFACE => {
            let assets = input
                .surface_assets
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("classify_surface_type requires surface_assets"))?;
            Ok(classify_surface_type(assets))
        }
        TOOL_SURFACE_COMPRESSION => {
            let panel = input
                .panel_rows
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("surface_compression requires panel_rows"))?;
            Ok(surface_compression(panel))
        }
        TOOL_METRIC_EQUIVALENCE => {
            let panel = input
                .panel_rows
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("metric_equivalence_check requires panel_rows"))?;
            Ok(metric_equivalence_check(
                panel,
                input.do_not_claim.as_deref(),
            ))
        }
        other => bail!("unknown tool id: {other}"),
    }
}

#[derive(Debug, Clone)]
pub struct ToolInput {
    pub activity_rows: Vec<ActivityDailyRow>,
    pub sender_volumes: Option<Vec<SenderVolume>>,
    pub surface_assets: Option<Vec<AssetSurfaceInput>>,
    pub panel_rows: Option<Vec<exchange::PanelMetricRow>>,
    pub do_not_claim: Option<Vec<String>>,
    pub coverage_fraction: f64,
    pub top_sender_rank: usize,
}

impl Default for ToolInput {
    fn default() -> Self {
        Self {
            activity_rows: Vec::new(),
            sender_volumes: None,
            surface_assets: None,
            panel_rows: None,
            do_not_claim: None,
            coverage_fraction: 0.8,
            top_sender_rank: 5,
        }
    }
}

impl ToolInput {
    pub fn with_coverage_fraction(mut self, fraction: f64) -> Self {
        self.coverage_fraction = fraction;
        self
    }

    pub fn with_top_sender_rank(mut self, rank: usize) -> Self {
        self.top_sender_rank = rank;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_article_one_and_three_tools() {
        let ids = list_tool_ids();
        assert!(ids.contains(&TOOL_WORKFLOW_SIGNATURE));
        assert!(ids.contains(&TOOL_SURFACE_COMPRESSION));
    }

    #[test]
    fn unknown_tool_errors() {
        assert!(run_tool("missing", ToolInput::default()).is_err());
    }
}
