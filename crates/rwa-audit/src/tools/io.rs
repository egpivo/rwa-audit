//! Load published CSV evidence into tool inputs.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::exchange::PanelMetricRow;
use super::registry::{activity_row_from_csv, ActivityDailyRow};
use super::types::parse_usd_field;

pub fn load_activity_daily_csv(path: &Path) -> Result<Vec<ActivityDailyRow>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read activity csv: {}", path.display()))?;
    parse_activity_daily_csv(&text)
}

pub fn parse_activity_daily_csv(text: &str) -> Result<Vec<ActivityDailyRow>> {
    let mut rows = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if idx == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 12 {
            continue;
        }
        // date, product_or_platform, …, volume_usd (5), …, active_user_count (8), …, include_in_figure (11)
        if let Some(row) = activity_row_from_csv(cols[1], cols[0], cols[5], cols[8], cols[11]) {
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn load_depth_panel_csv(path: &Path) -> Result<Vec<PanelMetricRow>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read panel csv: {}", path.display()))?;
    parse_depth_panel_csv(&text)
}

pub fn parse_depth_panel_csv(text: &str) -> Result<Vec<PanelMetricRow>> {
    let mut rows = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if idx == 0 || line.trim().is_empty() {
            continue;
        }
        let cols = split_csv_line(line);
        if cols.len() < 11 {
            continue;
        }
        let value = parse_usd_field(&cols[4]).unwrap_or(0.0);
        rows.push(PanelMetricRow {
            asset_or_example: cols[1].to_string(),
            venue_or_surface: cols[2].to_string(),
            metric_type: cols[3].to_string(),
            metric_value_usd: value,
            caveat: cols[10].to_string(),
        });
    }
    Ok(rows)
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    out.push(current);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_publish_activity_schema() {
        let csv = "date,product_or_platform,workflow_type,chain_or_venue,volume_metric_type,volume_usd,volume_tokens,active_user_metric_type,active_user_count,observation_domain,source,include_in_figure\n\
                   2026-05-01,BUIDL,Token-level,Ethereum,Daily volume,100.5,100.5,Daily unique senders,3,domain,source,yes\n";
        let rows = parse_activity_daily_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "BUIDL");
        assert_eq!(rows[0].unique_senders, 3);
    }
}
