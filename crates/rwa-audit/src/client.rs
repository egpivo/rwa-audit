//! HTTP/RPC client facade — delegates to [`crate::sources::SourceContext`].

use anyhow::Result;
use serde_json::Value;

use crate::models::RpcResponse;
use crate::sources::SourceContext;

pub use crate::evm::{
    decode_uint256, default_fallback_block, parse_hex_u64, parse_transfer_log, token_amount,
    ACTIVITY_CHUNK_BLOCKS,
};

pub struct HttpClient {
    ctx: SourceContext,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ctx: SourceContext::new()?,
        })
    }

    pub fn for_live() -> Result<Self> {
        Ok(Self {
            ctx: SourceContext::for_live_collection()?,
        })
    }

    pub fn with_context(ctx: SourceContext) -> Self {
        Self { ctx }
    }

    pub fn context(&self) -> &SourceContext {
        &self.ctx
    }

    pub fn http_get(
        &self,
        url: &str,
        params: &[(&str, &str)],
        retries: u32,
    ) -> Result<Option<Value>> {
        self.ctx.http_get(url, params, retries)
    }

    pub fn rpc_call(
        &self,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        self.ctx.rpc_call(rpc_url, method, params, retries)
    }

    pub fn get_current_block(&self, chain: &str) -> Result<u64> {
        self.ctx.get_current_block(chain)
    }

    pub fn get_latest_block_and_ts(&self, rpc_url: &str) -> Result<(u64, i64)> {
        self.ctx.get_latest_block_and_ts(rpc_url)
    }

    pub fn eth_call(&self, rpc_url: &str, contract: &str, data: &str) -> Result<Option<String>> {
        self.ctx.eth_call(rpc_url, contract, data)
    }

    pub fn get_coingecko_price(&self, cg_id: &str) -> Result<Option<f64>> {
        self.ctx.get_coingecko_price(cg_id)
    }

    pub fn get_ethplorer_token_info(&self, contract: &str) -> Result<Value> {
        self.ctx.get_ethplorer_token_info(contract)
    }

    pub fn get_ethplorer_top_holders(&self, contract: &str, limit: u32) -> Result<Vec<Value>> {
        self.ctx.get_ethplorer_top_holders(contract, limit)
    }

    pub fn get_transfer_logs_chunked(
        &self,
        contract: &str,
        chain: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        self.ctx
            .get_transfer_logs_chunked(contract, chain, from_block, to_block, chunk_blocks)
    }

    pub fn get_logs_activity(
        &self,
        rpc_url: &str,
        contract: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        self.ctx
            .get_logs_activity(rpc_url, contract, from_block, to_block, chunk_blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_client_wraps_source_context() {
        let client = HttpClient::new().unwrap();
        assert!(client.context().cache().root().ends_with("cache"));
    }
}
