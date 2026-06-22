use anyhow::Result;

pub use crate::sources::adapters::jupiter::{JupiterAdapter, JupiterQuoteEvidence};
use crate::sources::SourceContext;

pub fn fetch_aaplx_quote_100k() -> Result<JupiterQuoteEvidence> {
    let ctx = SourceContext::for_live_collection()?;
    JupiterAdapter::fetch_aaplx_quote_100k(&ctx)
}

pub fn fetch_aaplx_quote_100k_with(ctx: &SourceContext) -> Result<JupiterQuoteEvidence> {
    JupiterAdapter::fetch_aaplx_quote_100k(ctx)
}
