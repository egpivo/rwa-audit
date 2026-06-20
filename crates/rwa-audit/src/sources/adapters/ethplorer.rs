use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::super::adapter::SourceAdapter;
use super::super::context::SourceContext;
use super::super::fetch::http_get_cached;
use super::super::types::{SourceId, SourceRequest, SourceResponse};

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

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("EthplorerAdapter expects HttpGet request");
        };
        let mut q = query;
        if !q.iter().any(|(k, _)| k == "apiKey") {
            q.push(("apiKey".into(), self.api_key.clone()));
        }
        let mut resp = http_get_cached(self, ctx, &url, &q, &[])?;
        if resp.body.get("error").is_some() {
            resp.body = json!({});
        }
        Ok(resp)
    }
}

impl EthplorerAdapter {
    fn base_url(ctx: &SourceContext) -> Result<String> {
        ctx.http_base_url(SourceId::Ethplorer)
    }

    pub fn token_info(&self, ctx: &SourceContext, contract: &str) -> Result<Value> {
        let base = Self::base_url(ctx)?;
        let url = format!("{base}/getTokenInfo/{contract}");
        Ok(self
            .fetch(ctx, SourceRequest::HttpGet { url, query: vec![] })?
            .body)
    }

    pub fn top_holders(
        &self,
        ctx: &SourceContext,
        contract: &str,
        limit: u32,
    ) -> Result<Vec<Value>> {
        let base = Self::base_url(ctx)?;
        let url = format!("{base}/getTopTokenHolders/{contract}");
        let body = self
            .fetch(
                ctx,
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
