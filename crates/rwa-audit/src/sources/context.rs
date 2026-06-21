use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::models::RpcResponse;

use super::adapter::SourceAdapter;
use super::adapters::{EthplorerAdapter, PublicNodeRpcAdapter};
use super::cache::ResponseCache;
use super::registry::SourceRegistry;
use super::transport::HttpTransport;
use super::types::{SourceId, SourceRequest, SourceResponse};

/// Shared runtime for source adapters: registry, transport, cache, and typed helpers.
pub struct SourceContext {
    transport: HttpTransport,
    cache: ResponseCache,
    registry: SourceRegistry,
    ethplorer: EthplorerAdapter,
}

impl SourceContext {
    pub fn new() -> Result<Self> {
        Self::with_registry(SourceRegistry::load_default()?)
    }

    pub fn for_live_collection() -> Result<Self> {
        let registry = SourceRegistry::load_default()?;
        let cache = registry
            .build_cache()
            .with_ttl(Some(Duration::from_secs(300)))
            .with_bypass_volatile(true);
        Ok(Self {
            transport: HttpTransport::new()?,
            cache,
            registry,
            ethplorer: EthplorerAdapter::default(),
        })
    }

    /// Force a network fetch without reading or writing the response cache.
    pub fn for_force_refresh() -> Result<Self> {
        let registry = SourceRegistry::load_default()?;
        Ok(Self {
            transport: HttpTransport::new()?,
            cache: ResponseCache::disabled(),
            registry,
            ethplorer: EthplorerAdapter::default(),
        })
    }

    pub fn with_registry(registry: SourceRegistry) -> Result<Self> {
        let cache = registry.build_cache();
        Ok(Self {
            transport: HttpTransport::new()?,
            cache,
            registry,
            ethplorer: EthplorerAdapter::default(),
        })
    }

    pub fn with_cache(mut self, cache: ResponseCache) -> Self {
        self.cache = cache;
        self
    }

    pub fn registry(&self) -> &SourceRegistry {
        &self.registry
    }

    pub fn cache(&self) -> &ResponseCache {
        &self.cache
    }

    pub fn transport(&self) -> &HttpTransport {
        &self.transport
    }

    pub fn profile(&self, id: SourceId) -> Option<&super::profile::SourceProfile> {
        self.registry.profile(id)
    }

    pub fn require_profile(&self, id: SourceId) -> Result<&super::profile::SourceProfile> {
        self.registry.require_profile(id)
    }

    pub fn http_base_url(&self, id: SourceId) -> Result<String> {
        Ok(self.require_profile(id)?.http_base()?.to_string())
    }

    pub fn rate_limit_sleep(&self, id: SourceId) {
        if let Ok(profile) = self.require_profile(id) {
            if profile.rate_limit_ms > 0 {
                thread::sleep(Duration::from_millis(profile.rate_limit_ms));
            }
        }
    }

    pub fn fetch(&self, id: SourceId, req: SourceRequest) -> Result<SourceResponse> {
        let adapter = self.registry.resolve_adapter(id)?;
        SourceAdapter::fetch(adapter, self, req)
    }

    pub fn http_get(
        &self,
        url: &str,
        params: &[(&str, &str)],
        retries: u32,
    ) -> Result<Option<Value>> {
        match self.transport.http_get(url, params, retries)? {
            super::transport::HttpGetResult::Ok(v) => Ok(Some(v)),
            super::transport::HttpGetResult::RateLimited => Ok(None),
            super::transport::HttpGetResult::ClientError { status, .. } => {
                anyhow::bail!("HTTP {status} for {url}");
            }
        }
    }

    pub fn rpc_for_chain(&self, chain: &str) -> Result<String> {
        self.registry.rpc_url(chain)
    }

    pub fn rpc_call(
        &self,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        PublicNodeRpcAdapter::rpc_call(self, rpc_url, method, params, retries)
    }

    pub fn get_current_block(&self, chain: &str) -> Result<u64> {
        let rpc = self.rpc_for_chain(chain)?;
        let r = self
            .rpc_call(&rpc, "eth_blockNumber", json!([]), 3)?
            .context("eth_blockNumber failed")?;
        if let Some(result) = r.result {
            if let Some(hex) = result.as_str() {
                return crate::evm::parse_hex_u64(hex);
            }
        }
        Ok(0)
    }

    pub fn get_latest_block_and_ts(&self, rpc_url: &str) -> Result<(u64, i64)> {
        let r = self
            .rpc_call(rpc_url, "eth_getBlockByNumber", json!(["latest", false]), 4)?
            .context("eth_getBlockByNumber failed")?;
        let block = r.result.as_ref().context("missing block result")?;
        let number = crate::evm::parse_hex_u64(block["number"].as_str().context("block number")?)?;
        let ts =
            crate::evm::parse_hex_u64(block["timestamp"].as_str().context("timestamp")?)? as i64;
        Ok((number, ts))
    }

