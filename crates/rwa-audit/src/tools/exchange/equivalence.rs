use crate::tools::exchange::compression::{PanelMetricRow, RecordSurface};
use crate::tools::types::ToolResult;
use crate::tools::TOOL_METRIC_EQUIVALENCE;

const DEFAULT_DO_NOT_CLAIM: &[&str] = &[
    "Platform transfer ≠ CEX trading volume",
    "Bridged value ≠ transfer volume",
    "Jupiter quote ≠ executed trade or exit capacity",
];

#[derive(Debug, Clone, PartialEq)]
struct SurfacePair {
    left: RecordSurface,
    right: RecordSurface,
    reason: String,
}

/// Article 3: flag metrics that must not be treated as interchangeable.
pub fn metric_equivalence_check(
    rows: &[PanelMetricRow],
    do_not_claim: Option<&[String]>,
) -> ToolResult {
    let mut result = ToolResult::new(TOOL_METRIC_EQUIVALENCE);
    let rules = build_rules(do_not_claim);

    let surfaces: Vec<(RecordSurface, &PanelMetricRow)> = rows
        .iter()
        .map(|r| {
            (
                RecordSurface::from_panel(&r.metric_type, &r.venue_or_surface),
                r,
            )
        })
        .collect();

    let mut violations = 0usize;
    for pair in &rules {
        let has_left = surfaces.iter().any(|(s, _)| *s == pair.left);
        let has_right = surfaces.iter().any(|(s, _)| *s == pair.right);
        if has_left && has_right {
            violations += 1;
            result = result
                .gap(format!(
                    "do not equate {} with {}: {}",
                    pair.left.as_str(),
                    pair.right.as_str(),
                    pair.reason
                ))
                .label(format!(
                    "non-equivalent surfaces present: {} vs {}",
                    pair.left.as_str(),
                    pair.right.as_str()
                ));
        }
    }

    for line in do_not_claim.unwrap_or(&[]) {
        result = result.label(format!("manifest rule: {line}"));
    }

    result.metric("forbidden_pair_hits", violations as f64, "count")
}

fn build_rules(do_not_claim: Option<&[String]>) -> Vec<SurfacePair> {
    let mut rules = vec![
        SurfacePair {
            left: RecordSurface::PlatformTransferVolume,
            right: RecordSurface::DexPoolTvl,
            reason: "platform transfer counts holder-to-holder flow; pool TVL is liquidity depth"
                .into(),
        },
        SurfacePair {
            left: RecordSurface::PlatformTransferVolume,
            right: RecordSurface::DexPoolVolume24h,
            reason: "platform transfer is not DEX 24h volume".into(),
        },
        SurfacePair {
            left: RecordSurface::BridgedTokenValue,
            right: RecordSurface::PlatformTransferVolume,
            reason: "bridged stock is a stock record, not monthly transfer flow".into(),
        },
        SurfacePair {
            left: RecordSurface::AggregatorQuote,
            right: RecordSurface::DexPoolVolume24h,
            reason: "Jupiter quote is routing simulation, not executed pool volume".into(),
        },
    ];

    if let Some(extra) = do_not_claim {
        for line in extra {
            if line.contains("Bridged value") {
                rules.push(SurfacePair {
                    left: RecordSurface::BridgedTokenValue,
                    right: RecordSurface::PlatformTransferVolume,
                    reason: line.clone(),
                });
            }
        }
    }

    for line in DEFAULT_DO_NOT_CLAIM {
        if !rules.iter().any(|r| r.reason.contains(line)) {
            rules.push(SurfacePair {
                left: RecordSurface::PlatformTransferVolume,
                right: RecordSurface::BridgedTokenValue,
                reason: (*line).into(),
            });
        }
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::exchange::compression::PanelMetricRow;

    #[test]
    fn flags_platform_transfer_vs_pool_tvl() {
        let rows = vec![
            PanelMetricRow {
                asset_or_example: "xStocks platform".into(),
                venue_or_surface: "platform_transfer_volume".into(),
                metric_type: "monthly_transfer_volume".into(),
                metric_value_usd: 1.6e9,
                caveat: String::new(),
            },
            PanelMetricRow {
                asset_or_example: "AAPLx".into(),
                venue_or_surface: "dex_amm_pool".into(),
                metric_type: "pool_tvl_total".into(),
                metric_value_usd: 124_062.35,
                caveat: String::new(),
            },
        ];
        let result = metric_equivalence_check(&rows, None);
        assert!(!result.gaps.is_empty());
        assert!(result.gaps[0].contains("do not equate"));
    }
}
