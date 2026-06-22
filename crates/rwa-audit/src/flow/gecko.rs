//! GeckoTerminal pool/OHLCV helpers — delegates to [`GeckoTerminalAdapter`].

pub use crate::sources::adapters::geckoterminal::{
    aggregate_solana_search, DailyOhlcv, PoolMeta, SymbolPoolAggregate,
};
use crate::sources::{GeckoTerminalAdapter, SourceContext};

use crate::flow::config::PANEL_NETWORK;

pub struct GeckoClient<'a> {
    ctx: &'a SourceContext,
}

impl<'a> GeckoClient<'a> {
    pub fn new(ctx: &'a SourceContext) -> Self {
        Self { ctx }
    }

    pub fn token_pools(&self, token_address: &str) -> anyhow::Result<Vec<PoolMeta>> {
        let adapter = GeckoTerminalAdapter;
        adapter.token_pools(self.ctx, PANEL_NETWORK, token_address)
    }

    pub fn pool_daily_ohlcv(
        &self,
        pool_address: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<DailyOhlcv>> {
        let adapter = GeckoTerminalAdapter;
        adapter.pool_daily_ohlcv(self.ctx, PANEL_NETWORK, pool_address, limit)
    }
}

pub fn fetch_solana_symbol_pool_aggregate(
    ctx: &SourceContext,
    symbol: &str,
) -> anyhow::Result<SymbolPoolAggregate> {
    let adapter = GeckoTerminalAdapter;
    adapter.solana_symbol_pool_aggregate(ctx, symbol)
}
