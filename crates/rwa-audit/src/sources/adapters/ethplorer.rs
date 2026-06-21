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

    use crate::sources::cache::ResponseCache;
    use crate::sources::context::SourceContext;
    use crate::sources::profile::{SourceKind, SourceProfile};
    use crate::sources::registry::SourceRegistry;
    use crate::sources::test_support::MockHttpServer;
    use std::collections::HashMap;

    fn context_for_ethplorer(base_url: &str) -> SourceContext {
        let profile = SourceProfile {
            id: SourceId::Ethplorer,
            kind: SourceKind::Http,
            base_url: Some(base_url.to_string()),
            rpc_endpoints: HashMap::new(),
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        };
        SourceContext::with_registry(SourceRegistry::from_profiles(HashMap::from([(
            SourceId::Ethplorer,
            profile,
        )])))
        .unwrap()
        .with_cache(ResponseCache::disabled())
    }

    #[test]
    fn fetch_injects_api_key() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"name":"BUIDL"}"#);
        let ctx = context_for_ethplorer(&server.url);
        let adapter = EthplorerAdapter {
            api_key: "test_key".into(),
        };
        let resp = adapter
            .fetch(
                &ctx,
                SourceRequest::HttpGet {
                    url: server.url.clone(),
                    query: vec![],
                },
            )
            .unwrap();
        let request = server.request();
        assert!(request.contains("apiKey=test_key"), "request: {request}");
        assert_eq!(resp.body["name"], "BUIDL");
    }

    #[test]
    fn fetch_strips_error_body() {
        let server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"error":{"code":1},"name":"X"}"#,
        );
        let ctx = context_for_ethplorer(&server.url);
        let adapter = EthplorerAdapter::default();
        let resp = adapter
            .fetch(
                &ctx,
                SourceRequest::HttpGet {
                    url: server.url.clone(),
                    query: vec![],
                },
            )
            .unwrap();
        server.request();
        // error key present → body replaced with {}
        assert_eq!(resp.body, serde_json::json!({}));
    }

    #[test]
    fn token_info_fetches_get_token_info_path() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"symbol":"BUIDL"}"#);
        let ctx = context_for_ethplorer(&server.url);
        let adapter = EthplorerAdapter::default();
        let info = adapter.token_info(&ctx, "0xabc").unwrap();
        let request = server.request();
        assert!(
            request.contains("/getTokenInfo/0xabc"),
            "request: {request}"
        );
        assert_eq!(info["symbol"], "BUIDL");
    }

    #[test]
    fn top_holders_parses_holders_array() {
        let server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"holders":[{"address":"0x1","share":0.5}]}"#,
        );
        let ctx = context_for_ethplorer(&server.url);
        let adapter = EthplorerAdapter::default();
        let holders = adapter.top_holders(&ctx, "0xabc", 10).unwrap();
        server.request();
        assert_eq!(holders.len(), 1);
        assert_eq!(holders[0]["address"], "0x1");
    }

    #[test]
    fn top_holders_returns_empty_when_no_holders_key() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"data":[]}"#);
        let ctx = context_for_ethplorer(&server.url);
        let adapter = EthplorerAdapter::default();
        let holders = adapter.top_holders(&ctx, "0xabc", 10).unwrap();
        server.request();
        assert!(holders.is_empty());
    }
}
