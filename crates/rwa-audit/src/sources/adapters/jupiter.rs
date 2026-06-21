use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::flow::config::{AAPLX_SOLANA, JUPITER_QUOTE_USD, JUPITER_SLIPPAGE_BPS, USDC_SOLANA};

use super::super::adapter::SourceAdapter;
use super::super::context::SourceContext;
use super::super::fetch::http_get_cached;
use super::super::types::{Provenance, SourceId, SourceRequest, SourceResponse};

pub struct JupiterAdapter;

impl SourceAdapter for JupiterAdapter {
    fn id(&self) -> SourceId {
        SourceId::Jupiter
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("JupiterAdapter expects HttpGet request");
        };
        http_get_cached(self, ctx, &url, &query, &[])
    }
}

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
    #[serde(skip)]
    pub provenance: Option<Provenance>,
    pub raw_response: Value,
}

impl JupiterAdapter {
    fn quote_base(ctx: &SourceContext) -> Result<String> {
        ctx.http_base_url(SourceId::Jupiter)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn quote(
        ctx: &SourceContext,
        input_mint: &str,
        output_mint: &str,
        amount_raw: u64,
        slippage_bps: u32,
        input_symbol: &str,
        output_symbol: &str,
        input_amount_usd: u64,
    ) -> Result<JupiterQuoteEvidence> {
        let base = Self::quote_base(ctx)?;
        let url = format!("{base}/quote");
        let adapter = JupiterAdapter;
        let resp = adapter.fetch(
            ctx,
            SourceRequest::HttpGet {
                url: url.clone(),
                query: vec![
                    ("inputMint".into(), input_mint.into()),
                    ("outputMint".into(), output_mint.into()),
                    ("amount".into(), amount_raw.to_string()),
                    ("slippageBps".into(), slippage_bps.to_string()),
                ],
            },
        )?;
        Self::parse_quote_evidence(
            resp.body,
            resp.provenance,
            input_mint,
            output_mint,
            input_symbol,
            output_symbol,
            input_amount_usd,
            amount_raw,
            slippage_bps,
        )
    }

    pub fn fetch_aaplx_quote_100k(ctx: &SourceContext) -> Result<JupiterQuoteEvidence> {
        let input_amount_raw = JUPITER_QUOTE_USD * 1_000_000;
        Self::quote(
            ctx,
            USDC_SOLANA,
            AAPLX_SOLANA,
            input_amount_raw,
            JUPITER_SLIPPAGE_BPS,
            "USDC",
            "AAPLx",
            JUPITER_QUOTE_USD,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn parse_quote_evidence(
        body: Value,
        provenance: Provenance,
        input_mint: &str,
        output_mint: &str,
        input_symbol: &str,
        output_symbol: &str,
        input_amount_usd: u64,
        input_amount_raw: u64,
        slippage_bps: u32,
    ) -> Result<JupiterQuoteEvidence> {
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
            input_mint: input_mint.into(),
            output_mint: output_mint.into(),
            input_symbol: input_symbol.into(),
            output_symbol: output_symbol.into(),
            input_amount_usd,
            input_amount_raw,
            slippage_bps,
            price_impact_pct: price_impact,
            out_amount_raw: body
                .get("outAmount")
                .and_then(|v| v.as_str().map(str::to_string)),
            route_labels,
            source_url: provenance.request_url.clone(),
            provenance: Some(provenance),
            raw_response: body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance::new(SourceId::Jupiter, "https://example.test/quote", true)
    }

    #[test]
    fn evidence_keeps_runtime_provenance_out_of_payload() {
        let evidence = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({
                "priceImpactPct": "0.01",
                "outAmount": "42",
                "routePlan": []
            }),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();

        assert!(evidence.provenance.is_some());
        assert!(serde_json::to_value(&evidence)
            .unwrap()
            .get("provenance")
            .is_none());
    }

    #[test]
    fn parse_price_impact_large_float_not_scaled() {
        let ev = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({"priceImpactPct": 5.0}),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();
        assert!((ev.price_impact_pct.unwrap() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_price_impact_small_float_is_scaled_by_100() {
        let ev = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({"priceImpactPct": 0.01}),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();
        assert!((ev.price_impact_pct.unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn parse_price_impact_missing_field_is_none() {
        let ev = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({}),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();
        assert!(ev.price_impact_pct.is_none());
    }

    #[test]
    fn parse_route_plan_extracts_labels() {
        let ev = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({
                "priceImpactPct": "0.5",
                "outAmount": "42",
                "routePlan": [
                    {"swapInfo": {"label": "Orca"}},
                    {"swapInfo": {"label": "Raydium"}}
                ]
            }),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();
        assert_eq!(ev.route_labels, vec!["Orca", "Raydium"]);
        assert_eq!(ev.out_amount_raw.as_deref(), Some("42"));
    }

    #[test]
    fn parse_missing_out_amount_returns_none() {
        let ev = JupiterAdapter::parse_quote_evidence(
            serde_json::json!({"priceImpactPct": "1.5"}),
            prov(),
            "in",
            "out",
            "IN",
            "OUT",
            100,
            100_000_000,
            100,
        )
        .unwrap();
        assert!(ev.out_amount_raw.is_none());
    }
}
