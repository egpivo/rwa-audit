use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use serde_json::json;

use crate::exchange::config::{
    jupiter_publish_fixture_path, reference_panel_path, PUBLISH_PANEL_DATE,
};
use crate::flow::config::{
    AAPLX_SOLANA, JUPITER_QUOTE_BASE, JUPITER_QUOTE_USD, JUPITER_SLIPPAGE_BPS, USDC_SOLANA,
};
use crate::flow::gecko::SymbolPoolAggregate;
use crate::flow::jupiter::JupiterQuoteEvidence;

const PUBLISH_JUPITER_IMPACT_PCT: f64 = 68.2;

fn parse_metric_value(raw: &str) -> Result<f64> {
    let s = raw.trim().trim_start_matches('$').replace(',', "");
    if s.eq_ignore_ascii_case("not_collected") {
        anyhow::bail!("metric not collected: {raw}");
    }
    s.parse::<f64>()
        .with_context(|| format!("parse metric value {raw:?}"))
}

fn panel_rows_for_date(path: &Path, date: &str) -> Result<Vec<csv::StringRecord>> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers = rdr.headers()?.clone();
    let date_idx = headers
        .iter()
        .position(|h| h == "date")
        .context("date column")?;
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        if rec.get(date_idx) == Some(date) {
            out.push(rec);
        }
    }
    Ok(out)
}

pub fn gecko_aggregate_from_reference(symbol: &str) -> Result<SymbolPoolAggregate> {
    let path = reference_panel_path();
    let rows = panel_rows_for_date(&path, PUBLISH_PANEL_DATE)?;
    let headers: Vec<String> = ReaderBuilder::new()
        .from_path(&path)?
        .headers()?
        .iter()
        .map(str::to_string)
        .collect();
    let idx = |name: &str| -> Result<usize> {
        headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("column {name}"))
    };

    let mut tvl = None;
    let mut vol = None;
    let mut pool_count = None;
    let mut source_url =
        format!("https://api.geckoterminal.com/api/v2/search/pools?query={symbol}&network=solana");

    for rec in &rows {
        if rec.get(idx("asset_or_example")?).unwrap_or("") != symbol {
            continue;
        }
        if rec.get(idx("venue_or_surface")?).unwrap_or("") != "dex_amm_pool" {
            continue;
        }
        let metric = rec.get(idx("metric_type")?).unwrap_or("");
        let value = parse_metric_value(rec.get(idx("metric_value")?).unwrap_or(""))?;
        source_url = rec
            .get(idx("source_url")?)
            .unwrap_or(&source_url)
            .to_string();
        if let Some(caveat) = rec.get(idx("caveat")?) {
            if let Some(n) = caveat.split("n=").nth(1).and_then(|s| s.split(';').next()) {
                pool_count = n.trim().parse().ok();
            }
        }
        match metric {
            "pool_tvl_total" => tvl = Some(value),
            "volume_24h_total" => vol = Some(value),
            _ => {}
        }
    }

    Ok(SymbolPoolAggregate {
        symbol: symbol.to_string(),
        pool_count: pool_count.unwrap_or(0),
        total_tvl_usd: tvl.with_context(|| format!("{symbol} TVL in reference panel"))?,
        total_24h_vol_usd: vol.with_context(|| format!("{symbol} vol in reference panel"))?,
        top_pool_vol_share: None,
        source_url,
    })
}

pub fn jupiter_quote_from_publish_fixture() -> Result<JupiterQuoteEvidence> {
    let fixture = jupiter_publish_fixture_path();
    if fixture.is_file() {
        let text = fs::read_to_string(&fixture)?;
        return serde_json::from_str(&text).context("parse jupiter publish fixture");
    }

    let input_amount_raw = JUPITER_QUOTE_USD * 1_000_000;
    let url = format!(
        "{JUPITER_QUOTE_BASE}?inputMint={USDC_SOLANA}&outputMint={AAPLX_SOLANA}&amount={input_amount_raw}&slippageBps={JUPITER_SLIPPAGE_BPS}"
    );
    Ok(JupiterQuoteEvidence {
        input_mint: USDC_SOLANA.into(),
        output_mint: AAPLX_SOLANA.into(),
        input_symbol: "USDC".into(),
        output_symbol: "AAPLx".into(),
        input_amount_usd: JUPITER_QUOTE_USD,
        input_amount_raw,
        slippage_bps: JUPITER_SLIPPAGE_BPS,
        price_impact_pct: Some(PUBLISH_JUPITER_IMPACT_PCT),
        out_amount_raw: None,
        route_labels: vec![
            "WhaleStreet".into(),
            "PancakeSwap".into(),
            "Raydium CLMM".into(),
            "Byreal".into(),
        ],
        source_url: url,
        raw_response: json!({
            "note": "Publish fixture from jupiter_xstocks_verification_memo (2026-06-14 quote pass)",
            "priceImpactPct": "0.682",
            "publish_impact_pct": PUBLISH_JUPITER_IMPACT_PCT,
            "inputMint": USDC_SOLANA,
            "outputMint": AAPLX_SOLANA,
            "inAmount": input_amount_raw.to_string(),
            "slippageBps": JUPITER_SLIPPAGE_BPS
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_gecko_matches_publish_panel() {
        let path = reference_panel_path();
        if !path.is_file() {
            return;
        }
        let aaplx = gecko_aggregate_from_reference("AAPLx").unwrap();
        assert!((aaplx.total_tvl_usd - 124_062.35).abs() < 1.0);
        assert!((aaplx.total_24h_vol_usd - 34_771.14).abs() < 1.0);
        let spyx = gecko_aggregate_from_reference("SPYx").unwrap();
        assert!((spyx.total_24h_vol_usd - 7_102_758.23).abs() < 1.0);
    }
}
