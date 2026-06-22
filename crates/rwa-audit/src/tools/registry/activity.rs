use std::collections::HashMap;

use crate::tools::types::{parse_usd_field, ToolResult};
use crate::tools::TOOL_ACTIVITY_SURFACE;

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityDailyRow {
    pub symbol: String,
    pub date: String,
    pub volume_usd: f64,
    pub unique_senders: u64,
    pub include_in_chart: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetActivitySummary {
    pub symbol: String,
    pub total_volume_usd: f64,
    pub median_daily_volume_usd: f64,
    pub active_days: u32,
    pub window_days: u32,
    pub median_unique_senders: f64,
    pub volume_per_sender_usd: Option<f64>,
}

/// Article 1 Fig. 2: map volume vs active-user breadth per asset.
pub fn activity_surface(rows: &[ActivityDailyRow]) -> ToolResult {
    let mut by_symbol: HashMap<String, Vec<&ActivityDailyRow>> = HashMap::new();
    for row in rows.iter().filter(|r| r.include_in_chart) {
        by_symbol.entry(row.symbol.clone()).or_default().push(row);
    }

    let mut result = ToolResult::new(TOOL_ACTIVITY_SURFACE);
    if by_symbol.is_empty() {
        return result.gap("no chart-eligible activity rows");
    }

    for (symbol, asset_rows) in by_symbol {
        let summary = summarize_asset(&symbol, &asset_rows);
        result = result
            .metric(
                format!("{}_total_volume_usd", summary.symbol),
                summary.total_volume_usd,
                "USD",
            )
            .metric(
                format!("{}_median_daily_volume_usd", summary.symbol),
                summary.median_daily_volume_usd,
                "USD",
            )
            .metric(
                format!("{}_active_days", summary.symbol),
                summary.active_days as f64,
                "days",
            )
            .metric(
                format!("{}_median_unique_senders", summary.symbol),
                summary.median_unique_senders,
                "addresses",
            );
        if let Some(vps) = summary.volume_per_sender_usd {
            result = result.metric(
                format!("{}_volume_per_sender_usd", summary.symbol),
                vps,
                "USD",
            );
        }
        result = result.label(format!(
            "{}: ${:.0} total volume, {:.0} median daily senders",
            summary.symbol, summary.total_volume_usd, summary.median_unique_senders
        ));
    }

    result
}

pub(crate) fn summarize_asset(symbol: &str, rows: &[&ActivityDailyRow]) -> AssetActivitySummary {
    let window_days = rows.len() as u32;
    let volumes: Vec<f64> = rows.iter().map(|r| r.volume_usd).collect();
    let senders: Vec<f64> = rows
        .iter()
        .filter(|r| r.unique_senders > 0)
        .map(|r| r.unique_senders as f64)
        .collect();
    let total_volume_usd: f64 = volumes.iter().sum();
    let active_days = rows.iter().filter(|r| r.volume_usd > 0.0).count() as u32;
    let median_daily_volume_usd = median(&volumes);
    let median_unique_senders = if senders.is_empty() {
        0.0
    } else {
        median(&senders)
    };
    let volume_per_sender_usd = if median_unique_senders > 0.0 {
        Some(median_daily_volume_usd / median_unique_senders)
    } else {
        None
    };

    AssetActivitySummary {
        symbol: symbol.to_string(),
        total_volume_usd,
        median_daily_volume_usd,
        active_days,
        window_days,
        median_unique_senders,
        volume_per_sender_usd,
    }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[sorted.len() / 2]
}

/// Build activity rows from the published CSV schema.
pub fn activity_row_from_csv(
    symbol: &str,
    date: &str,
    volume_usd: &str,
    unique_senders: &str,
    include_in_figure: &str,
) -> Option<ActivityDailyRow> {
    Some(ActivityDailyRow {
        symbol: symbol.to_string(),
        date: date.to_string(),
        volume_usd: parse_usd_field(volume_usd)?,
        unique_senders: unique_senders.parse().ok()?,
        include_in_chart: include_in_figure.eq_ignore_ascii_case("yes"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<ActivityDailyRow> {
        vec![
            ActivityDailyRow {
                symbol: "BUIDL".into(),
                date: "2026-05-01".into(),
                volume_usd: 100.0,
                unique_senders: 2,
                include_in_chart: true,
            },
            ActivityDailyRow {
                symbol: "BUIDL".into(),
                date: "2026-05-02".into(),
                volume_usd: 0.0,
                unique_senders: 0,
                include_in_chart: true,
            },
            ActivityDailyRow {
                symbol: "PAXG".into(),
                date: "2026-05-01".into(),
                volume_usd: 1_000_000.0,
                unique_senders: 50,
                include_in_chart: true,
            },
        ]
    }

    #[test]
    fn activity_surface_summarizes_per_symbol() {
        let result = activity_surface(&sample_rows());
        assert!(result
            .metrics
            .iter()
            .any(|m| m.name == "PAXG_total_volume_usd" && m.value == 1_000_000.0));
        assert!(result.labels.iter().any(|l| l.contains("PAXG")));
    }

    #[test]
    fn summarize_asset_counts_active_days() {
        let sample = sample_rows();
        let rows: Vec<&ActivityDailyRow> = sample.iter().filter(|r| r.symbol == "BUIDL").collect();
        let s = summarize_asset("BUIDL", &rows);
        assert_eq!(s.active_days, 1);
        assert_eq!(s.window_days, 2);
    }
}
