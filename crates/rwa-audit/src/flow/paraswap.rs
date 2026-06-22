use anyhow::Result;

use crate::config::ensure_dir;
use crate::flow::config::{flow_data_dir, PANEL_TOKENS, QUOTE_USD_SIZES, USDC};
use crate::flow::output::{write_paraswap_quotes, ParaswapQuoteRow};
use crate::sources::{ParaSwapAdapter, SourceContext};

pub(crate) fn collect_paraswap_quotes() -> Result<()> {
    let out_dir = flow_data_dir();
    ensure_dir(&out_dir)?;

    let ctx = SourceContext::for_live_collection()?;
    let adapter = ParaSwapAdapter;

    let checkpoint = chrono::Utc::now().date_naive().to_string();
    let mut rows = Vec::new();

    for token in PANEL_TOKENS {
        for &usd_size in QUOTE_USD_SIZES {
            let token_amount = usd_size as f64 / token.approx_price_usd;
            let raw_amount = (token_amount * 10f64.powi(token.decimals as i32)) as u128;

            let (route_found, dest_amount, route_summary, error_msg, _) = adapter
                .fetch_price_route(
                    &ctx,
                    token.address,
                    USDC,
                    raw_amount,
                    token.decimals,
                    6,
                    "1",
                )?;

            rows.push(ParaswapQuoteRow {
                checkpoint_date: checkpoint.clone(),
                symbol: token.symbol.to_string(),
                src_token: token.address.to_string(),
                dest_token: USDC.to_string(),
                amount_usd: usd_size,
                route_found,
                dest_amount_usdc: dest_amount,
                route_summary,
                error_message: error_msg,
                source: "ParaSwap API v5 live quote".into(),
            });

            println!("  {} ${usd_size}: route_found={route_found}", token.symbol);
        }
    }

    write_paraswap_quotes(&out_dir, &rows)?;
    println!("Wrote {}", out_dir.join("paraswap_quotes.csv").display());
    Ok(())
}
