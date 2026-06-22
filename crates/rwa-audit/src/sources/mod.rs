pub mod adapter;
pub mod adapters;
pub mod cache;
pub mod capability;
pub mod context;
pub mod fetch;
pub mod profile;
pub mod provenance;
pub mod registry;
pub mod transport;
pub mod types;

#[cfg(test)]
pub(crate) mod test_support;

pub use adapter::SourceAdapter;
pub use adapters::{
    aggregate_solana_search, CoinGeckoAdapter, DailyOhlcv, EthplorerAdapter, GeckoTerminalAdapter,
    GoldDaily, JupiterAdapter, JupiterQuoteEvidence, ParaSwapAdapter, PoolMeta,
    PublicNodeRpcAdapter, SymbolPoolAggregate, YahooFinanceAdapter,
};
pub use cache::ResponseCache;
pub use capability::PriceOracle;
pub use context::SourceContext;
pub use profile::{SourceKind, SourceProfile, SourcesCacheConfig};
pub use provenance::write_json_with_provenance;
pub use registry::SourceRegistry;
pub use types::{Provenance, SourceId, SourceRequest, SourceResponse};
