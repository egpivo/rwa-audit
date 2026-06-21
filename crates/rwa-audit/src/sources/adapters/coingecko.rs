use anyhow::{bail, Result};
use serde_json::Value;

use super::super::adapter::SourceAdapter;
use super::super::capability::PriceOracle;
use super::super::context::SourceContext;
use super::super::fetch::http_get_cached;
use super::super::types::{SourceId, SourceRequest, SourceResponse};

pub struct CoinGeckoAdapter;

impl SourceAdapter for CoinGeckoAdapter {
    fn id(&self) -> SourceId {
        SourceId::CoinGecko
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("CoinGeckoAdapter expects HttpGet request");
        };
        http_get_cached(self, ctx, &url, &query, &[])
    }
}

impl CoinGeckoAdapter {
    fn base_url(ctx: &SourceContext) -> Result<String> {
        ctx.http_base_url(SourceId::CoinGecko)
    }

    pub fn simple_price_usd(ctx: &SourceContext, cg_id: &str) -> Result<Option<f64>> {
        let base = Self::base_url(ctx)?;
        let url = format!("{base}/simple/price");
        let adapter = Self;
        let resp = adapter.fetch(
            ctx,
            SourceRequest::HttpGet {
                url: url.clone(),
                query: vec![
                    ("ids".into(), cg_id.into()),
                    ("vs_currencies".into(), "usd".into()),
                ],
            },
        )?;
        Ok(Self::parse_price_usd(cg_id, &resp.body))
    }

    pub fn parse_price_usd(cg_id: &str, body: &Value) -> Option<f64> {
        body.get(cg_id)
            .and_then(|x| x.get("usd"))
            .and_then(|x| x.as_f64())
    }
}

impl PriceOracle for CoinGeckoAdapter {
    fn price_usd(&self, ctx: &SourceContext, id: &str) -> Result<Option<f64>> {
        Self::simple_price_usd(ctx, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_coingecko_fixture() {
        let path = crate::sources::types::repo_fixture("coingecko_simple_price.json");
        if !path.exists() {
            return;
        }
        let body: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let price = CoinGeckoAdapter::parse_price_usd(
            "blackrock-usd-institutional-digital-liquidity-fund",
            &body,
        );
        assert!(price.is_some());
    }

    #[test]
    fn parse_price_usd_missing_id_returns_none() {
        let body = json!({"other": {"usd": 2.0}});
        assert!(CoinGeckoAdapter::parse_price_usd("missing", &body).is_none());
    }

    #[test]
    fn parse_price_usd_returns_some_when_id_matches() {
        let body = json!({"bitcoin": {"usd": 65432.1}});
        let price = CoinGeckoAdapter::parse_price_usd("bitcoin", &body).unwrap();
        assert!((price - 65432.1).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_price_usd_missing_usd_key_returns_none() {
        let body = json!({"bitcoin": {"eur": 60000.0}});
        assert!(CoinGeckoAdapter::parse_price_usd("bitcoin", &body).is_none());
    }

    #[test]
    fn coingecko_adapter_id_is_coingecko() {
        use crate::sources::adapter::SourceAdapter;
        assert_eq!(
            CoinGeckoAdapter.id(),
            crate::sources::types::SourceId::CoinGecko
        );
    }
}
