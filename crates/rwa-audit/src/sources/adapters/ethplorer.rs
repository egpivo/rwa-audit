use anyhow::Result;
use serde_json::{json, Value};

use crate::config::SLEEP_BETWEEN_API_MS;

use super::super::adapter::SourceAdapter;
use super::super::cache::ResponseCache;
use super::super::transport::HttpTransport;
use super::super::types::{Provenance, SourceId, SourceRequest, SourceResponse};

pub struct EthplorerAdapter {
    pub api_key: String,
}

impl Default for EthplorerAdapter {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ETHPLORER_API_KEY").unwrap_or_else(|_| "freekey".into()),
        }
    }
}

impl SourceAdapter for EthplorerAdapter {
    fn id(&self) -> SourceId {
        SourceId::Ethplorer
    }

    fn fetch(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        req: SourceRequest,
    ) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            anyhow::bail!("EthplorerAdapter expects HttpGet request");
        };
        let mut q = query.clone();
        if !q.iter().any(|(k, _)| k == "apiKey") {
            q.push(("apiKey".into(), self.api_key.clone()));
        }
        let key = ResponseCache::key_from_url(
            &url,
            &q.iter()
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
        let params: Vec<(&str, &str)> = q.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let body = transport
            .http_get(&url, &params, 3)?
            .filter(|v| v.get("error").is_none())
            .unwrap_or(json!({}));
        cache.put(self.id(), &key, &body)?;
        Ok(SourceResponse {
            provenance: Provenance::new(self.id(), &url, true)
                .with_sha256(ResponseCache::body_sha256(&body)),
            body,
        })
    }
}

impl EthplorerAdapter {
    pub fn token_info(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        contract: &str,
    ) -> Result<Value> {
        let url = format!("https://api.ethplorer.io/getTokenInfo/{contract}");
        Ok(self
            .fetch(
                transport,
                cache,
                SourceRequest::HttpGet { url, query: vec![] },
            )?
            .body)
    }

    pub fn top_holders(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        contract: &str,
        limit: u32,
    ) -> Result<Vec<Value>> {
        let url = format!("https://api.ethplorer.io/getTopTokenHolders/{contract}");
        let body = self
            .fetch(
                transport,
                cache,
                SourceRequest::HttpGet {
                    url,
                    query: vec![("limit".into(), limit.to_string())],
                },
            )?
            .body;
        Ok(body
            .get("holders")
            .and_then(|h| serde_json::from_value(h.clone()).ok())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_holders_fixture() {
        let path = crate::sources::types::repo_fixture("ethplorer_top_holders.json");
        if !path.exists() {
            return;
        }
        let body: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let holders = body
            .get("holders")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(!holders.is_empty());
        assert!(holders[0].get("share").is_some());
    }
}
