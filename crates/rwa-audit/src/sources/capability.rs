use anyhow::Result;

use super::context::SourceContext;

/// Capability trait for adapters that can resolve a current spot USD price.
/// The `id` parameter is oracle-specific (e.g. CoinGecko coin ID, Yahoo ticker).
pub trait PriceOracle: Send + Sync {
    fn price_usd(&self, ctx: &SourceContext, id: &str) -> Result<Option<f64>>;
}