    pub fn eth_call(&self, rpc_url: &str, contract: &str, data: &str) -> Result<Option<String>> {
        let r = self.rpc_call(
            rpc_url,
            "eth_call",
            json!([{"to": contract, "data": data}, "latest"]),
            3,
        )?;
        Ok(r.and_then(|r| r.result.and_then(|v| v.as_str().map(str::to_string))))
    }

    pub fn get_price_usd(&self, oracle: SourceId, id: &str) -> Result<Option<f64>> {
        self.registry.price_oracle(oracle)?.price_usd(self, id)
    }

    pub fn get_coingecko_price(&self, cg_id: &str) -> Result<Option<f64>> {
        self.get_price_usd(SourceId::CoinGecko, cg_id)
    }

    pub fn get_ethplorer_token_info(&self, contract: &str) -> Result<Value> {
        self.ethplorer.token_info(self, contract)
    }

    pub fn get_ethplorer_top_holders(&self, contract: &str, limit: u32) -> Result<Vec<Value>> {
        self.ethplorer.top_holders(self, contract, limit)
    }

    pub fn get_transfer_logs_chunked(
        &self,
        contract: &str,
        chain: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        let rpc = self.rpc_for_chain(chain)?;
        let mut all_logs = Vec::new();
        let mut chunk = chunk_blocks;
        let mut current = from_block;
        let mut chunk_count = 0u64;
        let max_logs = 50_000usize;

        while current <= to_block && all_logs.len() < max_logs {
            let end = (current + chunk - 1).min(to_block);

            let params = json!([{
                "address": contract.to_lowercase(),
                "topics": [crate::config::TRANSFER_TOPIC],
                "fromBlock": format!("0x{current:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);

            let r = match self.rpc_call(&rpc, "eth_getLogs", params, 3)? {
                Some(r) => r,
                None => {
                    eprintln!("    Chunk {chunk_count}: RPC call failed");
                    break;
                }
            };

            if let Some(err) = r.error {
                let msg = err.message.unwrap_or_default();
                if msg.contains("exceed maximum block range") {
                    chunk /= 2;
                    if chunk < 500 {
                        eprintln!("    Chunk too small, stopping: {msg}");
                        break;
                    }
                    eprintln!("    Block range exceeded, halving chunk to {chunk}");
                    continue;
                }
                eprintln!("    getLogs error: {msg}");
                break;
            }

            if let Some(batch) = r.result.and_then(|v| v.as_array().cloned()) {
                if !batch.is_empty() {
                    all_logs.extend(batch);
                }
            }

            chunk_count += 1;
            if chunk_count.is_multiple_of(10) {
                eprintln!(
                    "    Chunk {chunk_count}: blocks 0x{current:x}-0x{end:x}, logs so far: {}",
                    all_logs.len()
                );
            }
            current = end + 1;
        }

        Ok(all_logs)
    }

    pub fn get_logs_activity(
        &self,
        rpc_url: &str,
        contract: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        let mut logs = Vec::new();
        let mut cur = from_block;
        let mut chunk = chunk_blocks;

        while cur <= to_block {
            let end = (cur + chunk - 1).min(to_block);
            let params = json!([{
                "address": contract,
                "topics": [crate::config::TRANSFER_TOPIC],
                "fromBlock": format!("0x{cur:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);

            match self.rpc_call(rpc_url, "eth_getLogs", params, 4) {
                Ok(Some(r)) => {
                    if let Some(err) = r.error {
                        let msg = err.message.unwrap_or_default();
                        if chunk > 5000
                            && (msg.to_lowercase().contains("range")
                                || msg.to_lowercase().contains("limit"))
                        {
                            chunk /= 2;
                            continue;
                        }
                        cur = end + 1;
                        continue;
                    }
                    if let Some(batch) = r.result.and_then(|v| v.as_array().cloned()) {
                        logs.extend(batch);
                    }
                    cur = end + 1;
                }
                _ => {
                    if chunk > 5000 {
                        chunk /= 2;
                        continue;
                    }
                    cur = end + 1;
                }
            }
        }

        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::sources::profile::{SourceKind, SourceProfile};
    use crate::sources::test_support::MockHttpServer;

    fn profile(id: SourceId, base_url: &str) -> SourceProfile {
        SourceProfile {
            id,
            kind: if id == SourceId::PublicNodeRpc {
                SourceKind::Rpc
            } else {
                SourceKind::Http
            },
            base_url: Some(base_url.to_string()),
            rpc_endpoints: if id == SourceId::PublicNodeRpc {
                HashMap::from([
                    ("ethereum".into(), base_url.to_string()),
                    ("polygon".into(), base_url.to_string()),
                ])
            } else {
                HashMap::new()
            },
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        }
    }

    fn test_context(profiles: Vec<SourceProfile>) -> SourceContext {
        let profiles = profiles.into_iter().map(|p| (p.id, p)).collect();
        SourceContext::with_registry(SourceRegistry::from_profiles(profiles))
            .unwrap()
            .with_cache(ResponseCache::disabled())
    }

    #[test]
    fn source_context_constructs() {
        SourceContext::new().unwrap();
    }

    #[test]
    fn force_refresh_disables_cache() {
        let ctx = SourceContext::for_force_refresh().unwrap();
        assert!(!ctx.cache().is_enabled());
    }

    #[test]
    fn fetch_coingecko_uses_registry_base() {
        let ctx = SourceContext::new().unwrap();
        let profile = ctx.profile(SourceId::CoinGecko).unwrap();
        assert!(profile.base_url.as_ref().unwrap().contains("coingecko"));
    }

    #[test]
    fn accessors_and_registry_errors_are_exposed() {
        let ctx = test_context(vec![profile(SourceId::CoinGecko, "http://example.test")]);

        assert_eq!(
            ctx.http_base_url(SourceId::CoinGecko).unwrap(),
            "http://example.test"
        );
        assert_eq!(ctx.registry().profiles().len(), 1);
        assert!(ctx.transport().http_get("not a url", &[], 1).is_err());
        assert!(ctx.require_profile(SourceId::Jupiter).is_err());
        assert!(ctx.rpc_for_chain("ethereum").is_err());
        ctx.rate_limit_sleep(SourceId::Jupiter);
    }

    #[test]
    fn generic_http_get_and_fetch_use_transport_and_adapter() {
        let direct_server =
            MockHttpServer::spawn("200 OK", "application/json", r#"{"direct":true}"#);
        let direct_ctx = test_context(vec![profile(SourceId::CoinGecko, &direct_server.url)]);
        let body = direct_ctx
            .http_get(&direct_server.url, &[("q", "audit")], 1)
            .unwrap()
            .unwrap();
        assert_eq!(body["direct"], true);
        direct_server.request();

        let fetch_server =
            MockHttpServer::spawn("200 OK", "application/json", r#"{"adapter":true}"#);
        let fetch_ctx = test_context(vec![profile(SourceId::CoinGecko, &fetch_server.url)]);
        let response = fetch_ctx
            .fetch(
                SourceId::CoinGecko,
                SourceRequest::HttpGet {
                    url: fetch_server.url.clone(),
                    query: vec![],
                },
            )
            .unwrap();
        assert_eq!(response.body["adapter"], true);
        fetch_server.request();
    }

    #[test]
    fn rpc_block_helpers_parse_results() {
        let block_server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x2a"}"#,
        );
        let block_ctx = test_context(vec![profile(SourceId::PublicNodeRpc, &block_server.url)]);
        assert_eq!(block_ctx.get_current_block("Ethereum").unwrap(), 42);
        block_server.request();

        let latest_server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":{"number":"0x2b","timestamp":"0x64"}}"#,
        );
        let latest_ctx = test_context(vec![profile(SourceId::PublicNodeRpc, &latest_server.url)]);
        assert_eq!(
            latest_ctx
                .get_latest_block_and_ts(&latest_server.url)
                .unwrap(),
            (43, 100)
        );
        latest_server.request();

        let call_server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x1234"}"#,
        );
        let call_ctx = test_context(vec![profile(SourceId::PublicNodeRpc, &call_server.url)]);
        assert_eq!(
            call_ctx
                .eth_call(&call_server.url, "0xcontract", "0xdata")
                .unwrap(),
            Some("0x1234".into())
        );
        call_server.request();
    }

    #[test]
    fn coingecko_price_uses_registered_base_url() {
        let server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"tokenized-fund":{"usd":101.25}}"#,
        );
        let ctx = test_context(vec![profile(SourceId::CoinGecko, &server.url)]);

        assert_eq!(
            ctx.get_coingecko_price("tokenized-fund").unwrap(),
            Some(101.25)
        );
        let request = server.request();
        assert!(request.starts_with("GET /simple/price?"));
        assert!(request.contains("ids=tokenized-fund"));
    }

    #[test]
    fn log_helpers_collect_single_rpc_batch() {
        let chunked_server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":[{"blockNumber":"0x1"}]}"#,
        );
        let chunked_ctx = test_context(vec![profile(SourceId::PublicNodeRpc, &chunked_server.url)]);
        let logs = chunked_ctx
            .get_transfer_logs_chunked("0xABC", "ethereum", 1, 2, 10)
            .unwrap();
        assert_eq!(logs.len(), 1);
        chunked_server.request();

        let activity_server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":[{"blockNumber":"0x2"}]}"#,
        );
        let activity_ctx =
            test_context(vec![profile(SourceId::PublicNodeRpc, &activity_server.url)]);
        let logs = activity_ctx
            .get_logs_activity(&activity_server.url, "0xabc", 1, 2, 10)
            .unwrap();
        assert_eq!(logs.len(), 1);
        activity_server.request();
    }
}
