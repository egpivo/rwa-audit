use crate::tools::types::ToolResult;
use crate::tools::TOOL_SURFACE_COMPRESSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSurface {
    PlatformTransferVolume,
    BridgedTokenValue,
    DexPoolTvl,
    DexPoolVolume24h,
    AggregatorQuote,
    Unknown,
}

impl RecordSurface {
    pub fn from_panel(metric_type: &str, venue_or_surface: &str) -> Self {
        let mt = metric_type.to_ascii_lowercase();
        let venue = venue_or_surface.to_ascii_lowercase();
        if mt.contains("monthly_transfer") || venue.contains("platform_transfer") {
            return Self::PlatformTransferVolume;
        }
        if mt.contains("bridged") {
            return Self::BridgedTokenValue;
        }
        if mt.contains("pool_tvl") {
            return Self::DexPoolTvl;
        }
        if mt.contains("volume_24h") {
            return Self::DexPoolVolume24h;
        }
        if mt.contains("price_impact") || venue.contains("aggregator") {
            return Self::AggregatorQuote;
        }
        Self::Unknown
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlatformTransferVolume => "platform_transfer_volume",
            Self::BridgedTokenValue => "bridged_token_value",
            Self::DexPoolTvl => "dex_pool_tvl",
            Self::DexPoolVolume24h => "dex_pool_volume_24h",
            Self::AggregatorQuote => "aggregator_quote",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelMetricRow {
    pub asset_or_example: String,
    pub venue_or_surface: String,
    pub metric_type: String,
    pub metric_value_usd: f64,
    pub caveat: String,
}

/// Article 3: quantify order-of-magnitude gaps across record surfaces.
pub fn surface_compression(rows: &[PanelMetricRow]) -> ToolResult {
    let mut result = ToolResult::new(TOOL_SURFACE_COMPRESSION);
    let positives: Vec<&PanelMetricRow> =
        rows.iter().filter(|r| r.metric_value_usd > 0.0).collect();
    if positives.is_empty() {
        return result.gap("no positive USD panel rows");
    }

    for row in &positives {
        let surface = RecordSurface::from_panel(&row.metric_type, &row.venue_or_surface);
        result = result
            .metric(
                format!(
                    "{}_{}",
                    row.asset_or_example.replace(' ', "_"),
                    row.metric_type
                ),
                row.metric_value_usd,
                "USD",
            )
            .label(format!(
                "{} / {}: {} = ${:.2}",
                row.asset_or_example,
                surface.as_str(),
                row.metric_type,
                row.metric_value_usd
            ));
    }

    let values: Vec<f64> = positives.iter().map(|r| r.metric_value_usd).collect();
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ratio = if min > 0.0 { max / min } else { 0.0 };
    let log_span = if min > 0.0 && max > 0.0 {
        max.log10() - min.log10()
    } else {
        0.0
    };

    result
        .metric("max_to_min_ratio", ratio, "ratio")
        .metric("log10_span", log_span, "orders_of_magnitude")
        .label(format!(
            "surface compression: {:.1}x span ({:.1} log10 orders)",
            ratio, log_span
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_fixture() -> Vec<PanelMetricRow> {
        vec![
            PanelMetricRow {
                asset_or_example: "xStocks platform".into(),
                venue_or_surface: "platform_transfer_volume".into(),
                metric_type: "monthly_transfer_volume".into(),
                metric_value_usd: 1_600_000_000.0,
                caveat: String::new(),
            },
            PanelMetricRow {
                asset_or_example: "AAPLx".into(),
                venue_or_surface: "dex_amm_pool".into(),
                metric_type: "pool_tvl_total".into(),
                metric_value_usd: 124_062.35,
                caveat: String::new(),
            },
        ]
    }

    #[test]
    fn compression_reports_large_ratio_between_surfaces() {
        let result = surface_compression(&panel_fixture());
        let ratio = result
            .metrics
            .iter()
            .find(|m| m.name == "max_to_min_ratio")
            .unwrap()
            .value;
        assert!(ratio > 10_000.0);
    }

    #[test]
    fn record_surface_detects_platform_transfer() {
        let s = RecordSurface::from_panel("monthly_transfer_volume", "platform_transfer_volume");
        assert_eq!(s, RecordSurface::PlatformTransferVolume);
    }
}
