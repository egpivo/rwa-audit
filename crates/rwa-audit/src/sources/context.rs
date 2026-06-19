use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::config::{rpc_for_chain, SLEEP_BETWEEN_RPC_MS, TRANSFER_TOPIC};
use crate::models::RpcResponse;

use super::adapters::{CoinGeckoAdapter, EthplorerAdapter, PublicNodeRpcAdapter};
use super::cache::ResponseCache;
use super::transport::HttpTransport;

/// Shared runtime for source adapters: transport, cache, and high-level RPC helpers.
pub struct SourceContext {
    transport: HttpTransport,
    cache: ResponseCache,
    ethplorer: EthplorerAdapter,
}

impl SourceContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: HttpTransport::new()?,
            cache: ResponseCache::default(),
            ethplorer: EthplorerAdapter::default(),
        })
    }

    pub fn for_live_collection() -> Result<Self> {
        Ok(Self {
            transport: HttpTransport::new()?,
            cache: ResponseCache::live_collection(),
            ethplorer: EthplorerAdapter::default(),
        })
    }

    pub fn with_cache(mut self, cache: ResponseCache) -> Self {
        self.cache = cache;
        self
    }

    pub fn cache(&self) -> &ResponseCache {
        &self.cache
    }

    pub fn transport(&self) -> &HttpTransport {
        &self.transport
    }

    pub fn http_get(
        &self,
        url: &str,
        params: &[(&str, &str)],
        retries: u32,
    ) -> Result<Option<Value>> {
        self.transport.http_get(url, params, retries)
    }

    pub fn rpc_call(
        &self,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        PublicNodeRpcAdapter::rpc_call(
            &self.transport,
            &self.cache,
            rpc_url,
            method,
            params,
            retries,
        )
    }

    pub fn get_current_block(&self, chain: &str) -> Result<u64> {
        let rpc = rpc_for_chain(chain);
        let r = self
            .rpc_call(rpc, "eth_blockNumber", json!([]), 3)?
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
        thread::sleep(Duration::from_millis(SLEEP_BETWEEN_RPC_MS));
        let r = self.rpc_call(
            rpc_url,
            "eth_call",
            json!([{"to": contract, "data": data}, "latest"]),
            3,
        )?;
        Ok(r.and_then(|r| r.result.and_then(|v| v.as_str().map(str::to_string))))
    }

    pub fn get_coingecko_price(&self, cg_id: &str) -> Result<Option<f64>> {
        CoinGeckoAdapter::simple_price_usd(&self.transport, &self.cache, cg_id)
    }

    pub fn get_ethplorer_token_info(&self, contract: &str) -> Result<Value> {
        self.ethplorer
            .token_info(&self.transport, &self.cache, contract)
    }

    pub fn get_ethplorer_top_holders(&self, contract: &str, limit: u32) -> Result<Vec<Value>> {
        self.ethplorer
            .top_holders(&self.transport, &self.cache, contract, limit)
    }

    pub fn get_transfer_logs_chunked(
        &self,
        contract: &str,
        chain: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        let rpc = rpc_for_chain(chain);
        let mut all_logs = Vec::new();
        let mut chunk = chunk_blocks;
        let mut current = from_block;
        let mut chunk_count = 0u64;
        let max_logs = 50_000usize;

        while current <= to_block && all_logs.len() < max_logs {
            let end = (current + chunk - 1).min(to_block);
            thread::sleep(Duration::from_millis(SLEEP_BETWEEN_RPC_MS));

            let params = json!([{
                "address": contract.to_lowercase(),
                "topics": [TRANSFER_TOPIC],
                "fromBlock": format!("0x{current:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);

            let r = match self.rpc_call(rpc, "eth_getLogs", params, 3)? {
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
                "topics": [TRANSFER_TOPIC],
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
                    thread::sleep(Duration::from_millis(100));
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

    #[test]
    fn source_context_constructs() {
        SourceContext::new().unwrap();
    }
}
