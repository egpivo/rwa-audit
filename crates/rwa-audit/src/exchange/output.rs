use std::fs;
use std::path::Path;

use crate::exchange::bridged::BridgedValueSum;
use crate::exchange::rwa_xyz::PlatformSnapshot;
use crate::flow::gecko::SymbolPoolAggregate;
use crate::flow::jupiter::JupiterQuoteEvidence;
use crate::sources::{write_json_with_provenance, Provenance};
use anyhow::Result;
use serde::Serialize;

pub use crate::core::manifest::AuditManifest as ExchangeManifest;

#[derive(Debug, Serialize)]
pub struct DepthPanelRow {
    pub date: String,
    pub asset_or_example: String,
    pub venue_or_surface: String,
    pub metric_type: String,
    pub metric_value: String,
    pub unit: String,
    pub quote_size_usd: String,
    pub source_url: String,
    pub accessed_date: String,
    pub confidence: String,
    pub caveat: String,
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

pub fn write_sourced_json(
    path: &Path,
    value: &impl Serialize,
    provenance: &Provenance,
) -> Result<()> {
    write_json_with_provenance(path, value, provenance.clone())
}

pub fn write_depth_panel(path: &Path, rows: &[DepthPanelRow]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn platform_row(snap: &PlatformSnapshot, accessed: &str) -> DepthPanelRow {
    DepthPanelRow {
        date: snap.date.clone(),
        asset_or_example: "xStocks platform".into(),
        venue_or_surface: "platform_transfer_volume".into(),
        metric_type: "monthly_transfer_volume".into(),
        metric_value: format!("{:.2}", snap.monthly_transfer_volume_usd),
        unit: "USD".into(),
        quote_size_usd: String::new(),
        source_url: snap.source_url.clone(),
        accessed_date: accessed.into(),
        confidence: snap.confidence.clone(),
        caveat: format!(
            "RWA.xyz on-chain holder-to-holder transfers; excludes mint/burn; NOT CEX trading volume. {}",
            snap.caveat
        ),
    }
}

pub fn bridged_row(b: &BridgedValueSum, accessed: &str) -> DepthPanelRow {
    DepthPanelRow {
        date: b.date.clone(),
        asset_or_example: "xStocks platform".into(),
        venue_or_surface: "platform_bridged_value".into(),
        metric_type: "bridged_token_value_total".into(),
        metric_value: format!("{:.2}", b.total_usd),
        unit: "USD".into(),
        quote_size_usd: String::new(),
        source_url: "https://app.rwa.xyz/platforms/xstocks".into(),
        accessed_date: accessed.into(),
        confidence: "high".into(),
        caveat: "Sum of RWA.xyz CSV export Bridged Token Value (Dollar) row; not transfer flow."
            .into(),
    }
}

pub fn gecko_row(agg: &SymbolPoolAggregate, metric_type: &str, accessed: &str) -> DepthPanelRow {
    let value = if metric_type == "pool_tvl_total" {
        agg.total_tvl_usd
    } else {
        agg.total_24h_vol_usd
    };
    DepthPanelRow {
        date: accessed.into(),
        asset_or_example: agg.symbol.clone(),
        venue_or_surface: "dex_amm_pool".into(),
        metric_type: metric_type.into(),
        metric_value: format!("{value:.2}"),
        unit: "USD".into(),
        quote_size_usd: String::new(),
        source_url: agg.source_url.clone(),
        accessed_date: accessed.into(),
        confidence: "medium".into(),
        caveat: format!(
            "Solana public pools n={}; outliers >$50M TVL excluded; GeckoTerminal search aggregate",
            agg.pool_count
        ),
    }
}

pub fn jupiter_row(q: &JupiterQuoteEvidence, accessed: &str) -> DepthPanelRow {
    DepthPanelRow {
        date: accessed.into(),
        asset_or_example: "AAPLx".into(),
        venue_or_surface: "aggregator_quote".into(),
        metric_type: "price_impact_pct".into(),
        metric_value: q
            .price_impact_pct
            .map(|p| format!("{p:.4}"))
            .unwrap_or_else(|| "N/A".into()),
        unit: "percent".into(),
        quote_size_usd: q.input_amount_usd.to_string(),
        source_url: q.source_url.clone(),
        accessed_date: accessed.into(),
        confidence: "high".into(),
        caveat: "Jupiter lite-api quote only; not executed trade or exit capacity.".into(),
    }
}

// Re-export for callers that build claims inline.
pub type ManifestClaim = crate::core::manifest::ManifestClaim;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::SourceId;

    fn temp_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rwa-exchange-output-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn row_builders_preserve_metric_semantics() {
        let platform = platform_row(
            &PlatformSnapshot {
                date: "2026-06-01".into(),
                monthly_transfer_volume_usd: 1_234.5,
                source_url: "https://example.test/platform".into(),
                accessed_date: "2026-06-02".into(),
                confidence: "high".into(),
                caveat: "snapshot".into(),
                source_type: None,
                exclude_from_interpolation: None,
            },
            "2026-06-20",
        );
        assert_eq!(platform.metric_value, "1234.50");
        assert!(platform.caveat.contains("NOT CEX trading volume"));

        let bridged = bridged_row(
            &BridgedValueSum {
                date: "2026-06-11".into(),
                total_usd: 765.4,
                source_file: "seed.csv".into(),
            },
            "2026-06-20",
        );
        assert_eq!(bridged.metric_type, "bridged_token_value_total");
        assert_eq!(bridged.metric_value, "765.40");

        let aggregate = SymbolPoolAggregate {
            symbol: "AAPLx".into(),
            pool_count: 3,
            total_tvl_usd: 100.0,
            total_24h_vol_usd: 25.0,
            top_pool_vol_share: Some(0.8),
            source_url: "https://example.test/gecko".into(),
            provenance: None,
        };
        assert_eq!(
            gecko_row(&aggregate, "pool_tvl_total", "2026-06-20").metric_value,
            "100.00"
        );
        assert_eq!(
            gecko_row(&aggregate, "volume_24h_total", "2026-06-20").metric_value,
            "25.00"
        );

        let quote = JupiterQuoteEvidence {
            input_mint: "in".into(),
            output_mint: "out".into(),
            input_symbol: "USDC".into(),
            output_symbol: "AAPLx".into(),
            input_amount_usd: 100_000,
            input_amount_raw: 100_000_000_000,
            slippage_bps: 100,
            price_impact_pct: Some(68.2),
            out_amount_raw: None,
            route_labels: vec![],
            source_url: "https://example.test/jupiter".into(),
            provenance: None,
            raw_response: serde_json::json!({}),
        };
        assert_eq!(jupiter_row(&quote, "2026-06-20").metric_value, "68.2000");

        let mut no_impact = quote;
        no_impact.price_impact_pct = None;
        assert_eq!(jupiter_row(&no_impact, "2026-06-20").metric_value, "N/A");
    }

    #[test]
    fn writers_emit_json_csv_and_provenance_envelope() {
        let dir = temp_dir();
        write_json(&dir.join("plain.json"), &serde_json::json!({"ok": true})).unwrap();
        assert!(std::fs::read_to_string(dir.join("plain.json"))
            .unwrap()
            .ends_with('\n'));

        let provenance = Provenance::new(SourceId::Jupiter, "https://example.test/quote", false);
        write_sourced_json(
            &dir.join("sourced.json"),
            &serde_json::json!({"value": 1}),
            &provenance,
        )
        .unwrap();
        let sourced: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("sourced.json")).unwrap())
                .unwrap();
        assert_eq!(sourced["provenance"]["source"], "jupiter");
        assert_eq!(sourced["data"]["value"], 1);

        let row = DepthPanelRow {
            date: "2026-06-20".into(),
            asset_or_example: "AAPLx".into(),
            venue_or_surface: "dex".into(),
            metric_type: "volume".into(),
            metric_value: "42.00".into(),
            unit: "USD".into(),
            quote_size_usd: String::new(),
            source_url: "u".into(),
            accessed_date: "2026-06-20".into(),
            confidence: "high".into(),
            caveat: "c".into(),
        };
        write_depth_panel(&dir.join("panel.csv"), &[row]).unwrap();
        let csv = std::fs::read_to_string(dir.join("panel.csv")).unwrap();
        assert!(csv.contains("asset_or_example"));
        assert!(csv.contains("AAPLx"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
