use anyhow::{bail, Result};
use serde_json::Value;

use super::super::adapter::SourceAdapter;
use super::super::context::SourceContext;
use super::super::fetch::http_get_cached_or_error;
use super::super::types::{SourceId, SourceRequest, SourceResponse};

pub struct ParaSwapAdapter;

impl SourceAdapter for ParaSwapAdapter {
    fn id(&self) -> SourceId {
        SourceId::ParaSwap
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("ParaSwapAdapter expects HttpGet request");
        };
        http_get_cached_or_error(self, ctx, &url, &query, &[])
    }
}

pub type ParaSwapQuoteResult = (bool, Option<f64>, String, Option<String>, Value);

impl ParaSwapAdapter {
    fn base_url(ctx: &SourceContext) -> Result<String> {
        ctx.http_base_url(SourceId::ParaSwap)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fetch_price_route(
        &self,
        ctx: &SourceContext,
        src_token: &str,
        dest_token: &str,
        raw_amount: u128,
        src_decimals: u32,
        dest_decimals: u32,
        network: &str,
    ) -> Result<ParaSwapQuoteResult> {
        let base = Self::base_url(ctx)?;
        let url = format!("{base}/prices/");
        let resp = self.fetch(
            ctx,
            SourceRequest::HttpGet {
                url,
                query: vec![
                    ("srcToken".into(), src_token.into()),
                    ("destToken".into(), dest_token.into()),
                    ("amount".into(), raw_amount.to_string()),
                    ("srcDecimals".into(), src_decimals.to_string()),
                    ("destDecimals".into(), dest_decimals.to_string()),
                    ("side".into(), "SELL".into()),
                    ("network".into(), network.into()),
                ],
            },
        )?;
        Ok(Self::parse_price_route(&resp.body, dest_decimals))
    }

    pub fn parse_price_route(body: &Value, dest_decimals: u32) -> ParaSwapQuoteResult {
        if body.get("priceRoute").is_none() {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("no route")
                .to_string();
            return (false, None, String::new(), Some(err), body.clone());
        }

        let route = match body.get("priceRoute") {
            Some(r) => r,
            None => {
                let err = body
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("no route")
                    .to_string();
                return (false, None, String::new(), Some(err), body.clone());
            }
        };

        let dest_raw = route
            .get("destAmount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .map(|a| a / 10f64.powi(dest_decimals as i32));

        let exchanges: Vec<String> = route
            .pointer("/bestRoute/0/swaps")
            .and_then(|s| s.as_array())
            .map(|swaps| {
                swaps
                    .iter()
                    .filter_map(|sw| {
                        sw.get("swapExchanges")
                            .and_then(|e| e.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|ex| ex.get("exchange"))
                            .and_then(|e| e.as_str())
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();

        (true, dest_raw, exchanges.join(" → "), None, body.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_price_route_scales_by_dest_decimals() {
        let body = json!({
            "priceRoute": {
                "destAmount": "1000000000000000000",
                "bestRoute": { "0": { "swaps": [] } }
            }
        });
        let (_, amount_18, _, _, _) = ParaSwapAdapter::parse_price_route(&body, 18);
        assert!((amount_18.unwrap() - 1.0).abs() < f64::EPSILON);

        let (_, amount_6, _, _, _) = ParaSwapAdapter::parse_price_route(&body, 6);
        assert!((amount_6.unwrap() - 1e12).abs() < 1.0);
    }
}
