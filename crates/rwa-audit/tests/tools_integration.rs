//! Integration tests for article analysis tools against published artifacts.

use std::path::PathBuf;

use rwa_audit::tools::{
    io::{load_activity_daily_csv, load_depth_panel_csv},
    run_tool, ToolInput, TOOL_ACTIVITY_SURFACE, TOOL_METRIC_EQUIVALENCE, TOOL_SURFACE_COMPRESSION,
    TOOL_WORKFLOW_SIGNATURE,
};

fn artifacts_data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/data")
}

#[test]
fn article1_activity_surface_on_frozen_csv() {
    let path = artifacts_data().join("rwa_activity_daily_30d.csv");
    if !path.exists() {
        return;
    }
    let rows = load_activity_daily_csv(&path).expect("load activity csv");
    assert!(!rows.is_empty());
    let result = run_tool(
        TOOL_ACTIVITY_SURFACE,
        ToolInput {
            activity_rows: rows,
            ..Default::default()
        },
    )
    .expect("activity_surface");
    assert!(!result.metrics.is_empty());
    assert!(result
        .labels
        .iter()
        .any(|l| l.contains("BUIDL") || l.contains("PAXG")));
}

#[test]
fn article1_workflow_signature_on_frozen_csv() {
    let path = artifacts_data().join("rwa_activity_daily_30d.csv");
    if !path.exists() {
        return;
    }
    let rows = load_activity_daily_csv(&path).expect("load activity csv");
    let result = run_tool(
        TOOL_WORKFLOW_SIGNATURE,
        ToolInput {
            activity_rows: rows,
            ..Default::default()
        },
    )
    .expect("workflow_signature");
    assert!(result
        .metrics
        .iter()
        .any(|m| m.name.contains("calendar_continuity")));
}

#[test]
fn article3_surface_compression_on_panel() {
    let path = artifacts_data().join("depth_vs_volume_panel_publish.csv");
    if !path.exists() {
        return;
    }
    let panel = load_depth_panel_csv(&path).expect("load panel csv");
    let result = run_tool(
        TOOL_SURFACE_COMPRESSION,
        ToolInput {
            panel_rows: Some(panel),
            ..Default::default()
        },
    )
    .expect("surface_compression");
    let ratio = result
        .metrics
        .iter()
        .find(|m| m.name == "max_to_min_ratio")
        .map(|m| m.value)
        .unwrap_or(0.0);
    assert!(
        ratio > 1_000.0,
        "expected large compression ratio, got {ratio}"
    );
}

#[test]
fn article3_metric_equivalence_on_panel() {
    let path = artifacts_data().join("depth_vs_volume_panel_publish.csv");
    if !path.exists() {
        return;
    }
    let panel = load_depth_panel_csv(&path).expect("load panel csv");
    let result = run_tool(
        TOOL_METRIC_EQUIVALENCE,
        ToolInput {
            panel_rows: Some(panel),
            do_not_claim: Some(vec![
                "Platform transfer ≠ CEX trading volume".into(),
                "Bridged value ≠ transfer volume".into(),
            ]),
            ..Default::default()
        },
    )
    .expect("metric_equivalence_check");
    assert!(!result.gaps.is_empty());
}
