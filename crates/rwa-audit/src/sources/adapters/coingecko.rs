use anyhow::Result;
use serde_json::Value;

use crate::config::{COINGECKO_BASE, SLEEP_BETWEEN_API_MS};

use super::super::adapter::SourceAdapter;
use super::super::cache::ResponseCache;
use super::super::transport::HttpTransport;
use super::super::types::{Provenance, SourceId, SourceRequest, SourceResponse};

pub struct CoinGeckoAdapter;

impl SourceAdapter for CoinGeckoAdapter {
    fn id(&self) -> SourceId {
        SourceId::CoinGecko
    }

    fn fetch(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        req: SourceRequest,
    ) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            anyhow::bail!("CoinGeckoAdapter expects HttpGet request");
        };
        let key = ResponseCache::key_from_url(
            &url,
            &query
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<_>>(),
        );
        if let Some(body) = cache.get(self.id(), &key) {
            return Ok(SourceResponse {
                provenance: Provenance::new(self.id(), &url, false),
                body,
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(SLEEP_BETWEEN_API_MS));
        let params: Vec<(&str, &str)> = query
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let body = transport.http_get(&url, &params, 3)?.unwrap_or(Value::Null);
        cache.put(self.id(), &key, &body)?;
        Ok(SourceResponse {
            provenance: Provenance::new(self.id(), &url, true)
                .with_sha256(ResponseCache::body_sha256(&body)),
            body,
        })
    }
}

impl CoinGeckoAdapter {
    pub fn simple_price_usd(
        transport: &HttpTransport,
        cache: &ResponseCache,
        cg_id: &str,
    ) -> Result<Option<f64>> {
        let url = format!("{COINGECKO_BASE}/simple/price");
        let adapter = Self;
        let resp = adapter.fetch(
            transport,
            cache,
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
}
