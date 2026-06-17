use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::exchange::bridged::BridgedValueSum;
use crate::exchange::rwa_xyz::PlatformSnapshot;
use crate::flow::gecko::SymbolPoolAggregate;
use crate::flow::jupiter::JupiterQuoteEvidence;

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

#[derive(Debug, Serialize)]
pub struct ManifestClaim {
    pub id: String,
    pub label: String,
    pub value_display: String,
    pub value_usd: Option<f64>,
    pub as_of: String,
    pub evidence_file: String,
    pub source_url: String,
    pub caveat: String,
}

#[derive(Debug, Serialize)]
pub struct ExchangeManifest {
    pub article: String,
    pub post_url: String,
    pub frozen_at: String,
    pub panel_date: String,
    pub claims: Vec<ManifestClaim>,
    pub do_not_claim: Vec<String>,
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
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
        caveat: "Sum of RWA.xyz CSV export Bridged Token Value (Dollar) row; not transfer flow.".into(),
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
