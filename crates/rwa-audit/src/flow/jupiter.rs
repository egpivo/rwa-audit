use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::flow::config::{
    AAPLX_SOLANA, JUPITER_QUOTE_BASE, JUPITER_QUOTE_USD, JUPITER_SLIPPAGE_BPS, USDC_SOLANA,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterQuoteEvidence {
    pub input_mint: String,
    pub output_mint: String,
    pub input_symbol: String,
    pub output_symbol: String,
    pub input_amount_usd: u64,
    pub input_amount_raw: u64,
    pub slippage_bps: u32,
    pub price_impact_pct: Option<f64>,
    pub out_amount_raw: Option<String>,
    pub route_labels: Vec<String>,
    pub source_url: String,
    pub raw_response: Value,
}

pub fn fetch_aaplx_quote_100k() -> Result<JupiterQuoteEvidence> {
    let input_amount_raw = JUPITER_QUOTE_USD * 1_000_000;
    let url = format!(
        "{JUPITER_QUOTE_BASE}?inputMint={USDC_SOLANA}&outputMint={AAPLX_SOLANA}&amount={input_amount_raw}&slippageBps={JUPITER_SLIPPAGE_BPS}"
    );
    let client = Client::builder()
        .user_agent("rwa-audit/0.1")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let body: Value = client
        .get(&url)
        .send()?
        .error_for_status()?
        .json()
        .context("jupiter quote json")?;

    let mut price_impact = body.get("priceImpactPct").and_then(|v| {
        v.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| v.as_f64())
    });
    if let Some(p) = price_impact {
        if p.abs() < 2.0 {
            price_impact = Some(p * 100.0);
        }
    }

    let route_labels = body
        .pointer("/routePlan")
        .and_then(|r| r.as_array())
        .map(|plans| {
            plans
                .iter()
                .filter_map(|p| {
                    p.get("swapInfo")
                        .and_then(|s| s.get("label"))
                        .and_then(|l| l.as_str())
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(JupiterQuoteEvidence {
        input_mint: USDC_SOLANA.into(),
        output_mint: AAPLX_SOLANA.into(),
        input_symbol: "USDC".into(),
        output_symbol: "AAPLx".into(),
        input_amount_usd: JUPITER_QUOTE_USD,
        input_amount_raw,
        slippage_bps: JUPITER_SLIPPAGE_BPS,
        price_impact_pct: price_impact,
        out_amount_raw: body
            .get("outAmount")
            .and_then(|v| v.as_str().map(str::to_string)),
        route_labels,
        source_url: url,
        raw_response: body,
    })
}
