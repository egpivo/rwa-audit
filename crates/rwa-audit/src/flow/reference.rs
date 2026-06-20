use anyhow::Result;
use chrono::NaiveDate;

use crate::sources::{SourceContext, YahooFinanceAdapter};

pub use crate::sources::GoldDaily;

pub fn fetch_gc_futures(start: NaiveDate, end: NaiveDate) -> Result<Vec<GoldDaily>> {
    let ctx = SourceContext::for_live_collection()?;
    let adapter = YahooFinanceAdapter;
    adapter.fetch_gc_futures(&ctx, start, end)
}

pub fn fetch_gc_futures_with(
    ctx: &SourceContext,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<GoldDaily>> {
    let adapter = YahooFinanceAdapter;
    adapter.fetch_gc_futures(ctx, start, end)
}
