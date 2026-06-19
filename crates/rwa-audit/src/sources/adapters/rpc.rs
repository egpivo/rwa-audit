use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::models::RpcResponse;

use super::super::adapter::SourceAdapter;
use super::super::cache::ResponseCache;
use super::super::transport::HttpTransport;
use super::super::types::{Provenance, SourceId, SourceRequest, SourceResponse};

pub struct PublicNodeRpcAdapter;

impl SourceAdapter for PublicNodeRpcAdapter {
    fn id(&self) -> SourceId {
        SourceId::PublicNodeRpc
    }

    fn fetch(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        req: SourceRequest,
    ) -> Result<SourceResponse> {
        let SourceRequest::Rpc {
            url,
            method,
            params,
        } = req
        else {
            anyhow::bail!("PublicNodeRpcAdapter expects Rpc request");
        };
        let key = ResponseCache::key_from_rpc(&url, &method, &params);
        if let Some(body) = cache.get_rpc(&method, &params, self.id(), &key) {
            return Ok(SourceResponse {
                provenance: Provenance::new(self.id(), &url, false),
                body,
            });
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });
        let body = transport
            .rpc_post(&url, &payload, 3)?
            .context("rpc request failed")?;
        cache.put_rpc(&method, &params, self.id(), &key, &body)?;
        Ok(SourceResponse {
            provenance: Provenance::new(self.id(), &url, true)
                .with_sha256(ResponseCache::body_sha256(&body)),
            body,
        })
    }
}

impl PublicNodeRpcAdapter {
    pub fn rpc_call(
        transport: &HttpTransport,
        cache: &ResponseCache,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        let adapter = Self;
        let key = ResponseCache::key_from_rpc(rpc_url, method, &params);
        if let Some(body) = cache.get_rpc(method, &params, adapter.id(), &key) {
            return Ok(serde_json::from_value(body).ok());
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        for attempt in 0..retries {
            if let Some(body) = transport.rpc_post(rpc_url, &payload, 1)? {
                cache.put_rpc(method, &params, adapter.id(), &key, &body)?;
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
}
