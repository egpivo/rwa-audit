use anyhow::Result;
use serde_json::{json, Value};

use crate::models::RpcResponse;

use super::super::adapter::SourceAdapter;
use super::super::cache::ResponseCache;
use super::super::context::SourceContext;
use super::super::fetch::rpc_fetch_cached;
use super::super::types::{SourceId, SourceRequest, SourceResponse};

pub struct PublicNodeRpcAdapter;

impl SourceAdapter for PublicNodeRpcAdapter {
    fn id(&self) -> SourceId {
        SourceId::PublicNodeRpc
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::Rpc {
            url,
            method,
            params,
        } = req
        else {
            anyhow::bail!("PublicNodeRpcAdapter expects Rpc request");
        };
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });
        rpc_fetch_cached(self, ctx, &url, &method, &params, &payload)
    }
}

impl PublicNodeRpcAdapter {
    pub fn rpc_call(
        ctx: &SourceContext,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        let adapter = Self;
        let key = ResponseCache::key_from_rpc(rpc_url, method, &params);
        if let Some(body) = ctx.cache().get_rpc(method, &params, adapter.id(), &key) {
            return Ok(serde_json::from_value(body).ok());
        }

        ctx.rate_limit_sleep(adapter.id());

        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        for attempt in 0..retries {
            if let Some(body) = ctx.transport().rpc_post(rpc_url, &payload, 1)? {
                ctx.cache()
                    .put_rpc(method, &params, adapter.id(), &key, &body)?;
                return Ok(serde_json::from_value(body).ok());
            }
            if attempt + 1 < retries {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_fixture_rpc_block_number() {
        let path = crate::sources::types::repo_fixture("rpc_eth_blockNumber.json");
        if !path.exists() {
            return;
        }
        let body: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let rpc: RpcResponse = serde_json::from_value(body).unwrap();
        let result = rpc.result.unwrap();
        let hex = result.as_str().unwrap();
        assert!(hex.starts_with("0x"));
    }

    #[test]
    fn cache_key_differs_by_method() {
        let a = ResponseCache::key_from_rpc("http://x", "eth_blockNumber", &json!([]));
        let b = ResponseCache::key_from_rpc("http://x", "eth_getLogs", &json!([]));
        assert_ne!(a, b);
    }

    #[test]
    fn adapter_id_is_publicnode_rpc() {
        use crate::sources::adapter::SourceAdapter;
        assert_eq!(PublicNodeRpcAdapter.id(), SourceId::PublicNodeRpc);
    }

    #[test]
    fn adapter_rejects_http_get_request() {
        use crate::sources::adapter::SourceAdapter;
        use crate::sources::cache::ResponseCache;
        use crate::sources::profile::{SourceKind, SourceProfile};
        use crate::sources::registry::SourceRegistry;
        use std::collections::HashMap;
        let mut rpc_endpoints = HashMap::new();
        rpc_endpoints.insert("ethereum".into(), "https://eth.test".into());
        let profile = SourceProfile {
            id: SourceId::PublicNodeRpc,
            kind: SourceKind::Rpc,
            base_url: None,
            rpc_endpoints,
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        };
        let reg =
            SourceRegistry::from_profiles(HashMap::from([(SourceId::PublicNodeRpc, profile)]));
        let ctx = crate::sources::context::SourceContext::with_registry(reg)
            .unwrap()
            .with_cache(ResponseCache::disabled());
        let result = PublicNodeRpcAdapter.fetch(
            &ctx,
            SourceRequest::HttpGet {
                url: "http://x".into(),
                query: vec![],
            },
        );
        assert!(result.is_err());
    }
}
