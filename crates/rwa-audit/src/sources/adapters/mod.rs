pub mod coingecko;
pub mod ethplorer;
pub mod geckoterminal;
pub mod jupiter;
pub mod paraswap;
pub mod rpc;
pub mod yahoo;

pub use coingecko::CoinGeckoAdapter;
pub use ethplorer::EthplorerAdapter;
pub use geckoterminal::{
    aggregate_solana_search, DailyOhlcv, GeckoTerminalAdapter, PoolMeta, SymbolPoolAggregate,
};
pub use jupiter::{JupiterAdapter, JupiterQuoteEvidence};
pub use paraswap::ParaSwapAdapter;
pub use rpc::PublicNodeRpcAdapter;
pub use yahoo::{GoldDaily, YahooFinanceAdapter};
