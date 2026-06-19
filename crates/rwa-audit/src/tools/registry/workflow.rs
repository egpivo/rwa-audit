use crate::tools::registry::activity::{summarize_asset, ActivityDailyRow};
use crate::tools::registry::sender_coverage::SenderVolume;
use crate::tools::types::ToolResult;
use crate::tools::TOOL_WORKFLOW_SIGNATURE;

/// Article 1 Fig. 4: continuity vs sender concentration (openness proxy).
pub fn workflow_signature(
    rows: &[ActivityDailyRow],
    sender_volumes: Option<&[SenderVolume]>,
    top_rank: usize,
) -> ToolResult {
    let mut result = ToolResult::new(TOOL_WORKFLOW_SIGNATURE);
    let mut by_symbol: std::collections::HashMap<String, Vec<&ActivityDailyRow>> =
        std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r.include_in_chart) {
        by_symbol.entry(row.symbol.clone()).or_default().push(row);
    }

    if by_symbol.is_empty() {
        return result.gap("no chart-eligible activity rows");
    }

    for (symbol, asset_rows) in by_symbol {
        let summary = summarize_asset(&symbol, &asset_rows);
        let continuity = if summary.window_days == 0 {
            0.0
        } else {
            summary.active_days as f64 / summary.window_days as f64
        };

        let top_concentration = sender_volumes.and_then(|all| {
            let subset: Vec<SenderVolume> =
                all.iter().filter(|s| s.symbol == symbol).cloned().collect();
            if subset.is_empty() {
                None
            } else {
                top_n_share(&subset, top_rank)
            }
        });

        result = result.metric(
            format!("{}_calendar_continuity", symbol),
            continuity,
            "ratio",
        );
        if let Some(c) = top_concentration {
            result = result.metric(
                format!("{}_top{}_sender_concentration", symbol, top_rank),
                c,
                "ratio",
            );
        } else {
            result = result.gap(format!(
                "{symbol}: sender-level volume breakdown not available; continuity only"
            ));
        }

        let label = if let Some(c) = top_concentration {
            format!(
                "{symbol}: continuity {:.2}, top-{top_rank} concentration {:.2}",
                continuity, c
            )
        } else {
            format!("{symbol}: continuity {:.2}", continuity)
        };
        result = result.label(label);
    }

    result
}

fn top_n_share(senders: &[SenderVolume], n: usize) -> Option<f64> {
    if senders.is_empty() {
        return None;
    }
    let total: f64 = senders.iter().map(|s| s.volume_usd).sum();
    if total <= 0.0 {
        return None;
    }
    let mut ranked = senders.to_vec();
    ranked.sort_by(|a, b| {
        b.volume_usd
            .partial_cmp(&a.volume_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_sum: f64 = ranked.iter().take(n).map(|s| s.volume_usd).sum();
    Some(top_sum / total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::sender_coverage::SenderVolume;

    #[test]
    fn workflow_signature_reports_continuity() {
        let rows = vec![
            ActivityDailyRow {
                symbol: "USDY".into(),
                date: "2026-05-01".into(),
                volume_usd: 10.0,
                unique_senders: 3,
                include_in_chart: true,
            },
            ActivityDailyRow {
                symbol: "USDY".into(),
                date: "2026-05-02".into(),
                volume_usd: 5.0,
                unique_senders: 2,
                include_in_chart: true,
            },
            ActivityDailyRow {
                symbol: "USDY".into(),
                date: "2026-05-03".into(),
                volume_usd: 0.0,
                unique_senders: 0,
                include_in_chart: true,
            },
        ];
        let senders = vec![
            SenderVolume {
                symbol: "USDY".into(),
                address: "0xa".into(),
                volume_usd: 12.0,
            },
            SenderVolume {
                symbol: "USDY".into(),
                address: "0xb".into(),
                volume_usd: 3.0,
            },
        ];
        let result = workflow_signature(&rows, Some(&senders), 5);
        let continuity = result
            .metrics
            .iter()
            .find(|m| m.name == "USDY_calendar_continuity")
            .unwrap()
            .value;
        assert!((continuity - 2.0 / 3.0).abs() < 1e-6);
    }
}
